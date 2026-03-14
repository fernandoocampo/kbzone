use std::sync::{Arc, Mutex, Once};

use rusqlite::{params, Connection};

use crate::domain::{EmbeddingInput, Kb, KbFilter, KbItem, ScoredKbItem, SemanticQuery};
use crate::errors::Error;
use crate::ports::{KbStore, VectorStore};

// ---------------------------------------------------------------------------
// DDL constants (translated from Go kbcli sqlite.go)
// ---------------------------------------------------------------------------

const CREATE_KBS_TABLE: &str = "
CREATE TABLE IF NOT EXISTS kbs (
    INTERNAL_ID  INTEGER PRIMARY KEY AUTOINCREMENT,
    KB_ID        TEXT NOT NULL UNIQUE,
    KB_KEY       TEXT NOT NULL UNIQUE,
    KB_VALUE     TEXT NOT NULL,
    NOTES        TEXT NOT NULL DEFAULT '',
    CATEGORY     TEXT NOT NULL DEFAULT '',
    NAMESPACE    TEXT NOT NULL DEFAULT '',
    REFERENCE    TEXT NOT NULL DEFAULT '',
    TAG_VALUES   TEXT NOT NULL DEFAULT '',
    CREATED_ON   TEXT NOT NULL
)";

const CREATE_FTS_TABLE: &str = "
CREATE VIRTUAL TABLE IF NOT EXISTS tags_idx
USING fts5(TAG_VALUES, content='kbs', content_rowid='INTERNAL_ID')";

const CREATE_TRIGGER_AI: &str = "
CREATE TRIGGER IF NOT EXISTS kbs_ai
AFTER INSERT ON kbs BEGIN
    INSERT INTO tags_idx(rowid, TAG_VALUES) VALUES (new.INTERNAL_ID, new.TAG_VALUES);
END";

const CREATE_TRIGGER_AD: &str = "
CREATE TRIGGER IF NOT EXISTS kbs_ad
AFTER DELETE ON kbs BEGIN
    INSERT INTO tags_idx(tags_idx, rowid, TAG_VALUES) VALUES ('delete', old.INTERNAL_ID, old.TAG_VALUES);
END";

const CREATE_TRIGGER_AU: &str = "
CREATE TRIGGER IF NOT EXISTS kbs_au
AFTER UPDATE ON kbs BEGIN
    INSERT INTO tags_idx(tags_idx, rowid, TAG_VALUES) VALUES ('delete', old.INTERNAL_ID, old.TAG_VALUES);
    INSERT INTO tags_idx(rowid, TAG_VALUES) VALUES (new.INTERNAL_ID, new.TAG_VALUES);
END";

// ---------------------------------------------------------------------------
// Vector DDL / DML constants
// ---------------------------------------------------------------------------

/// Template for vec0 virtual table DDL — `{}` is replaced with the dimension count.
const CREATE_EMBEDDINGS_TABLE_TPL: &str = "CREATE VIRTUAL TABLE IF NOT EXISTS kb_embeddings \
     USING vec0(kb_id TEXT PRIMARY KEY, embedding float[{}])";

const INSERT_EMBEDDING: &str = "INSERT INTO kb_embeddings(kb_id, embedding) VALUES (?1, ?2)";

const DELETE_EMBEDDING: &str = "DELETE FROM kb_embeddings WHERE kb_id = ?1";

/// Step 1 of semantic search: get nearest kb_ids from the vec0 virtual table.
/// Joined queries with MATCH are not supported by vec0; two separate queries are used.
const SEARCH_KNN: &str =
    "SELECT kb_id, distance FROM kb_embeddings WHERE embedding MATCH ?1 ORDER BY distance LIMIT ?2";

/// Step 2 of semantic search: fetch KB item metadata by ID.
const GET_KB_ITEM_BY_ID: &str =
    "SELECT KB_ID, KB_KEY, CATEGORY, NAMESPACE, TAG_VALUES FROM kbs WHERE KB_ID = ?1";

// ---------------------------------------------------------------------------
// SqliteStore helpers
// ---------------------------------------------------------------------------

/// Registers the sqlite-vec extension as an auto-extension once per process.
/// Must be called before opening any SQLite connection.
fn register_vec_extension() {
    type SqliteInitFn = unsafe extern "C" fn(
        *mut rusqlite::ffi::sqlite3,
        *mut *const i8,
        *const rusqlite::ffi::sqlite3_api_routines,
    ) -> i32;

    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        let init_fn: SqliteInitFn = std::mem::transmute(sqlite_vec::sqlite3_vec_init as *const ());
        rusqlite::ffi::sqlite3_auto_extension(Some(init_fn));
    });
}

// ---------------------------------------------------------------------------
// SqliteStore
// ---------------------------------------------------------------------------

/// SQLite-backed implementation of `KbStore`.
/// `Arc<Mutex<Connection>>` allows cheap `Clone` while keeping the connection
/// thread-safe (the CLI is single-threaded, but the bound is required by the trait).
#[derive(Debug, Clone)]
pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    /// Opens (or creates) the SQLite file at `db_path`.
    /// Creates parent directories if they don't exist.
    pub fn new(db_path: &str) -> Result<Self, Error> {
        register_vec_extension();
        let path = std::path::Path::new(db_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::StorageInitError(e.to_string()))?;
        }
        let conn = Connection::open(path).map_err(|e| Error::StorageInitError(e.to_string()))?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Creates an in-memory store (used by integration tests).
    pub fn in_memory() -> Result<Self, Error> {
        register_vec_extension();
        let conn =
            Connection::open_in_memory().map_err(|e| Error::StorageInitError(e.to_string()))?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

// ---------------------------------------------------------------------------
// Helper — parse a row into KbItem
// ---------------------------------------------------------------------------

fn row_to_kb_item(
    id: String,
    key: String,
    category: String,
    namespace: String,
    tag_values: String,
) -> KbItem {
    KbItem {
        id,
        key,
        category,
        namespace,
        tags: if tag_values.is_empty() {
            vec![]
        } else {
            tag_values.split_whitespace().map(str::to_string).collect()
        },
    }
}

// ---------------------------------------------------------------------------
// KbStore implementation
// ---------------------------------------------------------------------------

impl KbStore for SqliteStore {
    fn initialize(&self) -> Result<(), Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        for ddl in &[
            CREATE_KBS_TABLE,
            CREATE_FTS_TABLE,
            CREATE_TRIGGER_AI,
            CREATE_TRIGGER_AD,
            CREATE_TRIGGER_AU,
        ] {
            conn.execute_batch(ddl)
                .map_err(|e| Error::StorageInitError(e.to_string()))?;
        }
        Ok(())
    }

    fn get_kb_by_id(&self, id: &str) -> Result<Option<Kb>, Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT KB_ID, KB_KEY, KB_VALUE, NOTES, CATEGORY, NAMESPACE,
                        REFERENCE, TAG_VALUES, CREATED_ON
                 FROM kbs WHERE KB_ID = ?1",
            )
            .map_err(|_| Error::GetKBError)?;

        let mut rows = stmt.query(params![id]).map_err(|_| Error::GetKBError)?;

        if let Some(row) = rows.next().map_err(|_| Error::GetKBError)? {
            Ok(Some(row_to_kb(row)?))
        } else {
            Ok(None)
        }
    }

    fn get_kb_by_key(&self, key: &str) -> Result<Option<Kb>, Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT KB_ID, KB_KEY, KB_VALUE, NOTES, CATEGORY, NAMESPACE,
                        REFERENCE, TAG_VALUES, CREATED_ON
                 FROM kbs WHERE KB_KEY = ?1",
            )
            .map_err(|_| Error::GetKBError)?;

        let mut rows = stmt.query(params![key]).map_err(|_| Error::GetKBError)?;

        if let Some(row) = rows.next().map_err(|_| Error::GetKBError)? {
            Ok(Some(row_to_kb(row)?))
        } else {
            Ok(None)
        }
    }

    fn list_kbs(&self, filter: &KbFilter) -> Result<Vec<KbItem>, Error> {
        let conn = self.conn.lock().expect("mutex poisoned");

        let (conditions, bound_params) = build_list_filters(filter);

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };

        let limit_clause = filter
            .limit
            .map(|l| format!(" LIMIT {}", l))
            .unwrap_or_default();
        let offset_clause = filter
            .offset
            .map(|o| format!(" OFFSET {}", o))
            .unwrap_or_default();

        let sql = format!(
            "SELECT KB_ID, KB_KEY, CATEGORY, NAMESPACE, TAG_VALUES \
             FROM kbs{} ORDER BY CREATED_ON DESC{}{}",
            where_clause, limit_clause, offset_clause
        );

        let mut stmt = conn.prepare(&sql).map_err(|_| Error::ListError)?;

        let sql_params: Vec<&dyn rusqlite::types::ToSql> = bound_params
            .iter()
            .map(|s| s as &dyn rusqlite::types::ToSql)
            .collect();

        let items = stmt
            .query_map(sql_params.as_slice(), |row| {
                Ok(row_to_kb_item(
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .map_err(|_| Error::ListError)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| Error::ListError)?;

        Ok(items)
    }

    fn search_kbs(&self, filter: &KbFilter) -> Result<Vec<KbItem>, Error> {
        let keyword = match &filter.keyword {
            Some(k) if !k.is_empty() => format!("{}*", k),
            _ => return Ok(vec![]),
        };

        let conn = self.conn.lock().expect("mutex poisoned");

        let sql = "SELECT k.KB_ID, k.KB_KEY, k.CATEGORY, k.NAMESPACE, k.TAG_VALUES \
                   FROM kbs k \
                   JOIN tags_idx ON tags_idx.rowid = k.INTERNAL_ID \
                   WHERE tags_idx MATCH ?1 \
                   ORDER BY k.CREATED_ON DESC";

        let mut stmt = conn.prepare(sql).map_err(|_| Error::SearchError)?;

        let items = stmt
            .query_map(params![keyword], |row| {
                Ok(row_to_kb_item(
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .map_err(|_| Error::SearchError)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| Error::SearchError)?;

        Ok(items)
    }

    fn save_kb(&self, kb: &Kb) -> Result<(), Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        conn.execute(
            "INSERT INTO kbs (KB_ID, KB_KEY, KB_VALUE, NOTES, CATEGORY, NAMESPACE,
                              REFERENCE, TAG_VALUES, CREATED_ON)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                kb.id,
                kb.key,
                kb.value,
                kb.notes,
                kb.category,
                kb.namespace,
                kb.reference,
                kb.tags_as_string(),
                kb.created_on,
            ],
        )
        .map_err(|_| Error::CreateKBError)?;
        Ok(())
    }

    fn update_kb(&self, kb: &Kb) -> Result<bool, Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        let rows = conn
            .execute(
                "UPDATE kbs SET KB_KEY=?1, KB_VALUE=?2, NOTES=?3, CATEGORY=?4,
                              NAMESPACE=?5, REFERENCE=?6, TAG_VALUES=?7
                 WHERE KB_ID=?8",
                params![
                    kb.key,
                    kb.value,
                    kb.notes,
                    kb.category,
                    kb.namespace,
                    kb.reference,
                    kb.tags_as_string(),
                    kb.id,
                ],
            )
            .map_err(|_| Error::UpdateKBError)?;
        Ok(rows > 0)
    }

    fn delete_kb(&self, id: &str) -> Result<bool, Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        let rows = conn
            .execute("DELETE FROM kbs WHERE KB_ID=?1", params![id])
            .map_err(|_| Error::DeleteKBError)?;
        Ok(rows > 0)
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn row_to_kb(row: &rusqlite::Row) -> Result<Kb, Error> {
    let tag_values: String = row.get(7).map_err(|_| Error::GetKBError)?;
    Ok(Kb {
        id: row.get(0).map_err(|_| Error::GetKBError)?,
        key: row.get(1).map_err(|_| Error::GetKBError)?,
        value: row.get(2).map_err(|_| Error::GetKBError)?,
        notes: row.get(3).map_err(|_| Error::GetKBError)?,
        category: row.get(4).map_err(|_| Error::GetKBError)?,
        namespace: row.get(5).map_err(|_| Error::GetKBError)?,
        reference: row.get(6).map_err(|_| Error::GetKBError)?,
        tags: if tag_values.is_empty() {
            vec![]
        } else {
            tag_values.split_whitespace().map(str::to_string).collect()
        },
        created_on: row.get(8).map_err(|_| Error::GetKBError)?,
    })
}

/// Builds the WHERE conditions and bound parameter list for `list_kbs`.
/// Translated from Go kbcli `buildSQLFilters`.
fn build_list_filters(filter: &KbFilter) -> (Vec<String>, Vec<String>) {
    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<String> = Vec::new();
    let mut idx = 1usize;

    if let Some(cat) = &filter.category {
        conditions.push(format!("CATEGORY = ?{}", idx));
        params.push(cat.clone());
        idx += 1;
    }

    if let Some(ns) = &filter.namespace {
        conditions.push(format!("NAMESPACE = ?{}", idx));
        params.push(ns.clone());
        idx += 1;
    }

    if let Some(tags) = &filter.tags {
        for tag in tags {
            // Use LIKE for tag substring matching in the space-separated string.
            conditions.push(format!(
                "(TAG_VALUES = ?{idx} OR TAG_VALUES LIKE ?{next1} OR TAG_VALUES LIKE ?{next2} OR TAG_VALUES LIKE ?{next3})",
                idx = idx,
                next1 = idx + 1,
                next2 = idx + 2,
                next3 = idx + 3
            ));
            params.push(tag.clone());
            params.push(format!("{} %", tag));
            params.push(format!("% {}", tag));
            params.push(format!("% {} %", tag));
            idx += 4;
        }
    }

    (conditions, params)
}

// ---------------------------------------------------------------------------
// VectorStore implementation
// ---------------------------------------------------------------------------

impl VectorStore for SqliteStore {
    fn initialize_vectors(&self, dimensions: usize) -> Result<(), Error> {
        let ddl = CREATE_EMBEDDINGS_TABLE_TPL.replace("{}", &dimensions.to_string());
        let conn = self.conn.lock().expect("mutex poisoned");
        conn.execute_batch(&ddl)
            .map_err(|e| Error::VectorStoreInitError(e.to_string()))?;
        Ok(())
    }

    fn save_embedding(&self, input: &EmbeddingInput, embedding: &[f32]) -> Result<(), Error> {
        let bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
        let conn = self.conn.lock().expect("mutex poisoned");
        conn.execute(DELETE_EMBEDDING, params![input.kb_id])
            .map_err(|_| Error::VectorSearchError)?;
        conn.execute(INSERT_EMBEDDING, params![input.kb_id, bytes])
            .map_err(|e| Error::VectorStoreInitError(e.to_string()))?;
        Ok(())
    }

    fn delete_embedding(&self, kb_id: &str) -> Result<(), Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        conn.execute(DELETE_EMBEDDING, params![kb_id])
            .map_err(|_| Error::VectorSearchError)?;
        Ok(())
    }

    fn search_similar(
        &self,
        query: &SemanticQuery,
        embedding: &[f32],
    ) -> Result<Vec<ScoredKbItem>, Error> {
        let limit = query.limit.unwrap_or(10);
        let bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
        let conn = self.conn.lock().expect("mutex poisoned");

        // Step 1: KNN search — vec0 MATCH queries do not support JOINs
        let mut knn_stmt = conn
            .prepare(SEARCH_KNN)
            .map_err(|_| Error::VectorSearchError)?;
        let knn_rows: Vec<(String, f32)> = knn_stmt
            .query_map(params![bytes, limit], |row| {
                let kb_id: String = row.get(0)?;
                let distance: f64 = row.get(1)?;
                Ok((kb_id, distance as f32))
            })
            .map_err(|_| Error::VectorSearchError)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| Error::VectorSearchError)?;

        if knn_rows.is_empty() {
            return Ok(vec![]);
        }

        // Step 2: Fetch KB item metadata for each matched id
        let mut results = Vec::with_capacity(knn_rows.len());
        for (kb_id, score) in knn_rows {
            let mut item_stmt = conn
                .prepare(GET_KB_ITEM_BY_ID)
                .map_err(|_| Error::VectorSearchError)?;
            let item = item_stmt
                .query_row(params![kb_id], |row| {
                    let tag_values: String = row.get(4).unwrap_or_default();
                    Ok(row_to_kb_item(
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        tag_values,
                    ))
                })
                .ok();
            if let Some(item) = item {
                results.push(ScoredKbItem { item, score });
            }
        }

        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Integration tests (in-memory SQLite)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_kb(id: &str, key: &str) -> Kb {
        Kb {
            id: id.to_string(),
            key: key.to_string(),
            value: "test value".to_string(),
            notes: String::new(),
            category: "concept".to_string(),
            reference: String::new(),
            namespace: "default".to_string(),
            tags: vec!["rust".to_string(), "memory".to_string()],
            created_on: "2026-01-01T00:00:00+0000".to_string(),
        }
    }

    fn initialized_store() -> SqliteStore {
        let store = SqliteStore::in_memory().expect("in-memory store");
        store.initialize().expect("initialize");
        store
    }

    #[test]
    fn initialize_is_idempotent() {
        let store = SqliteStore::in_memory().unwrap();
        assert!(store.initialize().is_ok());
        assert!(store.initialize().is_ok());
    }

    #[test]
    fn save_and_get_by_id() {
        let store = initialized_store();
        let kb = make_kb("id-1", "rust-ownership");
        store.save_kb(&kb).unwrap();
        let fetched = store.get_kb_by_id("id-1").unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().key, "rust-ownership");
    }

    #[test]
    fn save_and_get_by_key() {
        let store = initialized_store();
        let kb = make_kb("id-1", "rust-ownership");
        store.save_kb(&kb).unwrap();
        let fetched = store.get_kb_by_key("rust-ownership").unwrap();
        assert!(fetched.is_some());
    }

    #[test]
    fn get_by_id_returns_none_for_missing() {
        let store = initialized_store();
        assert_eq!(store.get_kb_by_id("no-such").unwrap(), None);
    }

    #[test]
    fn list_all_returns_saved_entries() {
        let store = initialized_store();
        store.save_kb(&make_kb("id-1", "key-a")).unwrap();
        store.save_kb(&make_kb("id-2", "key-b")).unwrap();
        let filter = KbFilter::default();
        let items = store.list_kbs(&filter).unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn list_filters_by_category() {
        let store = initialized_store();
        let mut kb1 = make_kb("id-1", "key-a");
        kb1.category = "bookmark".to_string();
        let kb2 = make_kb("id-2", "key-b"); // category = "concept"
        store.save_kb(&kb1).unwrap();
        store.save_kb(&kb2).unwrap();

        let filter = KbFilter {
            category: Some("bookmark".to_string()),
            ..Default::default()
        };
        let items = store.list_kbs(&filter).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].key, "key-a");
    }

    #[test]
    fn update_kb_changes_value() {
        let store = initialized_store();
        let mut kb = make_kb("id-1", "rust-ownership");
        store.save_kb(&kb).unwrap();
        kb.value = "updated value".to_string();
        assert!(store.update_kb(&kb).unwrap());
        let fetched = store.get_kb_by_id("id-1").unwrap().unwrap();
        assert_eq!(fetched.value, "updated value");
    }

    #[test]
    fn delete_kb_removes_entry() {
        let store = initialized_store();
        store.save_kb(&make_kb("id-1", "rust-ownership")).unwrap();
        assert!(store.delete_kb("id-1").unwrap());
        assert_eq!(store.get_kb_by_id("id-1").unwrap(), None);
    }

    #[test]
    fn delete_kb_returns_false_for_missing() {
        let store = initialized_store();
        assert!(!store.delete_kb("no-such").unwrap());
    }

    #[test]
    fn search_kbs_via_fts5() {
        let store = initialized_store();
        let mut kb = make_kb("id-1", "rust-ownership");
        kb.tags = vec!["memory".to_string(), "rust".to_string()];
        store.save_kb(&kb).unwrap();

        let filter = KbFilter {
            keyword: Some("memo".to_string()),
            ..Default::default()
        };
        let results = store.search_kbs(&filter).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "rust-ownership");
    }

    fn initialized_store_with_vectors(dims: usize) -> SqliteStore {
        let store = SqliteStore::in_memory().expect("in-memory store");
        store.initialize().expect("initialize");
        store.initialize_vectors(dims).expect("initialize_vectors");
        store
    }

    #[test]
    fn initialize_vectors_is_idempotent() {
        let store = SqliteStore::in_memory().unwrap();
        store.initialize().unwrap();
        assert!(store.initialize_vectors(4).is_ok());
        assert!(store.initialize_vectors(4).is_ok());
    }

    #[test]
    fn save_and_delete_embedding() {
        let store = initialized_store_with_vectors(4);
        let kb = make_kb("id-1", "rust-ownership");
        store.save_kb(&kb).unwrap();

        let input = EmbeddingInput {
            kb_id: "id-1".to_string(),
            text: "rust ownership concept".to_string(),
        };
        let embedding = vec![0.1f32, 0.2, 0.3, 0.4];
        store.save_embedding(&input, &embedding).unwrap();
        assert!(store.delete_embedding("id-1").is_ok());
    }

    #[test]
    fn search_similar_returns_closest_entry() {
        let store = initialized_store_with_vectors(4);
        let kb = make_kb("id-1", "rust-ownership");
        store.save_kb(&kb).unwrap();

        let input = EmbeddingInput {
            kb_id: "id-1".to_string(),
            text: "rust ownership".to_string(),
        };
        let embedding = vec![1.0f32, 0.0, 0.0, 0.0];
        store.save_embedding(&input, &embedding).unwrap();

        let query_embedding = vec![1.0f32, 0.0, 0.0, 0.0];
        let query = SemanticQuery {
            text: "rust ownership".to_string(),
            limit: Some(5),
        };
        let results = store.search_similar(&query, &query_embedding).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item.key, "rust-ownership");
    }
}
