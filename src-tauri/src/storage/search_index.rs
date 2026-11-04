use rusqlite::{Connection, Result};

/// Content table and column names are selected from this closed set so no SQL
/// identifier comes from user input.
pub enum SearchIndex {
    History,
    Clipboard,
    Scratchpad,
}

pub fn ensure_search_index(conn: &Connection, index: SearchIndex) -> Result<()> {
    let (version, fts_table, content_table, content_column) = match index {
        SearchIndex::History => (1, "dictation_history_fts", "dictation_history", "text"),
        SearchIndex::Clipboard => (1, "clipboard_history_fts", "clipboard_history", "content"),
        SearchIndex::Scratchpad => (1, "scratchpad_draft_fts", "scratchpad_draft", "content"),
    };

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS rift_search_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );",
    )?;

    let fts_ddl = format!(
        "CREATE VIRTUAL TABLE IF NOT EXISTS {fts_table}
         USING fts5({content_column}, content='{content_table}', content_rowid='id', tokenize='trigram');

         CREATE TRIGGER IF NOT EXISTS {fts_table}_insert AFTER INSERT ON {content_table} BEGIN
             INSERT INTO {fts_table}(rowid, {content_column}) VALUES (new.id, new.{content_column});
         END;

         CREATE TRIGGER IF NOT EXISTS {fts_table}_delete AFTER DELETE ON {content_table} BEGIN
             INSERT INTO {fts_table}({fts_table}, rowid, {content_column})
             VALUES ('delete', old.id, old.{content_column});
         END;

         CREATE TRIGGER IF NOT EXISTS {fts_table}_update AFTER UPDATE OF {content_column} ON {content_table} BEGIN
             INSERT INTO {fts_table}({fts_table}, rowid, {content_column})
             VALUES ('delete', old.id, old.{content_column});
             INSERT INTO {fts_table}(rowid, {content_column}) VALUES (new.id, new.{content_column});
         END;"
    );
    conn.execute_batch(&fts_ddl)?;

    let already_migrated: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM rift_search_migrations WHERE version = ?1)",
        [version],
        |row| row.get(0),
    )?;
    if !already_migrated {
        conn.execute(
            &format!("INSERT INTO {fts_table}({fts_table}) VALUES ('rebuild')"),
            [],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO rift_search_migrations(version) VALUES (?1)",
            [version],
        )?;
    }

    Ok(())
}
