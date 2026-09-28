//! Prints a summary of what the app sees in the real OpenCode database.
//!
//! The fixture in `tests/aggregation.rs` pins the accounting rules, but it is
//! small and hand-built. This is the other half of the check: it runs the same
//! queries against whatever database this machine actually has, which is how
//! you find out a new OpenCode release moved a column before a user does.
//!
//! ```sh
//! cargo run --example live_probe
//! OPENCODE_DB_PATH=/path/to/opencode.db cargo run --example live_probe
//! cargo run --example live_probe -- 90
//! ```

use opencode_stats_win_lib::db;
use opencode_stats_win_lib::stats;

fn main() {
    let days = std::env::args()
        .nth(1)
        .and_then(|value| value.parse::<i64>().ok());

    let (database, info) = match db::open_primary() {
        Ok(pair) => pair,
        Err(err) => {
            eprintln!("could not open the database: {err}");
            std::process::exit(1);
        }
    };
    println!("path    {}", info.path);
    println!("size    {} bytes", info.size_bytes);
    println!("layout  {}", info.layout.as_deref().unwrap_or("unknown"));
    if !info.extra_databases.is_empty() {
        println!("others  {:?}", info.extra_databases);
    }

    let today = match stats::build_today(&database) {
        Ok(today) => today,
        Err(err) => {
            eprintln!("today query failed: {err}");
            std::process::exit(1);
        }
    };
    println!();
    println!("today   {} tokens, ${:.2}", today.tokens.total, today.cost);

    let snapshot = match stats::build_snapshot(&database, days) {
        Ok(snapshot) => snapshot,
        Err(err) => {
            eprintln!("snapshot failed: {err}");
            std::process::exit(1);
        }
    };
    let overview = &snapshot.overview;
    println!();
    println!("window  {:?}", snapshot.days.map(|d| format!("{d} days")).unwrap_or_else(|| "all time".into()));
    println!("sessions{} ", overview.session_count);
    println!("messages {} ({} assistant)", overview.message_count, overview.assistant_message_count);
    println!("span     {} days", overview.day_count);
    println!("cost     ${:.2} (${:.2}/day)", overview.total_cost, overview.avg_cost_per_day);
    println!("tokens   {} (in {} out {} reason {} read {} write {})",
        overview.tokens.total,
        overview.tokens.input,
        overview.tokens.output,
        overview.tokens.reasoning,
        overview.tokens.cache_read,
        overview.tokens.cache_write);
    println!("cache    {:.1}% hit rate", overview.cache_hit_rate);
    println!("session  avg {} median {}", overview.avg_tokens_per_session, overview.median_tokens_per_session);
    println!("days     {} buckets", snapshot.days_series.len());
    println!("heatmap  {} cells", snapshot.heatmap.len());
    println!("models   {}", snapshot.models.len());
    println!("provider {}", snapshot.providers.len());
    println!("projects {}", snapshot.projects.len());
    println!("tools    {}", snapshot.tools.len());
    println!("sessions {} rows", snapshot.sessions.len());
    println!("cache    {} miss of {} ({:.1}%), {} sessions never hit",
        snapshot.cache.miss, snapshot.cache.expected, snapshot.cache.miss_rate, snapshot.cache.no_cache_sessions);

    println!("\ntop models");
    for model in snapshot.models.iter().take(8) {
        println!("  {:<34} {:>7} msgs  {:>14} tokens  ${:>8.2}  [{}]",
            truncate(&model.name, 34),
            model.message_count,
            model.tokens.total,
            model.cost,
            model.providers.join(", "));
    }

    println!("\nprojects");
    for project in snapshot.projects.iter().take(8) {
        println!("  {:<34} {:>4} sessions  {:>14} tokens  ${:>8.2}",
            truncate(&project.name, 34),
            project.session_count,
            project.tokens.total,
            project.cost);
    }
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    let head: String = text.chars().take(width.saturating_sub(1)).collect();
    format!("{head}…")
}
