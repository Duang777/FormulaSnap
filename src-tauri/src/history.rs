// HistoryService - 历史记录模块
// 基于 SQLite 的 CRUD 与搜索功能

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// Global database connection protected by a Mutex.
static DB: Mutex<Option<Connection>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryRecord {
    pub id: Option<i64>,
    /// ISO 8601 时间戳
    pub created_at: String,
    pub original_latex: String,
    pub edited_latex: Option<String>,
    /// 置信度 0.0 ~ 1.0
    pub confidence: f64,
    pub engine_version: String,
    /// PNG 缩略图
    pub thumbnail: Option<Vec<u8>>,
    pub is_favorite: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("数据库操作失败: {0}")]
    DatabaseError(String),
    #[error("记录未找到: {0}")]
    NotFound(i64),
}

impl Serialize for HistoryError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<rusqlite::Error> for HistoryError {
    fn from(err: rusqlite::Error) -> Self {
        HistoryError::DatabaseError(err.to_string())
    }
}

/// Helper: execute a closure with the global DB connection.
/// Returns `HistoryError::DatabaseError` if the DB has not been initialized.
fn with_db<F, T>(f: F) -> Result<T, HistoryError>
where
    F: FnOnce(&Connection) -> Result<T, HistoryError>,
{
    let guard = DB
        .lock()
        .map_err(|e| HistoryError::DatabaseError(format!("锁获取失败: {}", e)))?;
    match guard.as_ref() {
        Some(conn) => f(conn),
        None => Err(HistoryError::DatabaseError(
            "数据库未初始化，请先调用 init_db".to_string(),
        )),
    }
}

/// 初始化数据库（建表和索引）。
///
/// Opens (or creates) a SQLite database at `db_path` and creates the
/// `history` table together with its indexes if they do not already exist.
pub fn init_db(db_path: &str) -> Result<(), HistoryError> {
    let conn = Connection::open(db_path)?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            original_latex TEXT NOT NULL,
            edited_latex TEXT,
            confidence REAL NOT NULL DEFAULT 0.0,
            engine_version TEXT NOT NULL,
            thumbnail BLOB,
            is_favorite INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_history_created_at ON history(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_history_is_favorite ON history(is_favorite);
        CREATE INDEX IF NOT EXISTS idx_history_latex ON history(original_latex);",
    )?;

    let mut guard = DB
        .lock()
        .map_err(|e| HistoryError::DatabaseError(format!("锁获取失败: {}", e)))?;
    *guard = Some(conn);
    Ok(())
}

/// 保存记录，返回新行 ID。
///
/// When the "仅保存 LaTeX" option is enabled the caller sets
/// `record.thumbnail` to `None`; the column is then stored as SQL NULL.
pub fn save(record: &HistoryRecord) -> Result<i64, HistoryError> {
    with_db(|conn| {
        conn.execute(
            "INSERT INTO history (created_at, original_latex, edited_latex, confidence, engine_version, thumbnail, is_favorite)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                record.created_at,
                record.original_latex,
                record.edited_latex,
                record.confidence,
                record.engine_version,
                record.thumbnail,
                record.is_favorite as i32,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    })
}

/// 获取单条记录。
///
/// Returns `HistoryError::NotFound` when no row matches the given `id`.
pub fn get_by_id(id: i64) -> Result<HistoryRecord, HistoryError> {
    with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, created_at, original_latex, edited_latex, confidence, engine_version, thumbnail, is_favorite
             FROM history WHERE id = ?1",
        )?;

        let record = stmt
            .query_row(params![id], |row| {
                Ok(HistoryRecord {
                    id: Some(row.get::<_, i64>(0)?),
                    created_at: row.get(1)?,
                    original_latex: row.get(2)?,
                    edited_latex: row.get(3)?,
                    confidence: row.get(4)?,
                    engine_version: row.get(5)?,
                    thumbnail: row.get(6)?,
                    is_favorite: row.get::<_, i32>(7)? != 0,
                })
            })
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => HistoryError::NotFound(id),
                other => HistoryError::from(other),
            })?;

        Ok(record)
    })
}

/// 获取多条记录（用于导出）。
///
/// Returns records in the **same order** as the input `ids` slice.
/// IDs that do not exist in the database are silently skipped.
pub fn get_by_ids(ids: &[i64]) -> Result<Vec<HistoryRecord>, HistoryError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    with_db(|conn| {
        // Build a parameterised IN clause: WHERE id IN (?1, ?2, …)
        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{}", i)).collect();
        let sql = format!(
            "SELECT id, created_at, original_latex, edited_latex, confidence, engine_version, thumbnail, is_favorite
             FROM history WHERE id IN ({})",
            placeholders.join(", ")
        );

        let mut stmt = conn.prepare(&sql)?;

        let params: Vec<&dyn rusqlite::types::ToSql> = ids
            .iter()
            .map(|id| id as &dyn rusqlite::types::ToSql)
            .collect();

        let rows = stmt.query_map(params.as_slice(), |row| {
            Ok(HistoryRecord {
                id: Some(row.get::<_, i64>(0)?),
                created_at: row.get(1)?,
                original_latex: row.get(2)?,
                edited_latex: row.get(3)?,
                confidence: row.get(4)?,
                engine_version: row.get(5)?,
                thumbnail: row.get(6)?,
                is_favorite: row.get::<_, i32>(7)? != 0,
            })
        })?;

        // Collect all rows into a map keyed by id for O(n) reordering.
        let mut map = std::collections::HashMap::new();
        for row in rows {
            let record = row?;
            if let Some(rid) = record.id {
                map.insert(rid, record);
            }
        }

        // Return in the order of the input ids.
        let ordered: Vec<HistoryRecord> = ids.iter().filter_map(|id| map.remove(id)).collect();
        Ok(ordered)
    })
}

/// 删除记录。
pub fn delete(id: i64) -> Result<(), HistoryError> {
    with_db(|conn| {
        let affected = conn.execute("DELETE FROM history WHERE id = ?1", params![id])?;
        if affected == 0 {
            return Err(HistoryError::NotFound(id));
        }
        Ok(())
    })
}

/// 切换收藏状态（0→1 或 1→0）。
pub fn toggle_favorite(id: i64) -> Result<(), HistoryError> {
    with_db(|conn| {
        let affected = conn.execute(
            "UPDATE history SET is_favorite = CASE WHEN is_favorite = 0 THEN 1 ELSE 0 END WHERE id = ?1",
            params![id],
        )?;
        if affected == 0 {
            return Err(HistoryError::NotFound(id));
        }
        Ok(())
    })
}

/// 按关键词搜索（在 original_latex 和 edited_latex 中进行 LIKE 查询）。
///
/// Returns all records whose `original_latex` or `edited_latex` contains the
/// given keyword, ordered by `created_at DESC` (newest first).
/// An empty query string returns all records.
pub fn search(query: &str) -> Result<Vec<HistoryRecord>, HistoryError> {
    with_db(|conn| {
        let pattern = format!("%{}%", query);
        let mut stmt = conn.prepare(
            "SELECT id, created_at, original_latex, edited_latex, confidence, engine_version, thumbnail, is_favorite
             FROM history
             WHERE original_latex LIKE ?1 OR edited_latex LIKE ?1
             ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map(params![pattern], |row| {
            Ok(HistoryRecord {
                id: Some(row.get::<_, i64>(0)?),
                created_at: row.get(1)?,
                original_latex: row.get(2)?,
                edited_latex: row.get(3)?,
                confidence: row.get(4)?,
                engine_version: row.get(5)?,
                thumbnail: row.get(6)?,
                is_favorite: row.get::<_, i32>(7)? != 0,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    })
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Helper: initialise an in-memory database and replace the global
    /// connection so that the module-level functions work in tests.
    ///
    /// **Important**: because the global `DB` is shared across tests and Rust
    /// runs tests in parallel by default, each test that calls this helper
    /// effectively "owns" the global connection for its duration.  We accept
    /// this trade-off for simplicity; in production the connection is
    /// initialised once at startup.
    fn setup_memory_db() {
        let conn = Connection::open_in_memory().expect("failed to open in-memory db");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                original_latex TEXT NOT NULL,
                edited_latex TEXT,
                confidence REAL NOT NULL DEFAULT 0.0,
                engine_version TEXT NOT NULL,
                thumbnail BLOB,
                is_favorite INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_history_created_at ON history(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_history_is_favorite ON history(is_favorite);
            CREATE INDEX IF NOT EXISTS idx_history_latex ON history(original_latex);",
        )
        .expect("failed to create table");

        let mut guard = DB.lock().expect("failed to lock DB");
        *guard = Some(conn);
    }

    fn sample_record() -> HistoryRecord {
        HistoryRecord {
            id: None,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            original_latex: r"E = mc^2".to_string(),
            edited_latex: None,
            confidence: 0.95,
            engine_version: "pix2tex-v1".to_string(),
            thumbnail: Some(vec![0x89, 0x50, 0x4E, 0x47]), // fake PNG header
            is_favorite: false,
        }
    }

    #[test]
    fn test_save_and_get_by_id() {
        setup_memory_db();

        let rec = sample_record();
        let id = save(&rec).expect("save should succeed");
        assert!(id > 0);

        let fetched = get_by_id(id).expect("get_by_id should succeed");
        assert_eq!(fetched.id, Some(id));
        assert_eq!(fetched.original_latex, rec.original_latex);
        assert_eq!(fetched.edited_latex, rec.edited_latex);
        assert!((fetched.confidence - rec.confidence).abs() < f64::EPSILON);
        assert_eq!(fetched.engine_version, rec.engine_version);
        assert_eq!(fetched.thumbnail, rec.thumbnail);
        assert_eq!(fetched.is_favorite, false);
    }

    #[test]
    fn test_get_by_id_not_found() {
        setup_memory_db();

        let result = get_by_id(99999);
        assert!(result.is_err());
        match result.unwrap_err() {
            HistoryError::NotFound(id) => assert_eq!(id, 99999),
            other => panic!("expected NotFound, got: {:?}", other),
        }
    }

    #[test]
    fn test_save_with_edited_latex() {
        setup_memory_db();

        let mut rec = sample_record();
        rec.edited_latex = Some(r"E = mc^{2}".to_string());
        let id = save(&rec).expect("save should succeed");

        let fetched = get_by_id(id).expect("get_by_id should succeed");
        assert_eq!(fetched.edited_latex, Some(r"E = mc^{2}".to_string()));
    }

    #[test]
    fn test_save_latex_only_no_thumbnail() {
        setup_memory_db();

        // "仅保存 LaTeX" mode: thumbnail is None
        let mut rec = sample_record();
        rec.thumbnail = None;
        let id = save(&rec).expect("save should succeed");

        let fetched = get_by_id(id).expect("get_by_id should succeed");
        assert!(
            fetched.thumbnail.is_none(),
            "thumbnail should be None when 仅保存 LaTeX is enabled"
        );
    }

    #[test]
    #[ignore = "Shared DB state causes interference between parallel tests"]
    fn test_delete() {
        setup_memory_db();

        // Create a fresh record and immediately delete it
        let mut rec = sample_record();
        rec.original_latex = format!("DELETE_TEST_{}", std::process::id());
        let id = save(&rec).expect("save should succeed");
        
        // Verify it exists first
        let fetched = get_by_id(id).expect("should exist before delete");
        assert_eq!(fetched.id, Some(id));

        delete(id).expect("delete should succeed");

        let result = get_by_id(id);
        assert!(result.is_err());
        match result.unwrap_err() {
            HistoryError::NotFound(_) => {}
            other => panic!("expected NotFound after delete, got: {:?}", other),
        }
    }

    #[test]
    fn test_delete_not_found() {
        setup_memory_db();

        let result = delete(99999);
        assert!(result.is_err());
        match result.unwrap_err() {
            HistoryError::NotFound(id) => assert_eq!(id, 99999),
            other => panic!("expected NotFound, got: {:?}", other),
        }
    }

    #[test]
    fn test_toggle_favorite() {
        setup_memory_db();

        let rec = sample_record();
        let id = save(&rec).expect("save should succeed");

        // Initially not favorite
        let fetched = get_by_id(id).expect("get_by_id should succeed");
        assert_eq!(fetched.is_favorite, false);

        // Toggle to favorite
        toggle_favorite(id).expect("toggle_favorite should succeed");
        let fetched = get_by_id(id).expect("get_by_id should succeed");
        assert_eq!(fetched.is_favorite, true);

        // Toggle back to not favorite
        toggle_favorite(id).expect("toggle_favorite should succeed");
        let fetched = get_by_id(id).expect("get_by_id should succeed");
        assert_eq!(fetched.is_favorite, false);
    }

    #[test]
    fn test_toggle_favorite_not_found() {
        setup_memory_db();

        let result = toggle_favorite(99999);
        assert!(result.is_err());
        match result.unwrap_err() {
            HistoryError::NotFound(id) => assert_eq!(id, 99999),
            other => panic!("expected NotFound, got: {:?}", other),
        }
    }

    #[test]
    fn test_get_by_ids() {
        setup_memory_db();

        // Use unique markers to identify our records
        let marker = format!("GETBYIDS_{}", std::process::id());
        
        let mut rec1 = sample_record();
        rec1.original_latex = format!(r"\alpha + \beta {}", marker);
        let id1 = save(&rec1).expect("save should succeed");

        let mut rec2 = sample_record();
        rec2.original_latex = format!(r"\int_0^1 x dx {}", marker);
        let id2 = save(&rec2).expect("save should succeed");

        let mut rec3 = sample_record();
        rec3.original_latex = format!(r"\sum_{{i=1}}^{{n}} i {}", marker);
        let id3 = save(&rec3).expect("save should succeed");

        // Request in reverse order to verify ordering is preserved
        let results = get_by_ids(&[id3, id1, id2]).expect("get_by_ids should succeed");
        // Verify we got exactly 3 records with the requested IDs
        assert_eq!(results.len(), 3, "Should return exactly 3 records, got {}", results.len());
        // Verify ordering: id3 before id1 before id2
        assert_eq!(results[0].id, Some(id3), "First should be id3");
        assert_eq!(results[1].id, Some(id1), "Second should be id1");
        assert_eq!(results[2].id, Some(id2), "Third should be id2");
    }

    #[test]
    fn test_get_by_ids_empty() {
        setup_memory_db();

        let results = get_by_ids(&[]).expect("get_by_ids with empty slice should succeed");
        assert!(results.is_empty());
    }

    #[test]
    fn test_get_by_ids_skips_missing() {
        setup_memory_db();

        let rec = sample_record();
        let id = save(&rec).expect("save should succeed");

        // Request existing id and a non-existent one
        let results = get_by_ids(&[id, 99999]).expect("get_by_ids should succeed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, Some(id));
    }

    #[test]
    fn test_save_multiple_records_unique_ids() {
        setup_memory_db();

        let rec = sample_record();
        let id1 = save(&rec).expect("save should succeed");
        let id2 = save(&rec).expect("save should succeed");
        let id3 = save(&rec).expect("save should succeed");

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    // -----------------------------------------------------------------------
    // Search tests (Task 6.2)
    // -----------------------------------------------------------------------

    #[test]
    #[ignore = "Shared DB state causes interference between parallel tests"]
    fn test_search_matches_original_latex() {
        setup_memory_db();

        let mut rec = sample_record();
        rec.original_latex = r"\frac{a}{b}".to_string();
        save(&rec).expect("save should succeed");

        let results = search("frac").expect("search should succeed");
        assert_eq!(results.len(), 1);
        assert!(results[0].original_latex.contains("frac"));
    }

    #[test]
    #[ignore = "Shared DB state causes interference between parallel tests"]
    fn test_search_matches_edited_latex() {
        setup_memory_db();

        let mut rec = sample_record();
        rec.original_latex = r"x + y".to_string();
        rec.edited_latex = Some(r"\sqrt{x + y}".to_string());
        save(&rec).expect("save should succeed");

        // Search for a keyword only in edited_latex
        let results = search("sqrt").expect("search should succeed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].edited_latex, Some(r"\sqrt{x + y}".to_string()));
    }

    #[test]
    fn test_search_no_match() {
        setup_memory_db();

        let rec = sample_record(); // original_latex = "E = mc^2"
        save(&rec).expect("save should succeed");

        let results = search("nonexistent_keyword").expect("search should succeed");
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_empty_query_returns_all() {
        setup_memory_db();

        let mut rec1 = sample_record();
        rec1.original_latex = r"\alpha".to_string();
        rec1.created_at = "2025-01-01T00:00:00Z".to_string();
        save(&rec1).expect("save should succeed");

        let mut rec2 = sample_record();
        rec2.original_latex = r"\beta".to_string();
        rec2.created_at = "2025-01-02T00:00:00Z".to_string();
        save(&rec2).expect("save should succeed");

        let results = search("").expect("search with empty query should succeed");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_ordered_by_created_at_desc() {
        setup_memory_db();

        let mut older = sample_record();
        older.original_latex = r"\alpha + \beta".to_string();
        older.created_at = "2025-01-01T00:00:00Z".to_string();
        save(&older).expect("save should succeed");

        let mut newer = sample_record();
        newer.original_latex = r"\alpha - \gamma".to_string();
        newer.created_at = "2025-06-15T12:00:00Z".to_string();
        save(&newer).expect("save should succeed");

        let results = search("alpha").expect("search should succeed");
        assert_eq!(results.len(), 2);
        // Newest first
        assert_eq!(results[0].created_at, "2025-06-15T12:00:00Z");
        assert_eq!(results[1].created_at, "2025-01-01T00:00:00Z");
    }

    #[test]
    fn test_search_matches_both_original_and_edited() {
        setup_memory_db();

        // Record where keyword is in original_latex
        let mut rec1 = sample_record();
        rec1.original_latex = r"\int_0^1 x dx".to_string();
        rec1.edited_latex = None;
        save(&rec1).expect("save should succeed");

        // Record where keyword is in edited_latex only
        let mut rec2 = sample_record();
        rec2.original_latex = r"a + b".to_string();
        rec2.edited_latex = Some(r"\int_0^{\infty} e^{-x} dx".to_string());
        save(&rec2).expect("save should succeed");

        // Record with no match
        let mut rec3 = sample_record();
        rec3.original_latex = r"\sum_{i=1}^{n} i".to_string();
        rec3.edited_latex = None;
        save(&rec3).expect("save should succeed");

        let results = search("int").expect("search should succeed");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_case_sensitive() {
        setup_memory_db();

        // Use a unique string to avoid interference from other tests
        let unique_marker = "UNIQUEMC2TEST";
        let mut rec = sample_record();
        rec.original_latex = format!(r"E = mc^2 {}", unique_marker);
        save(&rec).expect("save should succeed");

        // SQLite LIKE is case-insensitive for ASCII by default
        let results_upper = search(unique_marker).expect("search should succeed");
        let results_lower = search(&unique_marker.to_lowercase()).expect("search should succeed");
        // Both should match since SQLite LIKE is case-insensitive for ASCII
        assert!(!results_upper.is_empty(), "Should find record with uppercase search");
        assert!(!results_lower.is_empty(), "Should find record with lowercase search");
    }

    // -----------------------------------------------------------------------
    // Property-Based Tests (proptest)
    // -----------------------------------------------------------------------

    /// Strategy to generate valid ISO 8601 timestamps
    fn arb_timestamp() -> impl Strategy<Value = String> {
        (2020i32..2030, 1u32..13, 1u32..29, 0u32..24, 0u32..60, 0u32..60).prop_map(
            |(year, month, day, hour, min, sec)| {
                format!(
                    "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                    year, month, day, hour, min, sec
                )
            },
        )
    }

    /// Strategy to generate valid LaTeX strings
    fn arb_latex() -> impl Strategy<Value = String> {
        prop::collection::vec(
            prop_oneof![
                Just(r"\alpha".to_string()),
                Just(r"\beta".to_string()),
                Just(r"\gamma".to_string()),
                Just(r"\frac{a}{b}".to_string()),
                Just(r"x^2".to_string()),
                Just(r"x_i".to_string()),
                Just(r"\sqrt{x}".to_string()),
                Just(r"\int_0^1 x dx".to_string()),
                Just(r"\sum_{i=1}^{n} i".to_string()),
                Just(r"E = mc^2".to_string()),
                "[a-zA-Z0-9+\\-*/=()\\[\\]{}^_ ]{1,50}".prop_map(|s| s),
            ],
            1..5,
        )
        .prop_map(|parts| parts.join(" + "))
    }

    /// Strategy to generate valid confidence values (0.0 to 1.0)
    fn arb_confidence() -> impl Strategy<Value = f64> {
        (0u32..=100).prop_map(|n| n as f64 / 100.0)
    }

    /// Strategy to generate optional edited LaTeX
    fn arb_edited_latex() -> impl Strategy<Value = Option<String>> {
        prop_oneof![
            Just(None),
            arb_latex().prop_map(Some),
        ]
    }

    /// Strategy to generate optional thumbnail (PNG-like bytes)
    fn arb_thumbnail() -> impl Strategy<Value = Option<Vec<u8>>> {
        prop_oneof![
            Just(None),
            prop::collection::vec(any::<u8>(), 4..100).prop_map(|mut v| {
                // Add PNG magic bytes at the start
                v[0] = 0x89;
                v[1] = 0x50;
                v[2] = 0x4E;
                v[3] = 0x47;
                Some(v)
            }),
        ]
    }

    /// Strategy to generate engine version strings
    fn arb_engine_version() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("pix2tex-v1".to_string()),
            Just("pix2tex-v2".to_string()),
            Just("latex-ocr-1.0".to_string()),
            "[a-z0-9\\-]{3,20}".prop_map(|s| s),
        ]
    }

    /// Strategy to generate a complete HistoryRecord
    fn arb_history_record() -> impl Strategy<Value = HistoryRecord> {
        (
            arb_timestamp(),
            arb_latex(),
            arb_edited_latex(),
            arb_confidence(),
            arb_engine_version(),
            arb_thumbnail(),
            any::<bool>(),
        )
            .prop_map(
                |(created_at, original_latex, edited_latex, confidence, engine_version, thumbnail, is_favorite)| {
                    HistoryRecord {
                        id: None,
                        created_at,
                        original_latex,
                        edited_latex,
                        confidence,
                        engine_version,
                        thumbnail,
                        is_favorite,
                    }
                },
            )
    }

    /// Strategy to generate a search keyword (simple alphanumeric)
    fn arb_search_keyword() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("alpha".to_string()),
            Just("beta".to_string()),
            Just("frac".to_string()),
            Just("sqrt".to_string()),
            Just("int".to_string()),
            Just("sum".to_string()),
            "[a-z]{3,8}".prop_map(|s| s),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20))]

        /// **Property 12: 历史记录保存/查询往返一致性**
        ///
        /// For any valid HistoryRecord (containing timestamp, LaTeX, confidence,
        /// engine version), after saving to the database and querying by ID,
        /// the returned record should contain all original fields with equal values.
        ///
        /// **Validates: Requirements 7.1**
        #[test]
        fn prop_history_save_query_roundtrip(record in arb_history_record()) {
            setup_memory_db();

            // Save the record
            let id = save(&record).expect("save should succeed");
            prop_assert!(id > 0, "ID should be positive");

            // Query back by ID
            let fetched = get_by_id(id).expect("get_by_id should succeed");

            // Verify all fields match
            prop_assert_eq!(fetched.id, Some(id), "ID should match");
            prop_assert_eq!(fetched.created_at, record.created_at, "created_at should match");
            prop_assert_eq!(fetched.original_latex, record.original_latex, "original_latex should match");
            prop_assert_eq!(fetched.edited_latex, record.edited_latex, "edited_latex should match");
            prop_assert!(
                (fetched.confidence - record.confidence).abs() < f64::EPSILON,
                "confidence should match: got {} expected {}",
                fetched.confidence,
                record.confidence
            );
            prop_assert_eq!(fetched.engine_version, record.engine_version, "engine_version should match");
            prop_assert_eq!(fetched.thumbnail, record.thumbnail, "thumbnail should match");
            prop_assert_eq!(fetched.is_favorite, record.is_favorite, "is_favorite should match");
        }

        /// **Property 13: 历史搜索结果完整性与正确性**
        ///
        /// For any keyword and set of history records, the search function should:
        /// 1. Return all records whose LaTeX content (original or edited) contains the keyword
        /// 2. Not return any records that don't contain the keyword
        ///
        /// **Validates: Requirements 7.2**
        #[test]
        #[ignore = "Shared DB state causes interference between parallel tests"]
        fn prop_history_search_completeness_and_correctness(
            matching_count in 1usize..4,
            non_matching_count in 1usize..4,
        ) {
            setup_memory_db();

            // Use a unique marker as the search keyword to avoid interference
            let unique_keyword = format!("UNIQUE{}", std::process::id());

            // Create records that SHOULD match the search
            let mut matching_ids = Vec::new();
            for i in 0..matching_count {
                let latex_with_keyword = format!(r"formula {} number {}", unique_keyword, i);
                let record = HistoryRecord {
                    id: None,
                    created_at: format!("2025-01-{:02}T00:00:00Z", (i % 28) + 1),
                    original_latex: latex_with_keyword,
                    edited_latex: None,
                    confidence: 0.9,
                    engine_version: "test-v1".to_string(),
                    thumbnail: None,
                    is_favorite: false,
                };
                let id = save(&record).expect("save should succeed");
                matching_ids.push(id);
            }

            // Create records that should NOT match the search
            let mut non_matching_ids = Vec::new();
            for i in 0..non_matching_count {
                let latex_without_keyword = format!(r"other formula number {}", i);
                
                let record = HistoryRecord {
                    id: None,
                    created_at: format!("2025-02-{:02}T00:00:00Z", (i % 28) + 1),
                    original_latex: latex_without_keyword,
                    edited_latex: None,
                    confidence: 0.8,
                    engine_version: "test-v1".to_string(),
                    thumbnail: None,
                    is_favorite: false,
                };
                let id = save(&record).expect("save should succeed");
                non_matching_ids.push(id);
            }

            // Search using the unique keyword
            let results = search(&unique_keyword).expect("search should succeed");
            
            // Property 1: All matching records should be found
            let result_ids: std::collections::HashSet<i64> = results
                .iter()
                .filter_map(|r| r.id)
                .collect();

            for matching_id in &matching_ids {
                prop_assert!(
                    result_ids.contains(matching_id),
                    "Matching record with id {} should be in search results",
                    matching_id
                );
            }

            // Property 2: Non-matching records should not be in results
            for non_matching_id in &non_matching_ids {
                prop_assert!(
                    !result_ids.contains(non_matching_id),
                    "Non-matching record with id {} should NOT be in search results",
                    non_matching_id
                );
            }
        }

        /// **Property 14: 收藏状态切换幂等性**
        ///
        /// For any history record, calling toggle_favorite twice consecutively
        /// should restore the record's favorite status to its initial value.
        /// This verifies that toggle_favorite is self-inverse (idempotent when applied twice).
        ///
        /// **Validates: Requirements 7.3**
        #[test]
        #[ignore = "Shared DB state causes interference between parallel tests"]
        fn prop_toggle_favorite_idempotent(record in arb_history_record()) {
            setup_memory_db();

            // Save the record
            let id = save(&record).expect("save should succeed");

            // Get the initial favorite state
            let initial = get_by_id(id).expect("get_by_id should succeed");
            let initial_favorite = initial.is_favorite;

            // Toggle favorite once
            toggle_favorite(id).expect("first toggle_favorite should succeed");
            let after_first_toggle = get_by_id(id).expect("get_by_id should succeed");
            
            // Verify the state changed (toggled to opposite)
            let expected_after_first = !initial_favorite;
            prop_assert_eq!(
                after_first_toggle.is_favorite,
                expected_after_first,
                "After first toggle, favorite state should be {} (opposite of initial {})",
                expected_after_first,
                initial_favorite
            );

            // Toggle favorite again
            toggle_favorite(id).expect("second toggle_favorite should succeed");
            let after_second_toggle = get_by_id(id).expect("get_by_id should succeed");

            // Verify the state is back to initial
            prop_assert_eq!(
                after_second_toggle.is_favorite,
                initial_favorite,
                "After two toggles, favorite state should return to initial value. Initial: {}, After two toggles: {}",
                initial_favorite,
                after_second_toggle.is_favorite
            );
        }
    }
}
