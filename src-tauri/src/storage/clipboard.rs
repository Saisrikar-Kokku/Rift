use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use super::settings::get_app_data_dir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub id: i64,
    pub timestamp: String,
    pub content: String,
    pub category: String, // "code", "url", "email", "filepath", "text"
    pub source_app: Option<String>,
    pub char_count: usize,
    pub preview: String,
    pub is_pinned: bool,
    pub hash: String,
}

pub struct ClipboardStore {
    db_path: PathBuf,
}

impl ClipboardStore {
    pub fn new() -> Result<Self> {
        let db_path = get_app_data_dir().join("clipboard.db");
        let conn = Connection::open(&db_path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS clipboard_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                content TEXT NOT NULL,
                category TEXT NOT NULL,
                source_app TEXT,
                char_count INTEGER NOT NULL,
                preview TEXT NOT NULL,
                is_pinned INTEGER NOT NULL DEFAULT 0,
                hash TEXT NOT NULL
            )",
            [],
        )?;

        // Indexes for performance
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_clip_id_desc ON clipboard_history(id DESC)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_clip_pinned ON clipboard_history(is_pinned)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_clip_category ON clipboard_history(category)",
            [],
        );

        Ok(Self { db_path })
    }

    /// Compute a deterministic hash for duplicate detection
    fn compute_hash(text: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    /// Generate a clean, single-line preview string
    fn generate_preview(text: &str) -> String {
        let cleaned: String = text
            .chars()
            .take(140)
            .map(|c| if c == '\n' || c == '\r' || c == '\t' { ' ' } else { c })
            .collect();
        let trimmed = cleaned.trim();
        if text.chars().count() > 140 {
            format!("{}...", trimmed)
        } else {
            trimmed.to_string()
        }
    }

    /// Insert a new clipboard item. Returns Some(id) if inserted, or None if skipped as duplicate.
    pub fn insert_clip(
        &self,
        content: &str,
        source_app: Option<&str>,
        category: &str,
    ) -> Result<Option<i64>> {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        // Truncate overly massive text clips (safety limit: 200,000 characters)
        let safe_content = if trimmed.len() > 200_000 {
            &trimmed[..200_000]
        } else {
            trimmed
        };

        let hash = Self::compute_hash(safe_content);
        let mut conn = Connection::open(&self.db_path)?;

        // 1. Check if identical to the most recent clip
        let last_hash: Option<String> = conn
            .query_row(
                "SELECT hash FROM clipboard_history ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();

        if let Some(lh) = last_hash {
            if lh == hash {
                return Ok(None); // Skip consecutive duplicate
            }
        }

        let timestamp = chrono::Local::now().to_rfc3339();
        let char_count = safe_content.chars().count();
        let preview = Self::generate_preview(safe_content);

        conn.execute(
            "INSERT INTO clipboard_history (timestamp, content, category, source_app, char_count, preview, is_pinned, hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
            params![
                timestamp,
                safe_content,
                category,
                source_app,
                char_count,
                preview,
                hash
            ],
        )?;

        let new_id = conn.last_insert_rowid();

        // Auto-prune unpinned clips to keep maximum 50 unpinned
        let _ = Self::prune_internal(&mut conn, 50);

        Ok(Some(new_id))
    }

    /// Internal helper to prune unpinned items beyond max_keep
    fn prune_internal(conn: &mut Connection, max_keep: usize) -> Result<()> {
        let unpinned_count: usize = conn.query_row(
            "SELECT COUNT(*) FROM clipboard_history WHERE is_pinned = 0",
            [],
            |r| r.get(0),
        )?;

        if unpinned_count > max_keep {
            let delete_count = unpinned_count - max_keep;
            conn.execute(
                "DELETE FROM clipboard_history WHERE id IN (
                    SELECT id FROM clipboard_history WHERE is_pinned = 0 ORDER BY id ASC LIMIT ?1
                )",
                params![delete_count],
            )?;
        }
        Ok(())
    }

    /// Retrieve recent clips with optional category filter and search query
    pub fn get_recent_clips(
        &self,
        limit: usize,
        category_filter: Option<&str>,
        search_query: Option<&str>,
    ) -> Result<Vec<ClipboardEntry>> {
        let conn = Connection::open(&self.db_path)?;
        let max_limit = limit.clamp(1, 200);

        let mut query = String::from(
            "SELECT id, timestamp, content, category, source_app, char_count, preview, is_pinned, hash
             FROM clipboard_history WHERE 1=1"
        );

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(cat) = category_filter {
            let cat_clean = cat.trim();
            if !cat_clean.is_empty() && cat_clean != "all" {
                if cat_clean == "pinned" {
                    query.push_str(" AND is_pinned = 1");
                } else {
                    query.push_str(" AND category = ?");
                    params_vec.push(Box::new(cat_clean.to_string()));
                }
            }
        }

        if let Some(q) = search_query {
            let q_clean = q.trim();
            if !q_clean.is_empty() {
                query.push_str(" AND (content LIKE ? OR preview LIKE ? OR source_app LIKE ?)");
                let pattern = format!("%{}%", q_clean);
                params_vec.push(Box::new(pattern.clone()));
                params_vec.push(Box::new(pattern.clone()));
                params_vec.push(Box::new(pattern));
            }
        }

        // Pinned items stay at top or chronological? Standard clipboard timeline: sorted by id DESC
        // with pinned indicator.
        query.push_str(" ORDER BY is_pinned DESC, id DESC LIMIT ?");
        params_vec.push(Box::new(max_limit));

        let mut stmt = conn.prepare(&query)?;
        let params_slice: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();

        let rows = stmt.query_map(params_slice.as_slice(), |row| {
            let is_pinned_int: i64 = row.get(7)?;
            Ok(ClipboardEntry {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                content: row.get(2)?,
                category: row.get(3)?,
                source_app: row.get(4)?,
                char_count: row.get(5)?,
                preview: row.get(6)?,
                is_pinned: is_pinned_int != 0,
                hash: row.get(8)?,
            })
        })?;

        let mut entries = Vec::new();
        for r in rows.flatten() {
            entries.push(r);
        }

        Ok(entries)
    }

    /// Retrieve a single clip by its database ID
    pub fn get_clip_by_id(&self, id: i64) -> Result<Option<ClipboardEntry>> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, content, category, source_app, char_count, preview, is_pinned, hash
             FROM clipboard_history WHERE id = ?1",
        )?;

        let mut rows = stmt.query_map(params![id], |row| {
            let is_pinned_int: i64 = row.get(7)?;
            Ok(ClipboardEntry {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                content: row.get(2)?,
                category: row.get(3)?,
                source_app: row.get(4)?,
                char_count: row.get(5)?,
                preview: row.get(6)?,
                is_pinned: is_pinned_int != 0,
                hash: row.get(8)?,
            })
        })?;

        if let Some(entry) = rows.next() {
            Ok(Some(entry?))
        } else {
            Ok(None)
        }
    }

    /// Retrieve clip by relative index (1 = newest, 2 = 2nd newest, 3 = 3rd newest, etc.)
    pub fn get_clip_by_relative_index(&self, index_1_based: usize) -> Result<Option<ClipboardEntry>> {
        if index_1_based == 0 {
            return Ok(None);
        }
        let offset = index_1_based - 1;
        let conn = Connection::open(&self.db_path)?;

        let mut stmt = conn.prepare(
            "SELECT id, timestamp, content, category, source_app, char_count, preview, is_pinned, hash
             FROM clipboard_history ORDER BY id DESC LIMIT 1 OFFSET ?1",
        )?;

        let mut rows = stmt.query_map(params![offset], |row| {
            let is_pinned_int: i64 = row.get(7)?;
            Ok(ClipboardEntry {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                content: row.get(2)?,
                category: row.get(3)?,
                source_app: row.get(4)?,
                char_count: row.get(5)?,
                preview: row.get(6)?,
                is_pinned: is_pinned_int != 0,
                hash: row.get(8)?,
            })
        })?;

        if let Some(entry) = rows.next() {
            Ok(Some(entry?))
        } else {
            Ok(None)
        }
    }

    /// Toggle pin state. Returns new pin state (true = pinned, false = unpinned).
    pub fn toggle_pin(&self, id: i64) -> Result<bool> {
        let conn = Connection::open(&self.db_path)?;
        let current_pin: i64 = conn.query_row(
            "SELECT is_pinned FROM clipboard_history WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )?;

        let new_pin = if current_pin == 0 { 1 } else { 0 };
        conn.execute(
            "UPDATE clipboard_history SET is_pinned = ?1 WHERE id = ?2",
            params![new_pin, id],
        )?;

        Ok(new_pin == 1)
    }

    /// Delete a specific clip by ID
    pub fn delete_clip(&self, id: i64) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute("DELETE FROM clipboard_history WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Clear all unpinned clips, preserving all pinned clips
    pub fn clear_unpinned(&self) -> Result<usize> {
        let conn = Connection::open(&self.db_path)?;
        let deleted = conn.execute("DELETE FROM clipboard_history WHERE is_pinned = 0", [])?;
        Ok(deleted)
    }

    /// Get total count and pinned count
    pub fn get_counts(&self) -> Result<(usize, usize)> {
        let conn = Connection::open(&self.db_path)?;
        let total: usize = conn.query_row("SELECT COUNT(*) FROM clipboard_history", [], |r| r.get(0))?;
        let pinned: usize = conn.query_row("SELECT COUNT(*) FROM clipboard_history WHERE is_pinned = 1", [], |r| r.get(0))?;
        Ok((total, pinned))
    }
}
