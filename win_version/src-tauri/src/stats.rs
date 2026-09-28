//! Aggregating OpenCode's message log into the numbers the UI shows.
//!
//! # Accounting rules
//!
//! These are the rules the rest of the app family uses, kept identical on
//! purpose so the Windows build reports the same figures as the macOS one:
//!
//! * **Total tokens** is always `input + output + reasoning + cache.read +
//!   cache.write`. Cache reads are billable, so they count.
//! * **Cost** is never recomputed here. OpenCode already resolves model
//!   pricing (including charging reasoning tokens at the output rate) and
//!   stores the result at `message.data.cost`; we only sum it. Inventing a
//!   second price table would be a source of silent disagreement.
//! * **Subagent sessions** (`session.parent_id` set) are folded into their
//!   parent. One level only, matching how OpenCode nests tasks: a subagent's
//!   usage belongs to the session that spawned it.
//!
//! # Which timestamp filters a window
//!
//! Per-session and per-model totals are filtered on the session's creation
//! time, while anything drawn per-day (trend, heatmap) is filtered on the
//! message's own time. Mixing the two would double-count a long-running
//! session across days it was not actually used on.

use std::collections::HashMap;

use rusqlite::Connection;
use serde::Serialize;

use crate::cache;
use crate::db::{cutoff_params, Bindings, Database, Layout, Param, Schema};

/// Longest span the day/heatmap series will ever be expanded to, so a
/// multi-year-old database cannot produce an unbounded array.
const MAX_DAYS: i64 = 400;

/// Sessions shown in the Sessions tab.
const RECENT_SESSIONS: usize = 15;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// The five token counters OpenCode records, plus their sum.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenBreakdown {
    pub input: i64,
    pub output: i64,
    pub reasoning: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    pub total: i64,
}

impl TokenBreakdown {
    pub fn new(input: i64, output: i64, reasoning: i64, cache_read: i64, cache_write: i64) -> Self {
        TokenBreakdown {
            input,
            output,
            reasoning,
            cache_read,
            cache_write,
            total: input + output + reasoning + cache_read + cache_write,
        }
    }

    pub fn add(&mut self, other: &TokenBreakdown) {
        self.input += other.input;
        self.output += other.output;
        self.reasoning += other.reasoning;
        self.cache_read += other.cache_read;
        self.cache_write += other.cache_write;
        self.total += other.total;
    }

    /// Tokens that were billed as input, including cache traffic.
    ///
    /// This is the denominator for cache hit rate: a cache read is a hit, a
    /// cache write is billed input that did not come from cache.
    pub fn input_side(&self) -> i64 {
        self.input + self.cache_read + self.cache_write
    }

    /// Tokens that were billed as output.
    pub fn output_side(&self) -> i64 {
        self.output + self.reasoning
    }

    /// Share of billable input that was served from cache, 0-100.
    pub fn cache_hit_rate(&self) -> f64 {
        let denominator = self.input_side();
        if denominator <= 0 {
            0.0
        } else {
            round1(self.cache_read as f64 / denominator as f64 * 100.0)
        }
    }
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

/// Usage since local midnight, shown in the tray icon and the overview.
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodayStat {
    pub cost: f64,
    pub tokens: TokenBreakdown,
}

/// Headline figures for the whole selected window.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub session_count: i64,
    pub message_count: i64,
    pub assistant_message_count: i64,
    /// Number of calendar days spanned by the sessions in range.
    pub day_count: i64,
    pub first_session_date: Option<String>,
    pub last_session_date: Option<String>,
    pub total_cost: f64,
    pub avg_cost_per_day: f64,
    pub tokens: TokenBreakdown,
    pub cache_hit_rate: f64,
    pub avg_tokens_per_session: f64,
    pub median_tokens_per_session: f64,
}

/// One calendar day of usage, zero-filled so the trend line has no gaps.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayStat {
    /// `YYYY-MM-DD`, local time.
    pub date: String,
    pub message_count: i64,
    pub user_message_count: i64,
    pub cost: f64,
    pub tokens: TokenBreakdown,
    pub cache_miss: i64,
    pub cache_expected: i64,
    pub cache_miss_rate: f64,
}

/// A two-hour bucket of activity, the unit the heatmap is drawn from.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeatCell {
    /// `YYYY-MM-DD`, local time.
    pub date: String,
    /// Hour the bucket starts at: 0, 2, 4, ... 22.
    pub block: i64,
    pub message_count: i64,
    pub user_message_count: i64,
    pub cost: f64,
    pub tokens: TokenBreakdown,
}

/// Usage for one model, merged across every provider that serves it.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStat {
    pub name: String,
    /// Every provider seen serving this model, sorted.
    pub providers: Vec<String>,
    pub message_count: i64,
    pub cost: f64,
    pub tokens: TokenBreakdown,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStat {
    pub name: String,
    pub message_count: i64,
    pub cost: f64,
    pub tokens: TokenBreakdown,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStat {
    pub id: String,
    pub name: String,
    /// Filesystem path of the project, empty for OpenCode's own sessions.
    pub path: String,
    /// Top-level sessions only; subagent sessions are not counted separately.
    pub session_count: i64,
    /// All messages, including user turns and subagent turns.
    pub message_count: i64,
    pub cost: f64,
    pub tokens: TokenBreakdown,
    pub latest_session_id: Option<String>,
    pub latest_session_title: Option<String>,
    pub latest_session_updated_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStat {
    pub name: String,
    pub count: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStat {
    pub id: String,
    pub title: String,
    pub project_name: String,
    pub path: String,
    pub slug: String,
    pub cost: f64,
    pub message_count: i64,
    pub provider: String,
    pub model: String,
    /// RFC3339, the session's own last-modified time.
    pub last_updated: Option<String>,
    pub tokens: TokenBreakdown,
}

/// Everything one refresh produces.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub generated_at: String,
    /// The window this snapshot covers, in days. `None` means all time.
    pub days: Option<i64>,
    pub database: String,
    pub today: TodayStat,
    pub overview: Overview,
    pub days_series: Vec<DayStat>,
    pub heatmap: Vec<HeatCell>,
    pub models: Vec<ModelStat>,
    pub providers: Vec<ProviderStat>,
    pub projects: Vec<ProjectStat>,
    pub tools: Vec<ToolStat>,
    pub sessions: Vec<SessionStat>,
    pub cache: CacheSummary,
}

/// Cache effectiveness across the selected window.
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheSummary {
    pub miss: i64,
    pub expected: i64,
    pub miss_rate: f64,
    /// Share of billable input served from cache, 0-100.
    pub hit_rate: f64,
    /// Sessions that read from cache exactly zero times.
    pub no_cache_sessions: i64,
}

// ---------------------------------------------------------------------------
// SQL helpers
// ---------------------------------------------------------------------------

/// `SUM(CASE WHEN <guard> THEN json_extract(...) ELSE 0 END)` for one counter.
fn guarded_sum(guard: &str, json_path: &str) -> String {
    format!(
        "SUM(CASE WHEN {guard} THEN COALESCE(json_extract(m.data, '{json_path}'), 0) ELSE 0 END)"
    )
}

/// The cost and token columns, always in one piece.
///
/// Emitting them together removes any chance of a missing comma or a stray
/// column landing between the two, and guarantees the fixed layout
/// [`read_totals_at`] relies on: cost, then the five counters.
fn totals_columns(guard: &str) -> String {
    let mut columns = vec![format!(
        "COALESCE(SUM(CASE WHEN {guard} THEN COALESCE(json_extract(m.data, '$.cost'), 0) ELSE 0 END), 0) AS cost"
    )];
    for (alias, path) in [
        ("t_input", "$.tokens.input"),
        ("t_output", "$.tokens.output"),
        ("t_reasoning", "$.tokens.reasoning"),
        ("t_cache_read", "$.tokens.cache.read"),
        ("t_cache_write", "$.tokens.cache.write"),
    ] {
        columns.push(format!("{} AS {alias}", guarded_sum(guard, path)));
    }
    columns.join(", ")
}

fn text_at(row: &rusqlite::Row<'_>, index: usize) -> String {
    row.get::<_, Option<String>>(index).ok().flatten().unwrap_or_default()
}

fn i64_at(row: &rusqlite::Row<'_>, index: usize) -> i64 {
    row.get::<_, Option<i64>>(index).ok().flatten().unwrap_or(0)
}

fn f64_at(row: &rusqlite::Row<'_>, index: usize) -> f64 {
    row.get::<_, Option<f64>>(index).ok().flatten().unwrap_or(0.0)
}

/// Read the six contiguous columns produced by [`totals_columns`], starting at
/// `base`.
fn read_totals_at(row: &rusqlite::Row<'_>, base: usize) -> (f64, TokenBreakdown) {
    let cost = f64_at(row, base);
    let tokens = TokenBreakdown::new(
        i64_at(row, base + 1),
        i64_at(row, base + 2),
        i64_at(row, base + 3),
        i64_at(row, base + 4),
        i64_at(row, base + 5),
    );
    (cost, tokens)
}


/// A predicate restricting rows to the reporting window, or nothing at all
/// when the window is unbounded.
///
/// This is a suffix rather than a bare fragment so an absent window cannot
/// accidentally become a comparison against NULL, which matches no rows and
/// would silently report an empty database.
fn window_filter(cutoff: Option<i64>, column: &str) -> String {
    match cutoff {
        Some(_) => format!(" AND {column} >= :cutoff"),
        None => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Date helpers
// ---------------------------------------------------------------------------

/// Milliseconds since the epoch at local midnight `days_back` days ago.
fn local_midnight_ms(days_back: i64) -> i64 {
    use chrono::{Duration, Local, TimeZone};
    let today = Local::now().date_naive();
    let day = today - Duration::days(days_back);
    Local
        .from_local_datetime(&day.and_hms_opt(0, 0, 0).expect("midnight is a valid time"))
        .earliest()
        .map(|dt| dt.timestamp_millis())
        .unwrap_or(0)
}

/// Start of the reporting window. `days = Some(1)` means today only.
fn window_start(days: Option<i64>) -> Option<i64> {
    days.map(|count| local_midnight_ms((count - 1).max(0)))
}

fn now_rfc3339() -> String {
    chrono::Local::now().to_rfc3339()
}

fn ms_to_rfc3339(ms: i64) -> Option<String> {
    chrono::DateTime::from_timestamp_millis(ms).map(|dt| dt.to_rfc3339())
}

/// `YYYY-MM-DD` in local time, or `None` for a zero timestamp.
///
/// Splitting the epoch into days by dividing by 86 400 000 would give the UTC
/// date, which is the previous or next day for anyone outside UTC, so go
/// through chrono to get the same date the SQL `localtime` bucketing produces.
fn local_date(ms: i64) -> Option<String> {
    if ms <= 0 {
        return None;
    }
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|utc| utc.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string())
}

/// `YYYY-MM-DD` for a `YYYY-MM-DD` string produced by SQLite, used to turn a
/// row's day key into a human-facing date without re-parsing timestamps.
fn is_iso_date(value: &str) -> bool {
    value.len() == 10 && value.as_bytes().get(4) == Some(&b'-') && value.as_bytes().get(7) == Some(&b'-')
}

// ---------------------------------------------------------------------------
// Individual queries
// ---------------------------------------------------------------------------

/// Usage since local midnight, keyed off the message time.
fn query_today(conn: &Connection, schema: &Schema) -> Result<TodayStat, String> {
    let guard = format!("{} = 'assistant'", schema.role_expr());
    let sql = format!(
        "SELECT {totals} {source} WHERE m.time_created >= {cut}",
        totals = totals_columns(&guard),
        source = schema.message_source(),
        cut = local_midnight_ms(0)
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("today query failed: {e}"))?;
    let mut rows = stmt
        .query_map([], |row| Ok(read_totals_at(row, 0)))
        .map_err(|e| format!("today query failed: {e}"))?;
    match rows.next() {
        Some(row) => {
            let (cost, tokens) = row.map_err(|e| format!("today query failed: {e}"))?;
            Ok(TodayStat { cost, tokens })
        }
        // An empty table still aggregates to one row, but be defensive.
        None => Ok(TodayStat::default()),
    }
}

/// Session count and the span they cover, plus assistant message totals.
fn query_overview(conn: &Connection, schema: &Schema, cutoff: Option<i64>) -> Result<Overview, String> {
    let mut overview = Overview::default();

    // Sessions: top-level only, so a busy subagent tree does not inflate the
    // session count.
    let session_sql = format!(
        "SELECT COUNT(*), MIN(time_created), MAX(time_created) FROM session \
         WHERE parent_id IS NULL{}",
        window_filter(cutoff, "time_created")
    );
    let (count, min_ms, max_ms) = conn
        .query_row(&session_sql, cutoff_params(cutoff).as_params(), |row| {
            Ok((i64_at(row, 0), i64_at(row, 1), i64_at(row, 2)))
        })
        .map_err(|e| format!("session count query failed: {e}"))?;
    overview.session_count = count;
    overview.first_session_date = local_date(min_ms);
    overview.last_session_date = local_date(max_ms);
    if max_ms > 0 && min_ms > 0 {
        overview.day_count = ((max_ms - min_ms) as f64 / 86_400_000.0).ceil() as i64 + 1;
    }

    // Assistant totals, filtered on session creation time.
    let guard = format!("{} = 'assistant'", schema.role_expr());
    let totals_sql = format!(
        "SELECT {totals} {source} WHERE {guard}{cut}",
        totals = totals_columns(&guard),
        source = schema.message_source(),
        guard = guard,
        cut = window_filter(cutoff, "s.time_created")
    );
    if let Ok(row) = conn.query_row(&totals_sql, cutoff_params(cutoff).as_params(), |row| Ok(read_totals_at(row, 0))) {
        let (cost, tokens) = row;
        overview.total_cost = cost;
        overview.tokens = tokens;
    }
    overview.avg_cost_per_day = if overview.day_count > 0 {
        overview.total_cost / overview.day_count as f64
    } else {
        0.0
    };
    overview.cache_hit_rate = overview.tokens.cache_hit_rate();

    // Every message, all roles, plus the assistant-only count.
    let count_sql = format!(
        "SELECT COUNT(*), SUM(CASE WHEN {guard} THEN 1 ELSE 0 END) \
         {source} WHERE 1 = 1{cut}",
        guard = guard,
        source = schema.message_source(),
        cut = window_filter(cutoff, "s.time_created")
    );
    let counts = conn
        .query_row(&count_sql, cutoff_params(cutoff).as_params(), |row| {
            Ok((i64_at(row, 0), i64_at(row, 1)))
        })
        .map_err(|e| format!("message count query failed: {e}"))?;
    overview.message_count = counts.0;
    overview.assistant_message_count = counts.1;

    // Mean and median tokens per top-level session, with subagent usage
    // folded into the session that spawned it.
    let per_session = session_token_distribution(conn, schema, cutoff)?;
    if !per_session.is_empty() {
        let total: i64 = per_session.iter().sum();
        let divisor = overview.session_count.max(1) as f64;
        overview.avg_tokens_per_session = total as f64 / divisor;
        let mut sorted = per_session;
        sorted.sort_unstable();
        let mid = sorted.len() / 2;
        overview.median_tokens_per_session = if sorted.len() % 2 == 0 && sorted.len() > 1 {
            (sorted[mid - 1] + sorted[mid]) as f64 / 2.0
        } else {
            sorted[mid] as f64
        };
    }

    Ok(overview)
}

/// Tokens per top-level session, with subagent sessions merged into their
/// parent. Returns one entry per session that has at least one message.
fn session_token_distribution(
    conn: &Connection,
    schema: &Schema,
    cutoff: Option<i64>,
) -> Result<Vec<i64>, String> {
    let guard = format!("{} = 'assistant'", schema.role_expr());
    // The four counters billed per request. Reasoning is excluded here to stay
    // consistent with how this figure has always been reported: it measures
    // prompt work, not generation. Each term is coalesced so a message
    // missing one key does not null out the whole session.
    let session_tokens = "COALESCE(SUM(\
         COALESCE(json_extract(m.data, '$.tokens.input'), 0) \
       + COALESCE(json_extract(m.data, '$.tokens.output'), 0) \
       + COALESCE(json_extract(m.data, '$.tokens.cache.read'), 0) \
       + COALESCE(json_extract(m.data, '$.tokens.cache.write'), 0)\
     ), 0) AS session_tokens";
    let sql = format!(
        "SELECT {session_tokens} {source} \
         WHERE {guard}{cut} \
         GROUP BY COALESCE(s.parent_id, s.id)",
        source = schema.message_source(),
        guard = guard,
        cut = window_filter(cutoff, "s.time_created")
    );

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("per-session query failed: {e}"))?;
    let rows = stmt
        .query_map(cutoff_params(cutoff).as_params(), |row| Ok(i64_at(row, 0)))
        .map_err(|e| format!("per-session query failed: {e}"))?;
    Ok(rows.flatten().collect())
}

/// Per-day usage, keyed off the message time so the trend tracks actual usage.
fn query_days(conn: &Connection, schema: &Schema, cutoff: Option<i64>) -> Result<Vec<DayStat>, String> {
    let guard = format!("{} = 'assistant'", schema.role_expr());
    let day_expr = "date(m.time_created / 1000, 'unixepoch', 'localtime')";
    let sql = format!(
        "SELECT {day} AS day, COUNT(*), \
                SUM(CASE WHEN {role} = 'user' THEN 1 ELSE 0 END) AS user_count, \
                {totals} \
         {source} \
         WHERE 1 = 1{cut} \
         GROUP BY day ORDER BY day",
        day = day_expr,
        role = schema.role_expr(),
        totals = totals_columns(&guard),
        source = schema.message_source(),
        cut = window_filter(cutoff, "m.time_created")
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("daily query failed: {e}"))?;
    let rows = stmt
        .query_map(cutoff_params(cutoff).as_params(), |row| {
            let date = text_at(row, 0);
            // Every turn, user and assistant alike, so the day rows line up
            // with the heatmap's message counts.
            let message_count = i64_at(row, 1);
            let user_message_count = i64_at(row, 2);
            let (cost, tokens) = read_totals_at(row, 3);
            Ok(DayStat {
                date,
                message_count,
                user_message_count,
                cost,
                tokens,
                ..Default::default()
            })
        })
        .map_err(|e| format!("daily query failed: {e}"))?;
    let collected: Vec<DayStat> = rows.flatten().collect();
    debug_assert!(collected.iter().all(|d| is_iso_date(&d.date)));
    Ok(collected)
}

/// Two-hour activity buckets for the heatmap.
fn query_heatmap(conn: &Connection, schema: &Schema, cutoff: Option<i64>) -> Result<Vec<HeatCell>, String> {
    let guard = format!("{} = 'assistant'", schema.role_expr());
    let day_expr = "date(m.time_created / 1000, 'unixepoch', 'localtime')";
    // Truncating the hour to an even number gives twelve buckets per day.
    let block_expr =
        "(CAST(strftime('%H', m.time_created / 1000, 'unixepoch', 'localtime') AS INTEGER) / 2) * 2";
    let sql = format!(
        "SELECT {day} AS day, {block} AS block, COUNT(*), \
                SUM(CASE WHEN {role} = 'user' THEN 1 ELSE 0 END), \
                {totals} \
         {source} \
         WHERE 1 = 1{cut} \
         GROUP BY day, block ORDER BY day, block",
        day = day_expr,
        block = block_expr,
        role = schema.role_expr(),
        totals = totals_columns(&guard),
        source = schema.message_source(),
        cut = window_filter(cutoff, "m.time_created")
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("heatmap query failed: {e}"))?;
    let rows = stmt
        .query_map(cutoff_params(cutoff).as_params(), |row| {
            let date = text_at(row, 0);
            let block = i64_at(row, 1);
            let message_count = i64_at(row, 2);
            let user_message_count = i64_at(row, 3);
            let (cost, tokens) = read_totals_at(row, 4);
            Ok(HeatCell {
                date,
                block,
                message_count,
                user_message_count,
                cost,
                tokens,
            })
        })
        .map_err(|e| format!("heatmap query failed: {e}"))?;
    Ok(rows.flatten().collect())
}

/// Usage per model, then merged across providers in Rust.
///
/// Two providers can serve the same model id with different pricing, so the
/// provider list is kept as a set rather than collapsed, but the totals are
/// reported once.
fn query_models(conn: &Connection, schema: &Schema, cutoff: Option<i64>) -> Result<Vec<ModelStat>, String> {
    let guard = format!("{} = 'assistant'", schema.role_expr());
    let sql = format!(
        "SELECT COALESCE(json_extract(m.data, '$.modelID'), json_extract(m.data, '$.model.id'), 'unknown') AS model, \
                COALESCE(json_extract(m.data, '$.providerID'), json_extract(m.data, '$.model.providerID'), 'unknown') AS provider, \
                COUNT(*), {totals} \
         {source} \
         WHERE {guard}{cut} \
         GROUP BY model, provider",
        totals = totals_columns(&guard),
        source = schema.message_source(),
        guard = guard,
        cut = window_filter(cutoff, "s.time_created")
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("model query failed: {e}"))?;
    let rows = stmt
        .query_map(cutoff_params(cutoff).as_params(), |row| {
            let model = text_at(row, 0);
            let provider = text_at(row, 1);
            let count = i64_at(row, 2);
            let (cost, tokens) = read_totals_at(row, 3);
            Ok((model, provider, count, cost, tokens))
        })
        .map_err(|e| format!("model query failed: {e}"))?;

    let mut merged: HashMap<String, ModelStat> = HashMap::new();
    for (model, provider, count, cost, tokens) in rows.flatten() {
        let entry = merged.entry(model.clone()).or_insert_with(|| ModelStat {
            name: model,
            ..Default::default()
        });
        // A missing provider id tells us nothing useful, so it is not listed.
        if !provider.is_empty() && !entry.providers.contains(&provider) {
            entry.providers.push(provider);
        }
        entry.message_count += count;
        entry.cost += cost;
        entry.tokens.add(&tokens);
    }
    for stat in merged.values_mut() {
        stat.providers.sort();
    }
    let mut out: Vec<ModelStat> = merged.into_values().collect();
    out.sort_by(|a, b| b.tokens.total.cmp(&a.tokens.total).then_with(|| a.name.cmp(&b.name)));
    Ok(out)
}

fn query_providers(
    conn: &Connection,
    schema: &Schema,
    cutoff: Option<i64>,
) -> Result<Vec<ProviderStat>, String> {
    let guard = format!("{} = 'assistant'", schema.role_expr());
    let sql = format!(
        "SELECT COALESCE(json_extract(m.data, '$.providerID'), json_extract(m.data, '$.model.providerID'), 'unknown') AS provider, \
                COUNT(*), {totals} \
         {source} \
         WHERE {guard}{cut} \
         GROUP BY provider",
        totals = totals_columns(&guard),
        source = schema.message_source(),
        guard = guard,
        cut = window_filter(cutoff, "s.time_created")
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("provider query failed: {e}"))?;
    let rows = stmt
        .query_map(cutoff_params(cutoff).as_params(), |row| {
            let name = text_at(row, 0);
            let count = i64_at(row, 1);
            let (cost, tokens) = read_totals_at(row, 2);
            Ok(ProviderStat {
                name,
                message_count: count,
                cost,
                tokens,
            })
        })
        .map_err(|e| format!("provider query failed: {e}"))?;
    let mut out: Vec<ProviderStat> = rows.flatten().collect();
    out.sort_by(|a, b| b.tokens.total.cmp(&a.tokens.total).then_with(|| a.name.cmp(&b.name)));
    Ok(out)
}

/// Per-project usage. The name falls back to the last path component because
/// OpenCode leaves `project.name` NULL for local projects.
fn query_projects(
    conn: &Connection,
    schema: &Schema,
    cutoff: Option<i64>,
) -> Result<Vec<ProjectStat>, String> {
    if !schema.has_project {
        return Ok(Vec::new());
    }
    let guard = format!("{} = 'assistant'", schema.role_expr());
    let sql = format!(
        "SELECT p.id, p.worktree, p.name, \
                COUNT(DISTINCT CASE WHEN s.parent_id IS NULL THEN s.id END) AS session_count, \
                COUNT(m.id) AS message_count, \
                {totals}, \
                (SELECT s2.id FROM session s2 WHERE s2.project_id = p.id AND s2.parent_id IS NULL \
                 ORDER BY s2.time_updated DESC LIMIT 1) AS latest_id, \
                (SELECT s2.title FROM session s2 WHERE s2.project_id = p.id AND s2.parent_id IS NULL \
                 ORDER BY s2.time_updated DESC LIMIT 1) AS latest_title, \
                (SELECT s2.time_updated FROM session s2 WHERE s2.project_id = p.id AND s2.parent_id IS NULL \
                 ORDER BY s2.time_updated DESC LIMIT 1) AS latest_updated \
         FROM project p \
         JOIN session s ON s.project_id = p.id \
         {message_join} \
         WHERE 1 = 1{cut} \
         GROUP BY p.id \
         HAVING session_count > 0 \
         ORDER BY cost DESC",
        totals = totals_columns(&guard),
        message_join = schema.message_join(),
        cut = window_filter(cutoff, "s.time_created")
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("project query failed: {e}"))?;
    let rows = stmt
        .query_map(cutoff_params(cutoff).as_params(), |row| {
            let worktree = text_at(row, 1);
            let declared = row.get::<_, Option<String>>(2).ok().flatten();
            let name = declared.unwrap_or_else(|| basename(&worktree));
            // Columns: id, worktree, name, session_count, message_count, then
            // cost and the five token counters, then the three latest-* picks.
            let (cost, tokens) = read_totals_at(row, 5);
            Ok(ProjectStat {
                id: text_at(row, 0),
                name,
                path: worktree,
                session_count: i64_at(row, 3),
                message_count: i64_at(row, 4),
                cost,
                tokens,
                latest_session_id: Some(text_at(row, 11)).filter(|s| !s.is_empty()),
                latest_session_title: Some(text_at(row, 12)).filter(|s| !s.is_empty()),
                latest_session_updated_at: ms_to_rfc3339(i64_at(row, 13)),
            })
        })
        .map_err(|e| format!("project query failed: {e}"))?;
    let mut out: Vec<ProjectStat> = rows.flatten().collect();
    out.sort_by(|a, b| b.tokens.total.cmp(&a.tokens.total).then_with(|| a.name.cmp(&b.name)));
    Ok(out)
}

/// Last path component, tolerating both separators so a Windows-style path
/// does not come back as one long name.
fn basename(path: &str) -> String {
    let trimmed = path.trim_end_matches(['/', '\\']);
    // OpenCode's own project has `/` as its worktree, so there is no last
    // component to take. Falling back to the path itself matches what
    // `URL.lastPathComponent` yields for a root, and beats showing a blank
    // row for the busiest project on the machine.
    trimmed
        .rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .unwrap_or(path)
        .to_string()
}

/// Tool call frequency. Parts live in their own table on V1 and inline in the
/// message JSON on V2.
fn query_tools(conn: &Connection, schema: &Schema, cutoff: Option<i64>) -> Result<Vec<ToolStat>, String> {
    let sql = match schema.layout {
        Layout::V1 if schema.has_part => format!(
            "SELECT COALESCE(json_extract(p.data, '$.tool'), 'unknown') AS tool, COUNT(*) \
             FROM part p JOIN session s ON s.id = p.session_id \
             WHERE json_extract(p.data, '$.type') = 'tool'{} \
             GROUP BY tool ORDER BY COUNT(*) DESC LIMIT 20",
            window_filter(cutoff, "s.time_created")
        ),
        Layout::V2 => format!(
            "SELECT COALESCE(json_extract(item.value, '$.tool'), 'unknown') AS tool, COUNT(*) \
             FROM session_message m, json_each(m.data, '$.content') AS item \
             WHERE m.type = 'assistant' \
               AND json_extract(item.value, '$.type') = 'tool'{} \
             GROUP BY tool ORDER BY COUNT(*) DESC LIMIT 20",
            window_filter(cutoff, "m.time_created")
        ),
        _ => return Ok(Vec::new()),
    };
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("tool query failed: {e}"))?;
    let rows = stmt
        .query_map(cutoff_params(cutoff).as_params(), |row| {
            Ok((text_at(row, 0), i64_at(row, 1)))
        })
        .map_err(|e| format!("tool query failed: {e}"))?;
    let pairs: Vec<(String, i64)> = rows.flatten().collect();
    let total: i64 = pairs.iter().map(|(_, count)| count).sum();
    Ok(pairs
        .into_iter()
        .map(|(name, count)| ToolStat {
            name,
            count,
            percentage: if total > 0 {
                round1(count as f64 / total as f64 * 100.0)
            } else {
                0.0
            },
        })
        .collect())
}

/// The most recently touched top-level sessions, with their subagent usage
/// rolled into the parent.
fn query_sessions(
    conn: &Connection,
    schema: &Schema,
    cutoff: Option<i64>,
) -> Result<Vec<SessionStat>, String> {
    // Metadata first: only these sessions need usage looked up.
    let meta_sql = "SELECT s.id, s.title, COALESCE(s.directory, ''), COALESCE(s.slug, ''), s.time_updated, \
                           COALESCE(p.worktree, ''), p.name \
                    FROM session s LEFT JOIN project p ON p.id = s.project_id \
                    WHERE s.parent_id IS NULL \
                    ORDER BY s.time_updated DESC LIMIT ?1";
    let mut meta_stmt = conn
        .prepare(meta_sql)
        .map_err(|e| format!("session metadata query failed: {e}"))?;

    struct Meta {
        id: String,
        title: String,
        path: String,
        slug: String,
        updated: i64,
        project_name: String,
    }

    let metas: Vec<Meta> = {
        let rows = meta_stmt
            .query_map([RECENT_SESSIONS as i64], |row| {
                let worktree = text_at(row, 5);
                let declared = row.get::<_, Option<String>>(6).ok().flatten();
                Ok(Meta {
                    id: text_at(row, 0),
                    title: text_at(row, 1),
                    path: worktree.clone(),
                    slug: text_at(row, 3),
                    updated: i64_at(row, 4),
                    project_name: declared.unwrap_or_else(|| basename(&worktree)),
                })
            })
            .map_err(|e| format!("session metadata query failed: {e}"))?;
        rows.flatten().collect()
    };
    if metas.is_empty() {
        return Ok(Vec::new());
    }

    // Usage per root session, which also covers each session's subagents.
    let guard = format!("{} = 'assistant'", schema.role_expr());
    let usage_sql = format!(
        "SELECT COALESCE(s.parent_id, s.id) AS root_id, COUNT(*), {totals} \
         {source} \
         WHERE {guard}{cut} \
         GROUP BY root_id",
        totals = totals_columns(&guard),
        source = schema.message_source(),
        guard = guard,
        cut = window_filter(cutoff, "s.time_created")
    );
    let mut usage_stmt = conn
        .prepare(&usage_sql)
        .map_err(|e| format!("session usage query failed: {e}"))?;
    let mut usage: HashMap<String, (i64, f64, TokenBreakdown)> = HashMap::new();
    {
        let rows = usage_stmt
            .query_map(cutoff_params(cutoff).as_params(), |row| {
                let count = i64_at(row, 1);
                let (cost, tokens) = read_totals_at(row, 2);
                Ok((text_at(row, 0), count, cost, tokens))
            })
            .map_err(|e| format!("session usage query failed: {e}"))?;
        for (root, count, cost, tokens) in rows.flatten() {
            usage.insert(root, (count, cost, tokens));
        }
    }

    // Both lookups below only care about the sessions we are about to return,
    // so they are restricted to those ids. Without that, the newest-first
    // model scan would read the entire message log to answer a question about
    // fifteen rows.
    let ids: Vec<String> = metas.iter().map(|m| m.id.clone()).collect();
    let marks = vec!["?"; ids.len()].join(", ");
    let binds: Bindings = ids.iter().map(|id| Param::Text(id.as_str())).collect();

    // Message totals per session (all roles) and the model each session last
    // used. Scanning newest-first and keeping the first hit per session gets
    // that without a correlated subquery per row.
    let mut message_counts: HashMap<String, i64> = HashMap::new();
    let mut last_model: HashMap<String, (String, String)> = HashMap::new();
    {
        let sql = format!(
            "SELECT m.session_id, m.data {source} \
             WHERE {role} = 'assistant' AND m.session_id IN ({marks}) \
             ORDER BY m.time_created DESC",
            source = schema.message_source(),
            role = schema.role_expr(),
            marks = marks
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| format!("session model query failed: {e}"))?;
        let rows = stmt
            .query_map(binds.as_params(), |row| Ok((text_at(row, 0), text_at(row, 1))))
            .map_err(|e| format!("session model query failed: {e}"))?;
        for (session, data) in rows.flatten() {
            if !last_model.contains_key(&session) {
                let parsed: serde_json::Value = serde_json::from_str(&data).unwrap_or_default();
                let model = parsed
                    .get("modelID")
                    .or_else(|| parsed.get("model").and_then(|m| m.get("id")))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let provider = parsed
                    .get("providerID")
                    .or_else(|| parsed.get("model").and_then(|m| m.get("providerID")))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                last_model.insert(session, (provider, model));
            }
        }
    }
    {
        let sql = format!(
            "SELECT m.session_id, COUNT(*) {source} WHERE m.session_id IN ({marks}) GROUP BY m.session_id",
            source = schema.message_source(),
            marks = marks
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| format!("session count query failed: {e}"))?;
        let rows = stmt
            .query_map(binds.as_params(), |row| Ok((text_at(row, 0), i64_at(row, 1))))
            .map_err(|e| format!("session count query failed: {e}"))?;
        for (session, count) in rows.flatten() {
            message_counts.insert(session, count);
        }
    }

    let mut out: Vec<SessionStat> = metas
        .into_iter()
        .map(|meta| {
            let (count, cost, tokens) = usage
                .get(&meta.id)
                .cloned()
                .unwrap_or((0, 0.0, TokenBreakdown::default()));
            let (provider, model) = last_model
                .get(&meta.id)
                .cloned()
                .unwrap_or((String::new(), String::new()));
            SessionStat {
                id: meta.id.clone(),
                title: meta.title,
                project_name: meta.project_name,
                path: meta.path,
                slug: meta.slug,
                cost,
                // Show every message in the session, not just assistant turns.
                message_count: message_counts.get(&meta.id).copied().unwrap_or(count),
                provider,
                model,
                last_updated: ms_to_rfc3339(meta.updated),
                tokens,
            }
        })
        .collect();
    out.retain(|s| s.message_count > 0 || s.cost > 0.0 || s.tokens.total > 0);
    Ok(out)
}

/// Expand a sparse day series into a dense one so charts have no gaps.
///
/// Days are generated in the server's local timezone. Crossing a DST boundary
/// is handled by walking calendar dates rather than adding fixed durations.
fn zero_fill_days(days: Vec<DayStat>, range: Option<i64>) -> Vec<DayStat> {
    use chrono::{Duration, NaiveDate};

    if days.is_empty() {
        return Vec::new();
    }
    let parse = |value: &str| NaiveDate::parse_from_str(value, "%Y-%m-%d");
    let last = match parse(&days.last().expect("non-empty").date) {
        Ok(date) => date,
        Err(_) => return days,
    };

    // The window may begin before the first day that has data; start there so
    // the chart still covers the full requested range.
    let first = match range {
        Some(count) => {
            let start = last - Duration::days((count - 1).max(0));
            let earliest = match parse(&days.first().expect("non-empty").date) {
                Ok(date) => date,
                Err(_) => return days,
            };
            start.max(earliest)
        }
        None => match parse(&days.first().expect("non-empty").date) {
            Ok(date) => date,
            Err(_) => return days,
        },
    };

    let span = (last - first).num_days() + 1;
    if span <= 0 || span > MAX_DAYS {
        // Too sparse to expand safely - hand back what we have.
        return days;
    }

    let mut by_date: HashMap<String, DayStat> = days
        .into_iter()
        .map(|day| (day.date.clone(), day))
        .collect();
    let mut out = Vec::with_capacity(span as usize);
    let mut cursor = first;
    for _ in 0..span {
        let key = cursor.format("%Y-%m-%d").to_string();
        out.push(by_date.remove(&key).unwrap_or(DayStat {
            date: key,
            ..Default::default()
        }));
        cursor += Duration::days(1);
    }
    out
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Read every aggregate the UI needs in one pass over the database.
pub fn build_snapshot(db: &Database, days: Option<i64>) -> Result<Snapshot, String> {
    let conn = &db.conn;
    let schema = &db.schema;

    // One cutoff serves both timestamp conventions: the session-time
    // aggregates and the message-time ones are asked for the same window.
    let cutoff = window_start(days);

    let today = query_today(conn, schema)?;
    let overview = query_overview(conn, schema, cutoff)?;
    let days_series = zero_fill_days(query_days(conn, schema, cutoff)?, days);
    let heatmap = query_heatmap(conn, schema, cutoff)?;
    let models = query_models(conn, schema, cutoff)?;
    let providers = query_providers(conn, schema, cutoff)?;
    let projects = query_projects(conn, schema, cutoff)?;
    let tools = query_tools(conn, schema, cutoff)?;
    let sessions = query_sessions(conn, schema, cutoff)?;

    let analysis = cache::analyze(conn, schema, cutoff)?;
    let no_cache = cache::sessions_without_cache(conn, schema, cutoff)?.len() as i64;
    let cache = CacheSummary {
        miss: analysis.totals.miss,
        expected: analysis.totals.expected,
        miss_rate: analysis.totals.miss_rate(),
        hit_rate: overview.tokens.cache_hit_rate(),
        no_cache_sessions: no_cache,
    };

    // Fold the per-day cache pairs into the day series so the trend chart can
    // plot misses against cacheable volume on the same axis.
    let mut days_series = days_series;
    for day in days_series.iter_mut() {
        if let Some(pair) = analysis.by_day.get(&day.date) {
            day.cache_miss = pair.miss;
            day.cache_expected = pair.expected;
            day.cache_miss_rate = if pair.expected > 0 {
                round1(pair.miss as f64 / pair.expected as f64 * 100.0)
            } else {
                0.0
            };
        }
    }

    Ok(Snapshot {
        generated_at: now_rfc3339(),
        days,
        database: db.path.display().to_string(),
        today,
        overview,
        days_series,
        heatmap,
        models,
        providers,
        projects,
        tools,
        sessions,
        cache,
    })
}

/// Today's figures only, for the tray icon where a full snapshot is wasteful.
pub fn build_today(db: &Database) -> Result<TodayStat, String> {
    query_today(&db.conn, &db.schema)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_breakdown_totals_every_counter() {
        let t = TokenBreakdown::new(10, 20, 30, 40, 50);
        assert_eq!(t.total, 150);
        assert_eq!(t.input_side(), 10 + 40 + 50);
        assert_eq!(t.output_side(), 20 + 30);
    }

    #[test]
    fn cache_hit_rate_uses_billable_input_as_the_denominator() {
        let t = TokenBreakdown::new(0, 0, 0, 900, 100);
        // 900 of 1000 billable input tokens came from cache.
        assert_eq!(t.cache_hit_rate(), 90.0);
    }

    #[test]
    fn cache_hit_rate_is_zero_without_billable_input() {
        assert_eq!(TokenBreakdown::default().cache_hit_rate(), 0.0);
    }

    #[test]
    fn cache_hit_rate_rounds_to_one_decimal() {
        let t = TokenBreakdown::new(1, 0, 0, 1, 0);
        assert_eq!(t.cache_hit_rate(), 50.0);
        let t = TokenBreakdown::new(2, 0, 0, 1, 0);
        assert_eq!(t.cache_hit_rate(), 33.3);
    }

    #[test]
    fn adding_breakdowns_sums_totals_too() {
        let mut a = TokenBreakdown::new(1, 2, 3, 4, 5);
        a.add(&TokenBreakdown::new(10, 20, 30, 40, 50));
        assert_eq!(a, TokenBreakdown::new(11, 22, 33, 44, 55));
    }

    #[test]
    fn basename_handles_both_separators_and_trailing_slashes() {
        assert_eq!(basename("/home/me/project"), "project");
        assert_eq!(basename(r"C:\Users\me\project"), "project");
        assert_eq!(basename("/home/me/project/"), "project");
        assert_eq!(basename(r"C:\Users\me\project\"), "project");
        assert_eq!(basename(""), "");
        assert_eq!(basename("/"), "/");
    }

    #[test]
    fn zero_fill_inserts_empty_days_between_observations() {
        use chrono::NaiveDate;
        let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let make = |offset: i64, total: i64| DayStat {
            date: (today - chrono::Duration::days(offset)).format("%Y-%m-%d").to_string(),
            tokens: TokenBreakdown::new(0, 0, 0, total, 0),
            ..Default::default()
        };
        let sparse = vec![make(2, 5), make(0, 7)];
        let filled = zero_fill_days(sparse, Some(3));
        assert_eq!(filled.len(), 3);
        assert_eq!(filled[1].tokens.total, 0);
        assert_eq!(filled[0].tokens.total, 5);
        assert_eq!(filled[2].tokens.total, 7);
    }

    #[test]
    fn zero_fill_handles_an_empty_series() {
        assert!(zero_fill_days(Vec::new(), Some(30)).is_empty());
    }

    #[test]
    fn zero_fill_leaves_unparseable_dates_alone() {
        let bad = vec![DayStat {
            date: "not-a-date".to_string(),
            ..Default::default()
        }];
        assert_eq!(zero_fill_days(bad, Some(30)).len(), 1);
    }

    #[test]
    fn iso_date_recognises_only_ten_char_dates() {
        assert!(is_iso_date("2026-09-27"));
        assert!(!is_iso_date("2026-09"));
        assert!(!is_iso_date("20260927"));
    }
}
