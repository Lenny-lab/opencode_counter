//! Cache effectiveness analysis.
//!
//! Prompt caching only pays off when a follow-up request reuses the prefix
//! written by the previous one. That is observable from the data: if message
//! `A` was processed with `cacheRead = 0` it was a cold start, and the tokens
//! it consumed (`A.total`) represent work the next message should have been
//! able to read from cache for free. When message `B` follows and its
//! `cacheRead` falls short of `A.total`, the shortfall is cache *missed* work
//! - tokens that were paid for at full price but could have been cheap.
//!
//! This module pairs consecutive assistant messages within a session and
//! attributes the miss to the later message of each pair.

use std::collections::{BTreeMap, HashMap};

use rusqlite::Connection;

use crate::db::{cutoff_params, Layout, Schema};

/// Running totals of cache misses and what was cacheable in principle.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheTotals {
    /// Tokens that should have been served from cache but were not.
    pub miss: i64,
    /// Tokens that a warm cache would have covered.
    pub expected: i64,
}

impl CacheTotals {
    pub fn add(&mut self, other: CacheTotals) {
        self.miss += other.miss;
        self.expected += other.expected;
    }

    /// Share of cacheable tokens that were missed, 0-100.
    pub fn miss_rate(&self) -> f64 {
        if self.expected <= 0 {
            0.0
        } else {
            self.miss as f64 / self.expected as f64 * 100.0
        }
    }
}

/// Cache-miss totals for a whole window, plus the same split by local day.
#[derive(Debug, Clone, Default)]
pub struct CacheAnalysis {
    pub totals: CacheTotals,
    /// `YYYY-MM-DD` local time -> totals for pairs that closed that day.
    pub by_day: BTreeMap<String, CacheTotals>,
}

impl CacheAnalysis {
    /// Round the miss rate the same way the headline figure is rounded.
    pub fn miss_rate(&self) -> f64 {
        self.totals.miss_rate()
    }
}

/// The minimum a message must contain to take part in the pairing.
#[derive(Debug, Clone, Copy)]
pub struct Candidate {
    pub total: i64,
    pub cache_read: i64,
}

/// Accumulates cache-miss totals while streaming messages in order.
///
/// Holds a single pending message, so pairing costs O(1) memory regardless of
/// how many messages the database contains. Rows must arrive grouped by
/// session and sorted by timestamp within a session; a session change flushes
/// whatever was pending.
#[derive(Debug, Default)]
pub struct CacheMissPairer {
    pending: Option<Candidate>,
    totals: CacheTotals,
}

impl CacheMissPairer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one message. `compaction_between` reports whether a context
    /// compaction happened strictly between the previous message and this one,
    /// which invalidates the prefix and breaks the pair.
    ///
    /// Returns the pair this message closed, if any. A miss belongs to the
    /// message that suffered it, so the caller can file it under this
    /// message's day rather than the predecessor's.
    pub fn push(
        &mut self,
        candidate: Candidate,
        model_changed: bool,
        compaction_between: bool,
    ) -> Option<CacheTotals> {
        let mut paired = None;
        if let Some(prev) = self.pending.take() {
            // A cold previous message means there was no cache to hit, so the
            // tokens it used were never cacheable in the first place.
            if prev.cache_read > 0 && !model_changed && !compaction_between {
                paired = Some(CacheTotals {
                    miss: (prev.total - candidate.cache_read).max(0),
                    expected: prev.total,
                });
            }
        }
        if let Some(pair) = paired {
            self.totals.add(pair);
        }
        self.pending = Some(candidate);
        paired
    }

    /// Call when the session changes to drop an unpaired trailing message.
    pub fn flush(&mut self) {
        self.pending = None;
    }

    pub fn totals(&self) -> CacheTotals {
        self.totals
    }
}

/// Timestamps of context compactions per session, used to break pairs.
fn load_compaction_times(conn: &Connection, schema: &Schema) -> HashMap<String, Vec<i64>> {
    let sql = match schema.layout {
        Layout::V1 if schema.has_part => {
            "SELECT p.session_id, p.time_created FROM part p \
             WHERE json_extract(p.data, '$.type') = 'compaction'"
        }
        Layout::V2 => {
            "SELECT m.session_id, m.time_created FROM session_message m \
             WHERE m.type = 'compaction' AND json_extract(m.data, '$.status') = 'completed'"
        }
        // Without parts we cannot see compactions; pairing will occasionally
        // span one, which slightly over-counts. Better than dropping the
        // analysis entirely.
        _ => return HashMap::new(),
    };
    let mut map: HashMap<String, Vec<i64>> = HashMap::new();
    if let Ok(mut stmt) = conn.prepare(sql) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        }) {
            for entry in rows.flatten() {
                map.entry(entry.0).or_default().push(entry.1);
            }
        }
    }
    for times in map.values_mut() {
        times.sort_unstable();
    }
    map
}

/// Does any compaction fall strictly inside `(prev, current)`?
///
/// Endpoints are excluded: a compaction recorded at the same instant as a
/// message is part of that message, not something that happened between two of
/// them.
fn compaction_between(times: &[i64], prev: i64, current: i64) -> bool {
    // First index strictly after `prev`, then test whether that entry is still
    // strictly before `current`.
    let start = times.partition_point(|&t| t <= prev);
    start < times.len() && times[start] < current
}

/// Compute cache-miss totals across a window of assistant messages.
///
/// `cutoff_ms` restricts the scan to a time window; pass `None` for all time.
pub fn analyze(conn: &Connection, schema: &Schema, cutoff_ms: Option<i64>) -> Result<CacheAnalysis, String> {
    let compactions = load_compaction_times(conn, schema);
    let empty: &[i64] = &[];

    let sql = format!(
        "SELECT m.session_id, \
                COALESCE(json_extract(m.data, '$.modelID'), json_extract(m.data, '$.model.id'), 'unknown'), \
                COALESCE(json_extract(m.data, '$.providerID'), json_extract(m.data, '$.model.providerID'), 'unknown'), \
                COALESCE(json_extract(m.data, '$.tokens.input'), 0), \
                COALESCE(json_extract(m.data, '$.tokens.output'), 0), \
                COALESCE(json_extract(m.data, '$.tokens.reasoning'), 0), \
                COALESCE(json_extract(m.data, '$.tokens.cache.read'), 0), \
                COALESCE(json_extract(m.data, '$.tokens.cache.write'), 0), \
                COALESCE(json_extract(m.data, '$.tokens.total'), 0), \
                COALESCE(json_extract(m.data, '$.time.completed'), m.time_created), \
                date(m.time_created / 1000, 'unixepoch', 'localtime') \
         {} \
         WHERE {} = 'assistant' {} \
         ORDER BY m.session_id ASC, m.time_created ASC",
        schema.message_source(),
        schema.role_expr(),
        match cutoff_ms {
            Some(_) => "AND m.time_created >= :cutoff",
            None => "",
        }
    );

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("cache analysis query failed: {e}"))?;

    // Bind the cutoff when the query uses it so one prepared statement covers
    // both the filtered and unfiltered shapes.
    // The bindings have to outlive the `ParamsFromIter` they hand out.
    let bindings = cutoff_params(cutoff_ms);
    let bound = bindings.as_params();

    let rows = stmt
        .query_map(bound, |row| {
            let input: i64 = row.get(3)?;
            let output: i64 = row.get(4)?;
            let reasoning: i64 = row.get(5)?;
            let cache_read: i64 = row.get(6)?;
            let cache_write: i64 = row.get(7)?;
            let declared_total: i64 = row.get(8)?;
            let computed = input + output + reasoning + cache_read + cache_write;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                Candidate {
                    total: if declared_total > 0 { declared_total } else { computed },
                    cache_read,
                },
                row.get::<_, i64>(9)?,
                row.get::<_, String>(10)?,
            ))
        })
        .map_err(|e| format!("cache analysis query failed: {e}"))?;

    let mut analysis = CacheAnalysis::default();
    let mut pairer = CacheMissPairer::new();
    let mut last_session = String::new();
    let mut last_key = String::new();
    let mut last_ts = 0i64;

    for row in rows {
        let (session, model, provider, candidate, ts, day) =
            row.map_err(|e| format!("cache analysis read failed: {e}"))?;
        if session != last_session {
            pairer.flush();
            last_session = session.clone();
            last_key.clear();
        }
        let key = format!("{model}\u{1}{provider}");
        let model_changed = !last_key.is_empty() && last_key != key;
        let between = if last_key.is_empty() {
            false
        } else {
            compaction_between(
                compactions.get(&session).map(|v| v.as_slice()).unwrap_or(empty),
                last_ts,
                ts,
            )
        };
        if let Some(pair) = pairer.push(candidate, model_changed, between) {
            analysis.by_day.entry(day).or_default().add(pair);
        }
        last_key = key;
        last_ts = ts;
    }
    pairer.flush();

    analysis.totals = pairer.totals();
    Ok(analysis)
}

/// Sessions whose messages never read from cache at all.
///
/// These are worth surfacing separately: a provider that does not support
/// prompt caching, or a model configured with caching off, will show a 100%
/// miss rate that is really a configuration fact rather than a billing loss.
pub fn sessions_without_cache(
    conn: &Connection,
    schema: &Schema,
    cutoff_ms: Option<i64>,
) -> Result<Vec<String>, String> {
    let sql = format!(
        "SELECT m.session_id {} \
         WHERE {} = 'assistant' {} \
         GROUP BY m.session_id \
         HAVING SUM(COALESCE(json_extract(m.data, '$.tokens.cache.read'), 0)) = 0",
        schema.message_source(),
        schema.role_expr(),
        match cutoff_ms {
            Some(_) => "AND m.time_created >= :cutoff",
            None => "",
        }
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("no-cache query failed: {e}"))?;
    // Bind the cutoff when the query uses it so one prepared statement covers
    // both the filtered and unfiltered shapes.
    // The bindings have to outlive the `ParamsFromIter` they hand out.
    let bindings = cutoff_params(cutoff_ms);
    let bound = bindings.as_params();
    let rows = stmt
        .query_map(bound, |row| row.get::<_, String>(0))
        .map_err(|e| format!("no-cache query failed: {e}"))?;
    let mut out: Vec<String> = rows.flatten().collect();
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(total: i64, cache_read: i64) -> Candidate {
        Candidate { total, cache_read }
    }

    #[test]
    fn warm_predecessor_produces_a_miss_for_the_shortfall_only() {
        let mut p = CacheMissPairer::new();
        p.push(candidate(1000, 400), false, false);
        p.push(candidate(1200, 900), false, false);
        let totals = p.totals();
        assert_eq!(totals.expected, 1000);
        assert_eq!(totals.miss, 100);
        assert!((totals.miss_rate() - 10.0).abs() < 1e-9);
    }

    #[test]
    fn a_cold_predecessor_cannot_miss() {
        let mut p = CacheMissPairer::new();
        p.push(candidate(1000, 0), false, false);
        p.push(candidate(1200, 0), false, false);
        // Nothing was cacheable, so there is nothing to have missed.
        assert_eq!(p.totals(), CacheTotals::default());
    }

    #[test]
    fn a_model_switch_breaks_the_pair() {
        let mut p = CacheMissPairer::new();
        p.push(candidate(1000, 400), false, false);
        p.push(candidate(1200, 0), true, false);
        assert_eq!(p.totals(), CacheTotals::default());
    }

    #[test]
    fn a_compaction_between_breaks_the_pair() {
        let mut p = CacheMissPairer::new();
        p.push(candidate(1000, 400), false, false);
        p.push(candidate(1200, 0), false, true);
        assert_eq!(p.totals(), CacheTotals::default());
    }

    #[test]
    fn a_new_session_breaks_the_pair() {
        let mut p = CacheMissPairer::new();
        p.push(candidate(1000, 400), false, false);
        p.flush();
        p.push(candidate(1200, 0), false, false);
        assert_eq!(p.totals(), CacheTotals::default());
    }

    #[test]
    fn extra_cache_read_never_produces_a_negative_miss() {
        let mut p = CacheMissPairer::new();
        p.push(candidate(100, 50), false, false);
        p.push(candidate(200, 300), false, false);
        assert_eq!(p.totals().miss, 0);
        assert_eq!(p.totals().expected, 100);
    }

    #[test]
    fn a_chain_of_three_pairs_two_misses() {
        let mut p = CacheMissPairer::new();
        p.push(candidate(1000, 100), false, false);
        p.push(candidate(1100, 700), false, false);
        p.push(candidate(1200, 900), false, false);
        let totals = p.totals();
        assert_eq!(totals.expected, 1000 + 1100);
        assert_eq!(totals.miss, 300 + 200);
    }

    #[test]
    fn compaction_between_is_exclusive_of_the_endpoints() {
        let times = [500i64, 1500];
        // A compaction exactly at the previous message's timestamp happened
        // before this pair, so it does not break it.
        assert!(!compaction_between(&times, 500, 1000));
        // One at the current message's timestamp is also outside the interval.
        assert!(!compaction_between(&times, 400, 500));
        // Strictly between, however, invalidates the prefix.
        assert!(compaction_between(&times, 100, 1000));
    }

    #[test]
    fn miss_rate_is_zero_when_nothing_was_cacheable() {
        assert_eq!(CacheTotals::default().miss_rate(), 0.0);
    }
}
