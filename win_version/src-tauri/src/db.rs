//! Locating and opening the OpenCode SQLite database.
//!
//! OpenCode keeps its data in a single SQLite file. On every platform (including
//! Windows) the default location is `~/.local/share/opencode/opencode.db`, and
//! the surrounding directory may hold more than one `opencode*.db` file, so we
//! merge every candidate we can find instead of assuming a single file.
//!
//! This module only ever opens the database **read-only**: we never write to it
//! and never create index objects on it.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rusqlite::types::ToSqlOutput;
use rusqlite::{params_from_iter, Connection, OpenFlags, ToSql};

// ---------------------------------------------------------------------------
// Bound parameters
// ---------------------------------------------------------------------------

/// A value bound to a `?` placeholder.
///
/// SQLite only ever sees a couple of kinds here, so a concrete enum is easier
/// to build and inspect than a boxed trait object.
#[derive(Debug, Clone, Copy)]
pub enum Param<'a> {
    Int(i64),
    Text(&'a str),
}

impl ToSql for Param<'_> {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        match *self {
            Param::Int(value) => Ok(ToSqlOutput::from(value)),
            Param::Text(value) => Ok(ToSqlOutput::from(value)),
        }
    }
}

/// The parameters one query will bind.
#[derive(Debug, Default, Clone)]
pub struct Bindings<'a> {
    values: Vec<Param<'a>>,
}

impl<'a> Bindings<'a> {
    pub fn new() -> Self {
        Bindings { values: Vec::new() }
    }

    pub fn text(&mut self, value: &'a str) {
        self.values.push(Param::Text(value));
    }

    /// rusqlite has no `Params` impl for an arbitrary slice, only for a
    /// reference to a trait-object slice, so parameters are converted here.
    pub fn params(&self) -> Vec<&dyn ToSql> {
        self.values.iter().map(|value| value as &dyn ToSql).collect()
    }

    pub fn as_params(&self) -> rusqlite::ParamsFromIter<Vec<&dyn ToSql>> {
        params_from_iter(self.params())
    }
}

impl<'a> FromIterator<Param<'a>> for Bindings<'a> {
    fn from_iter<I: IntoIterator<Item = Param<'a>>>(iter: I) -> Self {
        Bindings { values: iter.into_iter().collect() }
    }
}

/// The parameter list for a query that optionally filters on `:cutoff`.
///
/// A single prepared statement serves both the filtered and unfiltered forms,
/// which keeps the query text free of interpolation.
pub fn cutoff_params(cutoff: Option<i64>) -> Bindings<'static> {
    match cutoff {
        Some(value) => Bindings::from_iter([Param::Int(value)]),
        None => Bindings::new(),
    }
}

/// Which physical table layout the messages live in.
///
/// OpenCode has migrated its message storage twice. Newer versions write to
/// `session_message` with a dedicated `type` column; older ones keep a JSON
/// blob in `message.data` with the role inside it. Both are still seen in the
/// wild because a migration leaves the old table behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// `message.data` JSON blob, role at `$.role`.
    V1,
    /// `session_message` with a `type` column.
    V2,
}

impl Layout {
    pub fn as_str(self) -> &'static str {
        match self {
            Layout::V1 => "v1",
            Layout::V2 => "v2",
        }
    }
}

/// What the database looks like, cached per open connection.
#[derive(Debug, Clone)]
pub struct Schema {
    pub layout: Layout,
    pub has_session: bool,
    pub has_session_v2: bool,
    pub has_project: bool,
    pub has_part: bool,
}

impl Schema {
    /// `FROM`/`JOIN` fragment that exposes every message row as alias `m`,
    /// with its session reachable as `s`.
    ///
    /// V2 messages can be joined against either session table depending on
    /// which ones this database actually has, so the fragment is built per
    /// connection rather than being a constant.
    pub fn message_source(&self) -> String {
        match self.layout {
            Layout::V1 => "FROM message m JOIN session s ON s.id = m.session_id".to_string(),
            Layout::V2 => {
                if self.has_session && self.has_session_v2 {
                    "FROM session_message m \
                     LEFT JOIN session s ON s.id = m.session_id \
                     LEFT JOIN session_v2 s2 ON m.session_id = s2.id"
                        .to_string()
                } else if self.has_session_v2 {
                    "FROM session_message m LEFT JOIN session_v2 s ON s.id = m.session_id"
                        .to_string()
                } else {
                    "FROM session_message m LEFT JOIN session s ON s.id = m.session_id".to_string()
                }
            }
        }
    }

    /// SQL expression yielding `'assistant'` / `'user'` / ... for a row `m`.
    pub fn role_expr(&self) -> &'static str {
        match self.layout {
            Layout::V1 => "json_extract(m.data, '$.role')",
            Layout::V2 => "m.type",
        }
    }

    /// `JOIN` fragment that attaches a session's messages to a session row
    /// already in scope under the alias `s`.
    ///
    /// Used by aggregates that group by session or project, where the session
    /// side of the join is already established.
    pub fn message_join(&self) -> &'static str {
        match self.layout {
            Layout::V1 => "LEFT JOIN message m ON m.session_id = s.id",
            Layout::V2 => "LEFT JOIN session_message m ON m.session_id = s.id",
        }
    }

    /// Session id column for the current layout.
    pub fn session_id_expr(&self) -> &'static str {
        "m.session_id"
    }

    /// Directory a session was started in, used to filter out OpenCode's own
    /// internal sessions. Empty when the column is unavailable.
    pub fn directory_expr(&self) -> String {
        match self.layout {
            Layout::V1 => "s.directory".to_string(),
            Layout::V2 => {
                if self.has_session_v2 {
                    "COALESCE(s.directory, s2.directory)".to_string()
                } else {
                    "s.directory".to_string()
                }
            }
        }
    }
}

/// Details about the database we ended up reading, surfaced in the UI so a
/// misconfigured path is obvious instead of silently showing zero usage.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseInfo {
    pub path: String,
    pub exists: bool,
    pub size_bytes: u64,
    pub modified: Option<String>,
    pub data_dir: String,
    pub layout: Option<String>,
    pub extra_databases: Vec<String>,
}

/// Everything the aggregation layer needs in order to talk to a database.
pub struct Database {
    pub conn: Connection,
    pub schema: Schema,
    pub path: PathBuf,
}

impl Database {
    /// Open `path` read-only and work out its layout.
    pub fn open(path: &Path) -> Result<Database, String> {
        let conn = open_readonly(path)?;
        let schema = detect_schema(&conn)?;
        Ok(Database {
            conn,
            schema,
            path: path.to_path_buf(),
        })
    }
}

/// Open a SQLite connection that can never modify the file.
fn open_readonly(path: &Path) -> Result<Connection, String> {
    if !path.is_file() {
        return Err(format!("database not found: {}", path.display()));
    }
    // The URI form with `mode=ro` makes SQLite itself reject writes, which is
    // a stronger guarantee than relying on the open flags alone.
    let uri = format!("file:{}?mode=ro", uri_escape(&path.to_string_lossy()));
    let conn = Connection::open_with_flags(&uri, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("cannot open {}: {e}", path.display()))?;

    // A UI that reads a few hundred MB should not thrash the disk. These are
    // best-effort: SQLite accepts and ignores pragmas it cannot honour.
    let _ = conn.execute_batch(
        "PRAGMA temp_store = MEMORY;\
         PRAGMA cache_size = -16384;\
         PRAGMA mmap_size = 67108864;",
    );
    Ok(conn)
}

/// Percent-encode the characters that would otherwise terminate a SQLite URI
/// path segment. Windows paths are full of backslashes and drive letters, and
/// a `#` or `?` in a user directory would silently truncate the path.
fn uri_escape(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for ch in path.chars() {
        match ch {
            '?' => out.push_str("%3f"),
            '#' => out.push_str("%23"),
            '%' => out.push_str("%25"),
            c => out.push(c),
        }
    }
    out
}

fn table_exists(conn: &Connection, name: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [name],
        |row| row.get::<_, i64>(0),
    )
    .map(|found| found != 0)
    .map_err(|e| format!("introspection failed: {e}"))
}

fn table_row_count(conn: &Connection, table: &str) -> i64 {
    // `table` comes from our own constant list, never from user input, so
    // interpolating it is safe; a binding cannot be used for an identifier.
    let sql = format!("SELECT COUNT(*) FROM {table}");
    conn.query_row(&sql, [], |row| row.get::<_, i64>(0))
        .unwrap_or(0)
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
    conn.query_row(&format!("SELECT 1 FROM pragma_table_info(?1) WHERE name = ?2"), [table, column], |r| {
        r.get::<_, i64>(0)
    })
    .map(|found| found != 0)
    .unwrap_or(false)
}

fn detect_schema(conn: &Connection) -> Result<Schema, String> {
    let has_message = table_exists(conn, "message")?;
    let has_session_message = table_exists(conn, "session_message")?;
    let has_session = table_exists(conn, "session")?;
    let has_session_v2 = table_exists(conn, "session_v2")?;
    let has_project = table_exists(conn, "project")?;
    let has_part = table_exists(conn, "part")?;

    if !has_message && !has_session_message {
        return Err(
            "no message or session_message table found - is this an OpenCode database?".to_string(),
        );
    }

    // After a migration both tables can be present, with the old one left in
    // place holding stale rows. The table with more rows is the live one.
    let layout = match (has_message, has_session_message) {
        (true, false) => Layout::V1,
        (false, true) => Layout::V2,
        (true, true) => {
            let v1 = table_row_count(conn, "message");
            let v2 = table_row_count(conn, "session_message");
            if v2 > v1 {
                Layout::V2
            } else {
                Layout::V1
            }
        }
        (false, false) => unreachable!("guarded above"),
    };

    // A `message` table without a `data` column would mean we picked up an
    // unrelated schema; fall back to the other layout rather than emitting
    // broken SQL.
    let layout = match layout {
        Layout::V1 if !column_exists(conn, "message", "data") => Layout::V2,
        other => other,
    };

    Ok(Schema {
        layout,
        has_session,
        has_session_v2,
        has_project,
        has_part,
    })
}

/// Home directory for the current user, honouring the environment variables
/// Windows actually sets.
pub fn home_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(profile) = env_non_empty("USERPROFILE") {
            return PathBuf::from(profile);
        }
        // Fall back to the drive-relative home if USERPROFILE is missing.
        if let Some(home_drive) = env_non_empty("HOMEDRIVE") {
            if let Some(home_path) = env_non_empty("HOMEPATH") {
                return PathBuf::from(format!("{home_drive}{home_path}"));
            }
        }
    }
    env_non_empty("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn env_non_empty(key: &str) -> Option<String> {
    match std::env::var(key) {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => None,
    }
}

/// Directories that may contain an `opencode.db`, in priority order.
///
/// Explicit environment variables win over the well-known defaults so a user
/// with an unusual layout can point the app at the right place.
fn candidate_data_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut push = |dir: PathBuf| {
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    };

    if let Some(dir) = env_non_empty("OPENCODE_DATA_DIR") {
        push(PathBuf::from(dir));
    }
    if let Some(dir) = env_non_empty("XDG_DATA_HOME") {
        push(PathBuf::from(dir).join("opencode"));
    }
    // Windows: alongside the other per-user application data.
    if let Some(dir) = env_non_empty("LOCALAPPDATA") {
        push(PathBuf::from(dir).join("opencode"));
    }
    if let Some(dir) = env_non_empty("APPDATA") {
        push(PathBuf::from(dir).join("opencode"));
    }
    // The layout OpenCode actually uses on every platform.
    push(home_dir().join(".local").join("share").join("opencode"));
    push(home_dir().join(".opencode"));

    dirs
}

/// Find every OpenCode database we can read, most recently active first.
pub fn discover_databases() -> Result<Vec<PathBuf>, String> {
    // A fully qualified path short-circuits discovery entirely.
    if let Some(explicit) = env_non_empty("OPENCODE_DB_PATH") {
        let path = PathBuf::from(explicit);
        if path.is_file() {
            return Ok(vec![path]);
        }
        return Err(format!("OPENCODE_DB_PATH does not point at a file: {}", path.display()));
    }

    let mut found: Vec<PathBuf> = Vec::new();
    let mut first_existing_dir: Option<PathBuf> = None;

    for dir in candidate_data_dirs() {
        if first_existing_dir.is_none() && dir.is_dir() {
            first_existing_dir = Some(dir.clone());
        }
        if !dir.is_dir() {
            continue;
        }
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name,
                None => continue,
            };
            // Skip auth stores - they hold credentials, not usage data - and
            // any other file that is not a database.
            if !name.starts_with("opencode") || name.starts_with("opencode-auth") {
                continue;
            }
            if !name.to_ascii_lowercase().ends_with(".db") {
                continue;
            }
            // A zero-byte file is a freshly created placeholder.
            if fs::metadata(&path).map(|m| m.len() == 0).unwrap_or(true) {
                continue;
            }
            if !found.contains(&path) {
                found.push(path);
            }
        }
    }

    if found.is_empty() {
        // Report the directory we expected so the error is actionable.
        let expected = first_existing_dir.unwrap_or_else(|| {
            home_dir().join(".local").join("share").join("opencode")
        });
        return Err(format!(
            "no opencode.db found - looked in {} (set OPENCODE_DB_PATH to override)",
            expected.display()
        ));
    }

    // Most recently written wins: the -wal sidecar is what actually changes
    // when a session is active, so it counts towards the activity score.
    found.sort_by(|a, b| {
        activity(b)
            .cmp(&activity(a))
            .then_with(|| a.file_name().cmp(&b.file_name()))
    });
    Ok(found)
}

/// Modification time of the database or its write-ahead log, whichever moved
/// most recently. `SystemTime` is used directly so we never need an extra
/// date-formatting dependency here.
pub fn activity(path: &Path) -> SystemTime {
    let mut newest = fs::metadata(path).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
    if let Ok(meta) = fs::metadata(path.with_extension("db-wal")) {
        if let Ok(modified) = meta.modified() {
            if modified > newest {
                newest = modified;
            }
        }
    }
    newest
}

/// Open the primary database, reporting details for the UI.
pub fn open_primary() -> Result<(Database, DatabaseInfo), String> {
    let paths = discover_databases()?;
    let primary = &paths[0];
    let db = Database::open(primary)?;
    let data_dir = primary
        .parent()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let meta = fs::metadata(primary);
    let info = DatabaseInfo {
        path: primary.display().to_string(),
        exists: true,
        size_bytes: meta.as_ref().map(|m| m.len()).unwrap_or(0),
        modified: meta
            .as_ref()
            .ok()
            .and_then(|m| m.modified().ok())
            .map(format_timestamp),
        data_dir,
        layout: Some(db.schema.layout.as_str().to_string()),
        extra_databases: paths
            .iter()
            .skip(1)
            .map(|p| p.display().to_string())
            .collect(),
    };
    Ok((db, info))
}

/// Describe the database without opening it, so the UI can report a clear
/// "not found" state instead of an opaque error.
pub fn probe() -> DatabaseInfo {
    match discover_databases() {
        Ok(paths) => {
            let primary = &paths[0];
            let meta = fs::metadata(primary);
            DatabaseInfo {
                path: primary.display().to_string(),
                exists: true,
                size_bytes: meta.as_ref().map(|m| m.len()).unwrap_or(0),
                modified: meta
                    .as_ref()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(format_timestamp),
                data_dir: primary
                    .parent()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
                layout: None,
                extra_databases: paths
                    .iter()
                    .skip(1)
                    .map(|p| p.display().to_string())
                    .collect(),
            }
        }
        Err(err) => DatabaseInfo {
            path: err,
            exists: false,
            size_bytes: 0,
            modified: None,
            data_dir: home_dir()
                .join(".local")
                .join("share")
                .join("opencode")
                .display()
                .to_string(),
            layout: None,
            extra_databases: Vec::new(),
        },
    }
}

fn format_timestamp(time: SystemTime) -> String {
    let secs = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_db() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn detects_v1_layout() {
        let conn = memory_db();
        conn.execute_batch(
            "CREATE TABLE message (id TEXT PRIMARY KEY, data TEXT, time_created INTEGER);
             CREATE TABLE session (id TEXT PRIMARY KEY, directory TEXT);",
        )
        .unwrap();
        let schema = detect_schema(&conn).unwrap();
        assert_eq!(schema.layout, Layout::V1);
        assert!(schema.message_source().contains("FROM message m"));
        assert_eq!(schema.role_expr(), "json_extract(m.data, '$.role')");
    }

    #[test]
    fn detects_v2_layout() {
        let conn = memory_db();
        conn.execute_batch(
            "CREATE TABLE session_message (id TEXT PRIMARY KEY, session_id TEXT, type TEXT, data TEXT);
             CREATE TABLE session (id TEXT PRIMARY KEY, directory TEXT);",
        )
        .unwrap();
        let schema = detect_schema(&conn).unwrap();
        assert_eq!(schema.layout, Layout::V2);
        assert_eq!(schema.role_expr(), "m.type");
        assert!(schema.message_source().contains("FROM session_message m"));
    }

    #[test]
    fn prefers_the_populated_table_after_a_migration() {
        let conn = memory_db();
        conn.execute_batch(
            "CREATE TABLE message (id TEXT PRIMARY KEY, data TEXT);
             CREATE TABLE session_message (id TEXT PRIMARY KEY, type TEXT);
             CREATE TABLE session (id TEXT PRIMARY KEY, directory TEXT);
             INSERT INTO message (id, data) VALUES ('a', '{}'), ('b', '{}');
             INSERT INTO session_message (id, type) VALUES ('a', 'assistant');",
        )
        .unwrap();
        // v1 has 2 rows, v2 has 1 - the stale table wins only by accident here.
        assert_eq!(detect_schema(&conn).unwrap().layout, Layout::V1);

        let conn2 = memory_db();
        conn2.execute_batch(
            "CREATE TABLE message (id TEXT PRIMARY KEY, data TEXT);
             CREATE TABLE session_message (id TEXT PRIMARY KEY, type TEXT);
             CREATE TABLE session (id TEXT PRIMARY KEY, directory TEXT);
             INSERT INTO message (id, data) VALUES ('a', '{}');
             INSERT INTO session_message (id, type) VALUES ('a', 'assistant'), ('b', 'assistant');",
        )
        .unwrap();
        assert_eq!(detect_schema(&conn2).unwrap().layout, Layout::V2);
    }

    #[test]
    fn rejects_a_database_without_messages() {
        let conn = memory_db();
        conn.execute_batch("CREATE TABLE unrelated (a INTEGER);").unwrap();
        assert!(detect_schema(&conn).is_err());
    }

    #[test]
    fn ignores_a_message_table_that_has_no_data_column() {
        let conn = memory_db();
        conn.execute_batch(
            "CREATE TABLE message (id TEXT PRIMARY KEY, time_created INTEGER);
             CREATE TABLE session_message (id TEXT PRIMARY KEY, type TEXT);
             CREATE TABLE session (id TEXT PRIMARY KEY, directory TEXT);",
        )
        .unwrap();
        assert_eq!(detect_schema(&conn).unwrap().layout, Layout::V2);
    }

    #[test]
    fn uri_escape_protects_windows_and_reserved_characters() {
        assert_eq!(uri_escape(r"C:\Users\a b\opencode.db"), r"C:\Users\a b\opencode.db");
        assert_eq!(uri_escape("/tmp/a#b?c.db"), "/tmp/a%23b%3fc.db");
        assert_eq!(uri_escape("/tmp/100%.db"), "/tmp/100%25.db");
    }

    #[test]
    fn read_only_connection_rejects_writes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opencode.db");
        {
            let setup = Connection::open(&path).unwrap();
            setup
                .execute_batch("CREATE TABLE t (a INTEGER); INSERT INTO t VALUES (1);")
                .unwrap();
        }
        let conn = open_readonly(&path).unwrap();
        assert!(conn.execute("INSERT INTO t VALUES (2)", []).is_err());
        // Reads still work.
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
