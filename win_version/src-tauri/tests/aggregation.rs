//! End-to-end checks for the aggregation SQL.
//!
//! These build small databases in the same shape OpenCode writes, then assert
//! on the numbers `build_snapshot` produces. Unit tests cover the arithmetic;
//! these cover the queries, which is where column ordering and JSON paths go
//! wrong.

use opencode_stats_win_lib::db::{self, Database};
use opencode_stats_win_lib::stats;

/// A fixed instant, so the fixture is reproducible. Well in the past and
/// clearly local-day-agnostic, which keeps the day-bucketing assertions
/// meaningful without depending on the clock.
const DAY: i64 = 24 * 60 * 60 * 1000;
const BASE: i64 = 1_700_000_000_000;

/// V1 layout, matching the tables older OpenCode versions create.
///
/// Two top-level sessions a day apart, a subagent under the first, one warm
/// cache pair, and a user turn that must never be counted.
fn v1_database() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("opencode.db");
    let conn = rusqlite::Connection::open(&path).expect("create fixture");
    conn.execute_batch(
        "CREATE TABLE project (id TEXT PRIMARY KEY, worktree TEXT NOT NULL, name TEXT);\
         CREATE TABLE session (\
           id TEXT PRIMARY KEY, project_id TEXT NOT NULL, parent_id TEXT,\
           slug TEXT NOT NULL, directory TEXT NOT NULL, title TEXT NOT NULL,\
           time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL);\
         CREATE TABLE message (\
           id TEXT PRIMARY KEY, session_id TEXT NOT NULL,\
           time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL, data TEXT NOT NULL);\
         CREATE TABLE part (\
           id TEXT PRIMARY KEY, message_id TEXT NOT NULL, session_id TEXT NOT NULL,\
           time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL, data TEXT NOT NULL);",
    )
    .expect("schema");

    // p1 has no declared name, so its label has to come from the worktree.
    conn.execute("INSERT INTO project VALUES ('p1', '/home/dev/alpha', NULL)", [])
        .expect("project p1");
    conn.execute(
        "INSERT INTO project VALUES ('p2', '/home/dev/beta', 'Beta Project')",
        [],
    )
    .expect("project p2");

    let session = |id: &str, project: &str, parent: Option<&str>, title: &str, at: i64| {
        conn.execute(
            "INSERT INTO session VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
            rusqlite::params![id, project, parent, format!("slug-{id}"), format!("/home/dev/{}", project), title, at],
        )
        .expect("session");
    };
    session("s1", "p1", None, "First", BASE);
    session("s1b", "p1", Some("s1"), "Sub", BASE + 1_000);
    session("s2", "p2", None, "Second", BASE + DAY);

    let message = |id: &str, session: &str, at: i64, data: &str| {
        conn.execute(
            "INSERT INTO message VALUES (?1, ?2, ?3, ?3, ?4)",
            rusqlite::params![id, session, at, data],
        )
        .expect("message");
    };
    // A warm turn: a large cache read is what makes the next turn pairable.
    message("m1", "s1", BASE + 2_000, r#"{"role":"assistant","modelID":"sonnet","providerID":"anthropic","cost":0.25,"tokens":{"input":10,"output":20,"reasoning":5,"cache":{"read":1000,"write":0}}}"#);
    // The successor reads 635 of the previous 1035 tokens back, so those 400
    // are the miss.
    message("m2", "s1", BASE + 3_000, r#"{"role":"assistant","modelID":"sonnet","providerID":"anthropic","cost":0.75,"tokens":{"input":5,"output":5,"reasoning":0,"cache":{"read":635,"write":0}}}"#);
    // The same model id through a different provider, so the model merge has
    // to keep both names. Cold cache, so it opens no pair.
    message("m3", "s1b", BASE + 4_000, r#"{"role":"assistant","modelID":"sonnet","providerID":"bedrock","cost":0.10,"tokens":{"input":1,"output":1,"reasoning":1,"cache":{"read":0,"write":0}}}"#);
    // A user turn must never contribute cost or tokens.
    message("m4", "s1", BASE + 2_500, r#"{"role":"user","tokens":{"input":9999,"output":9999}}"#);
    // `"total": -1` is what OpenCode writes when it has not resolved a total;
    // the five counters have to be summed instead of trusted.
    message("m5", "s2", BASE + DAY + 1_000, r#"{"role":"assistant","modelID":"sonnet","providerID":"anthropic","cost":1.5,"tokens":{"input":200,"output":100,"reasoning":50,"cache":{"read":400,"write":100},"total":-1}}"#);

    let part = |id: &str, message: &str, at: i64, data: &str| {
        conn.execute(
            "INSERT INTO part VALUES (?1, ?2, ?3, ?4, ?4, ?5)",
            rusqlite::params![id, message, "s1", at, data],
        )
        .expect("part");
    };
    part("pt1", "m1", BASE + 2_050, r#"{"type":"tool","tool":"bash"}"#);
    part("pt2", "m1", BASE + 2_060, r#"{"type":"tool","tool":"bash"}"#);
    part("pt3", "m2", BASE + 3_050, r#"{"type":"tool","tool":"read"}"#);
    part("pt4", "m1", BASE + 2_070, r#"{"type":"text","text":"hi"}"#);

    drop(conn);
    (dir, path)
}

fn open(path: &std::path::Path) -> Database {
    db::Database::open(path).expect("open fixture")
}

#[test]
fn totals_ignore_user_turns_and_fold_subagents_into_their_parent() {
    let (_dir, path) = v1_database();
    let snapshot = stats::build_snapshot(&open(&path), None).expect("snapshot");

    // s1: m1 (10+20+5+1000) + m2 (5+5+0+635) + subagent m3 (1+1+1+0).
    // The user turn m4 is excluded, and s2 belongs to the all-time window too.
    let s1: i64 = 10 + 20 + 5 + 1000 + 5 + 5 + 635 + 1 + 1 + 1;
    let s2: i64 = 200 + 100 + 50 + 400 + 100;
    assert_eq!(
        snapshot.overview.tokens.total,
        s1 + s2,
        "user tokens must not be counted"
    );
    assert_eq!(snapshot.overview.total_cost, 0.25 + 0.75 + 0.10 + 1.5);
    assert_eq!(snapshot.overview.session_count, 2, "only top-level sessions");
    assert_eq!(snapshot.overview.assistant_message_count, 4);
    assert_eq!(snapshot.overview.message_count, 5, "user turns included here");
}

#[test]
fn a_declared_token_total_of_minus_one_is_recomputed() {
    let (_dir, path) = v1_database();
    let snapshot = stats::build_snapshot(&open(&path), None).expect("snapshot");
    // m5 carries `"total": -1`; the five counters are authoritative.
    let m5 = 200 + 100 + 50 + 400 + 100;
    let s1 = 10 + 20 + 5 + 1000 + 5 + 5 + 635 + 1 + 1 + 1;
    assert_eq!(snapshot.overview.tokens.total, s1 + m5);
}

#[test]
fn models_merge_across_providers_and_keep_both_names() {
    let (_dir, path) = v1_database();
    let snapshot = stats::build_snapshot(&open(&path), None).expect("snapshot");

    assert_eq!(snapshot.models.len(), 1, "one model id, two providers");
    let model = &snapshot.models[0];
    assert_eq!(model.name, "sonnet");
    assert_eq!(model.providers, vec!["anthropic", "bedrock"], "sorted");
    assert_eq!(model.message_count, 4);
    assert_eq!(model.cost, 0.25 + 0.75 + 0.10 + 1.5);
}

#[test]
fn providers_stay_separate_and_are_ranked_by_tokens() {
    let (_dir, path) = v1_database();
    let snapshot = stats::build_snapshot(&open(&path), None).expect("snapshot");

    let names: Vec<&str> = snapshot.providers.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["anthropic", "bedrock"]);
    assert_eq!(snapshot.providers[0].tokens.total, 1035 + 645 + 850);
    assert_eq!(snapshot.providers[1].tokens.total, 3);
}

#[test]
fn project_names_fall_back_to_the_path_and_subagents_are_not_extra_sessions() {
    let (_dir, path) = v1_database();
    let snapshot = stats::build_snapshot(&open(&path), None).expect("snapshot");

    let alpha = snapshot
        .projects
        .iter()
        .find(|p| p.id == "p1")
        .expect("alpha project");
    assert_eq!(alpha.name, "alpha", "name derived from the worktree");
    assert_eq!(alpha.path, "/home/dev/alpha");
    assert_eq!(alpha.session_count, 1, "the subagent is not a second session");
    assert_eq!(alpha.message_count, 4, "subagent messages still belong here");
    assert_eq!(alpha.latest_session_title.as_deref(), Some("First"));

    let beta = snapshot
        .projects
        .iter()
        .find(|p| p.id == "p2")
        .expect("beta project");
    assert_eq!(beta.name, "Beta Project", "a declared name wins");
}

#[test]
fn tool_counts_ignore_parts_that_are_not_tool_calls() {
    let (_dir, path) = v1_database();
    let snapshot = stats::build_snapshot(&open(&path), None).expect("snapshot");

    let bash = snapshot.tools.iter().find(|t| t.name == "bash").expect("bash");
    let read = snapshot.tools.iter().find(|t| t.name == "read").expect("read");
    assert_eq!(bash.count, 2);
    assert_eq!(read.count, 1);
    assert_eq!(snapshot.tools.len(), 2, "the text part is not a tool");
    // Three tool calls total, so bash is 2/3.
    assert!((bash.percentage - 66.7).abs() < 0.05);
}

#[test]
fn cache_misses_are_attributed_to_the_successor() {
    let (_dir, path) = v1_database();
    let snapshot = stats::build_snapshot(&open(&path), None).expect("snapshot");

    // m1 -> m2 is the only warm pair. m1's total is 1035 and m2 read 635 back,
    // so 400 tokens were re-sent. m3 is cold (cache_read 0) so nothing pairs.
    assert_eq!(snapshot.cache.expected, 1035);
    assert_eq!(snapshot.cache.miss, 400);
    assert!((snapshot.cache.miss_rate - 38.6).abs() < 0.1);

    let paired_days: Vec<_> = snapshot
        .days_series
        .iter()
        .filter(|d| d.cache_expected > 0)
        .collect();
    assert_eq!(paired_days.len(), 1, "exactly one day carries the pair");
    assert_eq!(paired_days[0].cache_miss, 400);
}

#[test]
fn sessions_without_any_cache_read_are_counted() {
    let (_dir, path) = v1_database();
    let snapshot = stats::build_snapshot(&open(&path), None).expect("snapshot");
    // s1b (the subagent) read nothing at all. It is counted on its own
    // because the query groups by literal session id.
    assert_eq!(snapshot.cache.no_cache_sessions, 1);
}

#[test]
fn recent_sessions_use_the_last_assistant_model_and_roll_up_subagents() {
    let (_dir, path) = v1_database();
    let snapshot = stats::build_snapshot(&open(&path), None).expect("snapshot");

    assert_eq!(snapshot.sessions.len(), 2);
    let first = snapshot
        .sessions
        .iter()
        .find(|s| s.id == "s1")
        .expect("first session");
    // The subagent's cost is folded in so the list agrees with the totals, but
    // the model shown is the one used by the parent's own last assistant turn.
    assert_eq!(first.title, "First");
    assert_eq!(first.project_name, "alpha");
    assert_eq!(first.model, "sonnet");
    assert_eq!(first.provider, "anthropic");
    assert_eq!(first.cost, 0.25 + 0.75 + 0.10, "the subagent rolls up");
    assert_eq!(first.message_count, 3, "two assistant turns plus the user turn");
}

#[test]
fn a_day_filter_excludes_sessions_created_before_it() {
    let (_dir, path) = v1_database();
    let database = open(&path);
    // A one-day window cannot reach the epoch-date sessions in the fixture, so
    // every aggregate comes back empty rather than wrong.
    let snapshot = stats::build_snapshot(&database, Some(1)).expect("snapshot");
    assert_eq!(snapshot.overview.session_count, 0);
    assert_eq!(snapshot.overview.tokens.total, 0);
    assert_eq!(snapshot.overview.total_cost, 0.0);
    assert!(snapshot.models.is_empty());
    assert!(snapshot.projects.is_empty());
}

#[test]
fn an_all_time_window_keeps_everything() {
    let (_dir, path) = v1_database();
    let snapshot = stats::build_snapshot(&open(&path), None).expect("snapshot");
    assert_eq!(snapshot.overview.session_count, 2);
    assert_eq!(snapshot.days_series.len(), 2, "one row per day that has data");
}
