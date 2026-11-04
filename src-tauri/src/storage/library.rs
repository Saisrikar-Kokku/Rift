use chrono::Local;
use rusqlite::{params, Connection, OptionalExtension, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::search_index::{ensure_search_index, SearchIndex};
use super::settings::get_app_data_dir;

const SEARCH_ITEMS_SQL: &str = r#"
    SELECT
        'transcript' AS kind,
        CAST(h.id AS TEXT) AS source_id,
        h.timestamp AS timestamp,
        h.target_app AS source_app,
        NULL AS category,
        h.text AS content
    FROM history_db.dictation_history h
    WHERE COALESCE(h.target_app, '') <> 'Rift Scratchpad'

    UNION ALL

    SELECT
        'note' AS kind,
        'legacy:' || CAST(h.id AS TEXT) AS source_id,
        h.timestamp AS timestamp,
        'Scratchpad' AS source_app,
        'note' AS category,
        substr(h.text, length('[Scratchpad Note] ') + 1) AS content
    FROM history_db.dictation_history h
    WHERE h.target_app = 'Rift Scratchpad'
      AND h.text LIKE '[Scratchpad Note] %'
      AND NOT EXISTS (
          SELECT 1
          FROM scratchpad_db.scratchpad_draft d
          WHERE d.id = 1
            AND instr(d.content, substr(h.text, length('[Scratchpad Note] ') + 1)) > 0
      )

    UNION ALL

    SELECT
        'clipboard' AS kind,
        CAST(c.id AS TEXT) AS source_id,
        c.timestamp AS timestamp,
        c.source_app AS source_app,
        c.category AS category,
        c.content AS content
    FROM clipboard_db.clipboard_history c

    UNION ALL

    SELECT
        'note' AS kind,
        'draft' AS source_id,
        d.updated_at AS timestamp,
        'Scratchpad' AS source_app,
        'draft' AS category,
        d.content AS content
    FROM scratchpad_db.scratchpad_draft d
    WHERE d.id = 1 AND length(trim(d.content)) > 0
"#;

const SEARCH_ITEMS_FTS_SQL: &str = r#"
    SELECT
        'transcript' AS kind,
        CAST(h.id AS TEXT) AS source_id,
        h.timestamp AS timestamp,
        h.target_app AS source_app,
        NULL AS category,
        h.text AS content
    FROM history_db.dictation_history h
    WHERE COALESCE(h.target_app, '') <> 'Rift Scratchpad'
      AND h.id IN (
          SELECT rowid FROM history_db.dictation_history_fts WHERE text MATCH ?1
      )

    UNION ALL

    SELECT
        'note' AS kind,
        'legacy:' || CAST(h.id AS TEXT) AS source_id,
        h.timestamp AS timestamp,
        'Scratchpad' AS source_app,
        'note' AS category,
        substr(h.text, length('[Scratchpad Note] ') + 1) AS content
    FROM history_db.dictation_history h
    WHERE h.target_app = 'Rift Scratchpad'
      AND h.text LIKE '[Scratchpad Note] %'
      AND h.id IN (
          SELECT rowid FROM history_db.dictation_history_fts WHERE text MATCH ?1
      )
      AND instr(lower(substr(h.text, length('[Scratchpad Note] ') + 1)), lower(?2)) > 0
      AND NOT EXISTS (
          SELECT 1
          FROM scratchpad_db.scratchpad_draft d
          WHERE d.id = 1
            AND instr(d.content, substr(h.text, length('[Scratchpad Note] ') + 1)) > 0
      )

    UNION ALL

    SELECT
        'clipboard' AS kind,
        CAST(c.id AS TEXT) AS source_id,
        c.timestamp AS timestamp,
        c.source_app AS source_app,
        c.category AS category,
        c.content AS content
    FROM clipboard_db.clipboard_history c
    WHERE c.id IN (
        SELECT rowid FROM clipboard_db.clipboard_history_fts WHERE content MATCH ?1
    )

    UNION ALL

    SELECT
        'note' AS kind,
        'draft' AS source_id,
        d.updated_at AS timestamp,
        'Scratchpad' AS source_app,
        'draft' AS category,
        d.content AS content
    FROM scratchpad_db.scratchpad_draft d
    WHERE d.id = 1
      AND length(trim(d.content)) > 0
      AND d.id IN (
          SELECT rowid FROM scratchpad_db.scratchpad_draft_fts WHERE content MATCH ?1
      )
"#;

#[derive(Debug, Clone, Serialize)]
pub struct LibraryItem {
    pub id: String,
    pub kind: String,
    pub timestamp: String,
    pub source_app: Option<String>,
    pub category: Option<String>,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LibrarySearchPage {
    pub results: Vec<LibraryItem>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScratchpadDraft {
    pub content: String,
    pub updated_at: Option<String>,
}

#[derive(Clone)]
pub struct LibraryStore {
    scratchpad_db_path: PathBuf,
}

impl LibraryStore {
    pub fn new() -> Result<Self> {
        let scratchpad_db_path = get_app_data_dir().join("scratchpad.db");
        let conn = Connection::open(&scratchpad_db_path)?;
        conn.busy_timeout(Duration::from_secs(2))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS scratchpad_draft (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                content TEXT NOT NULL DEFAULT '',
                updated_at TEXT NOT NULL
            );",
        )?;

        ensure_search_index(&conn, SearchIndex::Scratchpad)?;

        Ok(Self { scratchpad_db_path })
    }

    pub fn get_draft(&self) -> Result<ScratchpadDraft> {
        let conn = self.open_scratchpad_connection()?;
        conn.query_row(
            "SELECT content, updated_at FROM scratchpad_draft WHERE id = 1",
            [],
            |row| {
                Ok(ScratchpadDraft {
                    content: row.get(0)?,
                    updated_at: Some(row.get(1)?),
                })
            },
        )
        .optional()
        .map(|draft| {
            draft.unwrap_or(ScratchpadDraft {
                content: String::new(),
                updated_at: None,
            })
        })
    }

    pub fn save_draft(&self, content: &str) -> Result<String> {
        let conn = self.open_scratchpad_connection()?;
        let updated_at = chrono::Utc::now().to_rfc3339();
        let persisted_updated_at: String = conn.query_row(
            "INSERT INTO scratchpad_draft (id, content, updated_at)
             VALUES (1, ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET
                 content = excluded.content,
                 updated_at = CASE
                     WHEN scratchpad_draft.content IS NOT excluded.content THEN excluded.updated_at
                     ELSE scratchpad_draft.updated_at
                 END
             RETURNING updated_at",
            params![content, updated_at],
            |row| row.get(0),
        )?;
        Ok(persisted_updated_at)
    }

    pub fn append_voice_note(&self, note: &str) -> Result<()> {
        let note = note.trim();
        if note.is_empty() {
            return Ok(());
        }

        let time = Local::now().format("%H:%M");
        let entry = format!("• [{time}] {note}");
        let updated_at = chrono::Utc::now().to_rfc3339();
        let conn = self.open_scratchpad_connection()?;
        conn.execute(
            "INSERT INTO scratchpad_draft (id, content, updated_at)
             VALUES (1, ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET
                 content = CASE
                     WHEN length(trim(scratchpad_draft.content)) = 0 THEN excluded.content
                     WHEN substr(scratchpad_draft.content, -1, 1) = char(10)
                         THEN scratchpad_draft.content || excluded.content
                     ELSE scratchpad_draft.content || char(10) || excluded.content
                 END,
                 updated_at = excluded.updated_at",
            params![entry, updated_at],
        )?;
        Ok(())
    }

    pub fn search(
        &self,
        query: &str,
        kind: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<LibrarySearchPage> {
        if let Some(kind) = kind {
            if !matches!(kind, "transcript" | "clipboard" | "note") {
                return Err(rusqlite::Error::InvalidParameterName(
                    "kind must be transcript, clipboard, or note".to_string(),
                ));
            }
        }

        let query: String = query.trim().chars().take(256).collect();
        let escaped_query = query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{escaped_query}%");
        let limit = limit.clamp(1, 100);
        let offset = offset.min(1_000_000);
        let sql_limit = i64::try_from(limit).unwrap_or(100);
        let sql_offset = i64::try_from(offset).unwrap_or(i64::MAX);

        let conn = self.open_search_connection()?;
        let (count_sql, results_sql, mut params): (String, String, Vec<rusqlite::types::Value>) =
            if query.is_empty() {
                (
                    format!("SELECT COUNT(*) FROM ({SEARCH_ITEMS_SQL}) items WHERE (?1 IS NULL OR items.kind = ?1)"),
                    format!("SELECT kind, source_id, timestamp, source_app, category, {} AS excerpt FROM ({SEARCH_ITEMS_SQL}) items WHERE (?1 IS NULL OR items.kind = ?1) ORDER BY julianday(items.timestamp) DESC, items.kind ASC, items.source_id DESC LIMIT ?2 OFFSET ?3", excerpt_expression(4)),
                    vec![kind.map(str::to_owned).map_or(rusqlite::types::Value::Null, rusqlite::types::Value::Text)],
                )
            } else if query.chars().count() >= 3 {
                let phrase = format!("\"{}\"", query.replace('"', "\"\""));
                (
                    format!("SELECT COUNT(*) FROM ({SEARCH_ITEMS_FTS_SQL}) items WHERE (?3 IS NULL OR items.kind = ?3)"),
                    format!("SELECT kind, source_id, timestamp, source_app, category, {} AS excerpt FROM ({SEARCH_ITEMS_FTS_SQL}) items WHERE (?3 IS NULL OR items.kind = ?3) ORDER BY julianday(items.timestamp) DESC, items.kind ASC, items.source_id DESC LIMIT ?4 OFFSET ?5", excerpt_expression(6)),
                    vec![
                        rusqlite::types::Value::Text(phrase),
                        rusqlite::types::Value::Text(query.clone()),
                        kind.map(str::to_owned).map_or(rusqlite::types::Value::Null, rusqlite::types::Value::Text),
                    ],
                )
            } else {
                (
                    format!("SELECT COUNT(*) FROM ({SEARCH_ITEMS_SQL}) items WHERE items.content LIKE ?1 ESCAPE char(92) AND (?2 IS NULL OR items.kind = ?2)"),
                    format!("SELECT kind, source_id, timestamp, source_app, category, {} AS excerpt FROM ({SEARCH_ITEMS_SQL}) items WHERE items.content LIKE ?1 ESCAPE char(92) AND (?2 IS NULL OR items.kind = ?2) ORDER BY julianday(items.timestamp) DESC, items.kind ASC, items.source_id DESC LIMIT ?3 OFFSET ?4", excerpt_expression(5)),
                    vec![
                        rusqlite::types::Value::Text(pattern),
                        kind.map(str::to_owned).map_or(rusqlite::types::Value::Null, rusqlite::types::Value::Text),
                    ],
                )
            };

        let total: i64 = conn.query_row(
            &count_sql,
            rusqlite::params_from_iter(params.iter()),
            |row| row.get(0),
        )?;
        if query.is_empty() {
            params.push(rusqlite::types::Value::Integer(sql_limit));
            params.push(rusqlite::types::Value::Integer(sql_offset));
        } else {
            params.push(rusqlite::types::Value::Integer(sql_limit));
            params.push(rusqlite::types::Value::Integer(sql_offset));
        }
        params.push(rusqlite::types::Value::Text(query.clone()));
        let mut stmt = conn.prepare(&results_sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
            let kind: String = row.get(0)?;
            let source_id: String = row.get(1)?;
            let timestamp: String = row.get(2)?;
            let source_app: Option<String> = row.get(3)?;
            let category: Option<String> = row.get(4)?;
            let excerpt: String = row.get(5)?;
            Ok(LibraryItem {
                id: format!("{kind}:{source_id}"),
                kind,
                timestamp,
                source_app,
                category,
                excerpt,
            })
        })?;

        let results = rows.collect::<Result<Vec<_>>>()?;
        Ok(LibrarySearchPage {
            results,
            total: total.max(0) as usize,
            limit,
            offset,
        })
    }

    pub fn get_content(&self, id: &str) -> Result<Option<String>> {
        let conn = self.open_search_connection()?;
        let Some((kind, source_id)) = id.split_once(':') else {
            return Ok(None);
        };

        let content = match kind {
            "transcript" => parse_positive_id(source_id).map(|source_id| {
                conn.query_row(
                    "SELECT text FROM history_db.dictation_history WHERE id = ?1 AND COALESCE(target_app, '') <> 'Rift Scratchpad'",
                    [source_id],
                    |row| row.get(0),
                )
            }),
            "clipboard" => parse_positive_id(source_id).map(|source_id| {
                conn.query_row(
                    "SELECT content FROM clipboard_db.clipboard_history WHERE id = ?1",
                    [source_id],
                    |row| row.get(0),
                )
            }),
            "note" if source_id == "draft" => Some(conn.query_row(
                "SELECT content FROM scratchpad_db.scratchpad_draft WHERE id = 1",
                [],
                |row| row.get(0),
            )),
            "note" if source_id.starts_with("legacy:") => {
                parse_positive_id(&source_id["legacy:".len()..]).map(|source_id| {
                    conn.query_row(
                        "SELECT substr(text, length('[Scratchpad Note] ') + 1) FROM history_db.dictation_history WHERE id = ?1 AND target_app = 'Rift Scratchpad' AND text LIKE '[Scratchpad Note] %'",
                        [source_id],
                        |row| row.get(0),
                    )
                })
            }
            _ => None,
        };

        match content {
            Some(result) => result.optional(),
            None => Ok(None),
        }
    }

    fn open_scratchpad_connection(&self) -> Result<Connection> {
        let conn = Connection::open(&self.scratchpad_db_path)?;
        conn.busy_timeout(Duration::from_secs(2))?;
        Ok(conn)
    }

    fn open_search_connection(&self) -> Result<Connection> {
        let conn = Connection::open_in_memory()?;
        conn.busy_timeout(Duration::from_secs(2))?;

        let data_dir = get_app_data_dir();
        attach_database(&conn, "history_db", &data_dir.join("history.db"))?;
        attach_database(&conn, "clipboard_db", &data_dir.join("clipboard.db"))?;
        attach_database(&conn, "scratchpad_db", &self.scratchpad_db_path)?;
        Ok(conn)
    }
}

fn parse_positive_id(source_id: &str) -> Option<i64> {
    source_id.parse::<i64>().ok().filter(|id| *id > 0)
}

fn attach_database(conn: &Connection, schema: &str, path: &Path) -> Result<()> {
    let sql = match schema {
        "history_db" => "ATTACH DATABASE ?1 AS history_db",
        "clipboard_db" => "ATTACH DATABASE ?1 AS clipboard_db",
        "scratchpad_db" => "ATTACH DATABASE ?1 AS scratchpad_db",
        _ => {
            return Err(rusqlite::Error::InvalidParameterName(
                "unknown database schema".into(),
            ))
        }
    };
    conn.execute(sql, params![path.to_string_lossy().as_ref()])?;
    Ok(())
}

fn excerpt_expression(query_parameter: usize) -> String {
    let match_position = format!("instr(lower(items.content), lower(?{query_parameter}))");
    let start_position = format!("max(1, {match_position} - 73)");
    format!(
        "CASE WHEN length(items.content) <= 220 THEN items.content ELSE\
         (CASE WHEN {match_position} > 74 THEN '…' ELSE '' END) ||\
         substr(items.content, {start_position}, 220) ||\
         (CASE WHEN {match_position} > 0 AND {match_position} + 146 < length(items.content) THEN '…' ELSE '' END) END"
    )
}
