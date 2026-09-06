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
    CREATED_ON   TEXT NOT NULL,
    KB_PATH      TEXT DEFAULT NULL
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
// CRUD DML constants
// ---------------------------------------------------------------------------

const GET_KB_BY_ID: &str = "SELECT KB_ID, KB_KEY, KB_VALUE, NOTES, CATEGORY, NAMESPACE, \
                             REFERENCE, TAG_VALUES, CREATED_ON, PARENT_KB_ID, KB_PATH, MEDIA_EXTENSION \
                             FROM kbs WHERE KB_ID = ?1";

const GET_KB_BY_KEY: &str = "SELECT KB_ID, KB_KEY, KB_VALUE, NOTES, CATEGORY, NAMESPACE, \
                              REFERENCE, TAG_VALUES, CREATED_ON, PARENT_KB_ID, KB_PATH, MEDIA_EXTENSION \
                              FROM kbs WHERE KB_KEY = ?1";

const INSERT_KB: &str = "INSERT INTO kbs \
                          (KB_ID, KB_KEY, KB_VALUE, NOTES, CATEGORY, NAMESPACE, REFERENCE, TAG_VALUES, CREATED_ON, PARENT_KB_ID, KB_PATH, MEDIA_EXTENSION) \
                          VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)";

const UPDATE_KB: &str = "UPDATE kbs SET KB_KEY=?1, KB_VALUE=?2, NOTES=?3, CATEGORY=?4, \
                          NAMESPACE=?5, REFERENCE=?6, TAG_VALUES=?7, PARENT_KB_ID=?8, KB_PATH=?9, MEDIA_EXTENSION=?10 \
                          WHERE KB_ID=?11";

const DELETE_KB: &str = "DELETE FROM kbs WHERE KB_ID=?1";

const GET_RANDOM_QUOTE: &str = "SELECT KB_ID, KB_KEY, KB_VALUE, NOTES, CATEGORY, NAMESPACE, \
                                 REFERENCE, TAG_VALUES, CREATED_ON, PARENT_KB_ID, KB_PATH, MEDIA_EXTENSION \
                                 FROM kbs WHERE LOWER(CATEGORY) = 'quote' \
                                 ORDER BY RANDOM() LIMIT 1";

const GET_CHILDREN_IDS: &str = "SELECT KB_ID FROM kbs WHERE PARENT_KB_ID = ?1";

const GET_DISTINCT_CATEGORIES: &str =
    "SELECT DISTINCT CATEGORY FROM kbs WHERE CATEGORY != '' ORDER BY CATEGORY ASC";

const GET_DISTINCT_CATEGORIES_BY_NAMESPACE: &str =
    "SELECT DISTINCT CATEGORY FROM kbs WHERE CATEGORY != '' AND NAMESPACE = ?1 \
     ORDER BY CATEGORY ASC";

const LIST_KBS_FULL_BASE: &str = "SELECT KB_ID, KB_KEY, KB_VALUE, NOTES, CATEGORY, NAMESPACE, \
                                   REFERENCE, TAG_VALUES, CREATED_ON, PARENT_KB_ID, KB_PATH, MEDIA_EXTENSION FROM kbs";

const SEARCH_FTS: &str = "SELECT k.KB_ID, k.KB_KEY, k.CATEGORY, k.NAMESPACE, k.TAG_VALUES \
                           FROM kbs k \
                           JOIN tags_idx ON tags_idx.rowid = k.INTERNAL_ID \
                           WHERE tags_idx MATCH ?1 \
                           ORDER BY k.CREATED_ON DESC";

const SEARCH_FTS_WITH_REF: &str =
    "SELECT k.KB_ID, k.KB_KEY, k.CATEGORY, k.NAMESPACE, k.TAG_VALUES \
                                    FROM kbs k \
                                    JOIN tags_idx ON tags_idx.rowid = k.INTERNAL_ID \
                                    WHERE tags_idx MATCH ?1 \
                                    AND LOWER(k.REFERENCE) LIKE ?2 \
                                    ORDER BY k.CREATED_ON DESC";

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
}

// ---------------------------------------------------------------------------
// Helper — parse a row into KbItem
// ---------------------------------------------------------------------------

fn row_to_kb_item(row: &rusqlite::Row) -> rusqlite::Result<KbItem> {
    let tag_values: String = row.get(4).unwrap_or_default();
    Ok(KbItem {
        id: row.get(0)?,
        key: row.get(1)?,
        category: row.get(2)?,
        namespace: row.get(3)?,
        tags: if tag_values.is_empty() {
            vec![]
        } else {
            tag_values.split_whitespace().map(str::to_string).collect()
        },
    })
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
        // Idempotent migration: add PARENT_KB_ID column if it does not exist yet.
        if let Err(e) =
            conn.execute_batch("ALTER TABLE kbs ADD COLUMN PARENT_KB_ID TEXT DEFAULT NULL")
        {
            if !e.to_string().contains("duplicate column name") {
                return Err(Error::StorageInitError(e.to_string()));
            }
        }
        // Idempotent migration: add KB_PATH column if it does not exist yet.
        if let Err(e) = conn.execute_batch("ALTER TABLE kbs ADD COLUMN KB_PATH TEXT DEFAULT NULL") {
            if !e.to_string().contains("duplicate column name") {
                return Err(Error::StorageInitError(e.to_string()));
            }
        }
        // Idempotent migration: add MEDIA_EXTENSION column if it does not exist yet.
        if let Err(e) =
            conn.execute_batch("ALTER TABLE kbs ADD COLUMN MEDIA_EXTENSION TEXT DEFAULT NULL")
        {
            if !e.to_string().contains("duplicate column name") {
                return Err(Error::StorageInitError(e.to_string()));
            }
        }
        Ok(())
    }

    fn get_kb_by_id(&self, id: &str) -> Result<Option<Kb>, Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        let mut stmt = conn
            .prepare(GET_KB_BY_ID)
            .map_err(|e| Error::GetKBError(e.to_string()))?;

        let mut rows = stmt
            .query(params![id])
            .map_err(|e| Error::GetKBError(e.to_string()))?;

        if let Some(row) = rows.next().map_err(|e| Error::GetKBError(e.to_string()))? {
            Ok(Some(row_to_kb(row)?))
        } else {
            Ok(None)
        }
    }

    fn get_kb_by_key(&self, key: &str) -> Result<Option<Kb>, Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        let mut stmt = conn
            .prepare(GET_KB_BY_KEY)
            .map_err(|e| Error::GetKBError(e.to_string()))?;

        let mut rows = stmt
            .query(params![key])
            .map_err(|e| Error::GetKBError(e.to_string()))?;

        if let Some(row) = rows.next().map_err(|e| Error::GetKBError(e.to_string()))? {
            Ok(Some(row_to_kb(row)?))
        } else {
            Ok(None)
        }
    }

    fn get_kbs(&self, filter: &KbFilter) -> Result<Vec<KbItem>, Error> {
        match &filter.keyword {
            Some(k) if !k.is_empty() => self.get_kbs_fts(filter),
            _ => self.get_kbs_list(filter),
        }
    }

    fn save_kb(&self, kb: &Kb) -> Result<(), Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        conn.execute(
            INSERT_KB,
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
                kb.parent,
                kb.path,
                kb.media_extension,
            ],
        )
        .map_err(|e| Error::CreateKBError(e.to_string()))?;
        Ok(())
    }

    fn update_kb(&self, kb: &Kb) -> Result<bool, Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        let rows = conn
            .execute(
                UPDATE_KB,
                params![
                    kb.key,
                    kb.value,
                    kb.notes,
                    kb.category,
                    kb.namespace,
                    kb.reference,
                    kb.tags_as_string(),
                    kb.parent,
                    kb.path,
                    kb.media_extension,
                    kb.id,
                ],
            )
            .map_err(|e| Error::UpdateKBError(e.to_string()))?;
        Ok(rows > 0)
    }

    fn delete_kb(&self, id: &str) -> Result<bool, Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        let rows = conn
            .execute(DELETE_KB, params![id])
            .map_err(|e| Error::DeleteKBError(e.to_string()))?;
        Ok(rows > 0)
    }

    fn random_quote(&self) -> Result<Kb, Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        let mut stmt = conn
            .prepare(GET_RANDOM_QUOTE)
            .map_err(|e| Error::QuoteError(e.to_string()))?;
        let mut rows = stmt
            .query([])
            .map_err(|e| Error::QuoteError(e.to_string()))?;
        match rows.next().map_err(|e| Error::QuoteError(e.to_string()))? {
            Some(row) => row_to_kb(row),
            None => Err(Error::QuoteNotFound),
        }
    }

    fn get_children_ids(&self, parent_id: &str) -> Result<Vec<String>, Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        let mut stmt = conn
            .prepare(GET_CHILDREN_IDS)
            .map_err(|e| Error::GetKBError(e.to_string()))?;
        let ids = stmt
            .query_map(params![parent_id], |row| row.get(0))
            .map_err(|e| Error::GetKBError(e.to_string()))?
            .collect::<Result<Vec<String>, _>>()
            .map_err(|e| Error::GetKBError(e.to_string()))?;
        Ok(ids)
    }

    fn get_categories(&self, namespace: Option<&str>) -> Result<Vec<String>, Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        let categories = match namespace {
            Some(ns) => {
                let mut stmt = conn
                    .prepare(GET_DISTINCT_CATEGORIES_BY_NAMESPACE)
                    .map_err(|e| Error::ListError(e.to_string()))?;
                let rows = stmt
                    .query_map(params![ns], |row| row.get(0))
                    .map_err(|e| Error::ListError(e.to_string()))?
                    .collect::<Result<Vec<String>, _>>()
                    .map_err(|e| Error::ListError(e.to_string()))?;
                rows
            }
            None => {
                let mut stmt = conn
                    .prepare(GET_DISTINCT_CATEGORIES)
                    .map_err(|e| Error::ListError(e.to_string()))?;
                let rows = stmt
                    .query_map([], |row| row.get(0))
                    .map_err(|e| Error::ListError(e.to_string()))?
                    .collect::<Result<Vec<String>, _>>()
                    .map_err(|e| Error::ListError(e.to_string()))?;
                rows
            }
        };
        Ok(categories)
    }

    fn get_kbs_full(&self, filter: &KbFilter) -> Result<Vec<Kb>, Error> {
        let (where_clause, limit_clause, offset_clause, bound_params) =
            build_filter_clauses(filter);

        let sql = format!(
            "{}{} ORDER BY CREATED_ON ASC{}{}",
            LIST_KBS_FULL_BASE, where_clause, limit_clause, offset_clause
        );

        let conn = self.conn.lock().expect("mutex poisoned");
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| Error::ListError(e.to_string()))?;

        let sql_params: Vec<&dyn rusqlite::types::ToSql> = bound_params
            .iter()
            .map(|s| s as &dyn rusqlite::types::ToSql)
            .collect();

        let mut rows = stmt
            .query(sql_params.as_slice())
            .map_err(|e| Error::ListError(e.to_string()))?;

        let mut items = Vec::new();
        while let Some(row) = rows.next().map_err(|e| Error::ListError(e.to_string()))? {
            items.push(row_to_kb(row)?);
        }

        Ok(items)
    }
}

// ---------------------------------------------------------------------------
// SqliteStore private helpers
// ---------------------------------------------------------------------------

impl SqliteStore {
    fn get_kbs_fts(&self, filter: &KbFilter) -> Result<Vec<KbItem>, Error> {
        let keyword = format!("{}*", filter.keyword.as_deref().unwrap_or(""));
        let ref_pattern = filter
            .reference
            .as_deref()
            .filter(|r| !r.is_empty())
            .map(|r| format!("%{}%", r.to_lowercase()));

        let mut sql = if ref_pattern.is_some() {
            SEARCH_FTS_WITH_REF.to_string()
        } else {
            SEARCH_FTS.to_string()
        };

        if let Some(l) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", l));
        }
        if let Some(o) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", o));
        }

        let conn = self.conn.lock().expect("mutex poisoned");
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| Error::SearchError(e.to_string()))?;

        let mut sql_params: Vec<&dyn rusqlite::types::ToSql> = vec![&keyword];
        if let Some(ref r) = ref_pattern {
            sql_params.push(r);
        }

        let items = stmt
            .query_map(sql_params.as_slice(), row_to_kb_item)
            .map_err(|e| Error::SearchError(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| Error::SearchError(e.to_string()))?;
        Ok(items)
    }

    fn get_kbs_list(&self, filter: &KbFilter) -> Result<Vec<KbItem>, Error> {
        let (where_clause, limit_clause, offset_clause, bound_params) =
            build_filter_clauses(filter);

        let sql = format!(
            "SELECT KB_ID, KB_KEY, CATEGORY, NAMESPACE, TAG_VALUES \
             FROM kbs{} ORDER BY CREATED_ON DESC{}{}",
            where_clause, limit_clause, offset_clause
        );

        let conn = self.conn.lock().expect("mutex poisoned");
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| Error::ListError(e.to_string()))?;

        let sql_params: Vec<&dyn rusqlite::types::ToSql> = bound_params
            .iter()
            .map(|s| s as &dyn rusqlite::types::ToSql)
            .collect();

        let items = stmt
            .query_map(sql_params.as_slice(), row_to_kb_item)
            .map_err(|e| Error::ListError(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| Error::ListError(e.to_string()))?;

        Ok(items)
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn row_to_kb(row: &rusqlite::Row) -> Result<Kb, Error> {
    let tag_values: String = row.get(7).map_err(|e| Error::GetKBError(e.to_string()))?;
    Ok(Kb {
        id: row.get(0).map_err(|e| Error::GetKBError(e.to_string()))?,
        key: row.get(1).map_err(|e| Error::GetKBError(e.to_string()))?,
        value: row.get(2).map_err(|e| Error::GetKBError(e.to_string()))?,
        notes: row.get(3).map_err(|e| Error::GetKBError(e.to_string()))?,
        category: row.get(4).map_err(|e| Error::GetKBError(e.to_string()))?,
        namespace: row.get(5).map_err(|e| Error::GetKBError(e.to_string()))?,
        reference: row.get(6).map_err(|e| Error::GetKBError(e.to_string()))?,
        tags: if tag_values.is_empty() {
            vec![]
        } else {
            tag_values.split_whitespace().map(str::to_string).collect()
        },
        created_on: row.get(8).map_err(|e| Error::GetKBError(e.to_string()))?,
        parent: row
            .get::<_, Option<String>>(9)
            .map_err(|e| Error::GetKBError(e.to_string()))?,
        path: row
            .get::<_, Option<String>>(10)
            .map_err(|e| Error::GetKBError(e.to_string()))?,
        media_extension: row
            .get::<_, Option<String>>(11)
            .map_err(|e| Error::GetKBError(e.to_string()))?,
    })
}

/// Builds the WHERE clause string, LIMIT clause, OFFSET clause, and bound parameters
/// for a filtered list query. Returns `(where_clause, limit_clause, offset_clause, params)`.
fn build_filter_clauses(filter: &KbFilter) -> (String, String, String, Vec<String>) {
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
    (where_clause, limit_clause, offset_clause, bound_params)
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

    if let Some(r) = &filter.reference {
        if !r.is_empty() {
            conditions.push(format!("LOWER(REFERENCE) LIKE ?{}", idx));
            params.push(format!("%{}%", r.to_lowercase()));
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
            .map_err(|e| Error::VectorSearchError(e.to_string()))?;
        conn.execute(INSERT_EMBEDDING, params![input.kb_id, bytes])
            .map_err(|e| Error::VectorStoreInitError(e.to_string()))?;
        Ok(())
    }

    fn delete_embedding(&self, kb_id: &str) -> Result<(), Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        conn.execute(DELETE_EMBEDDING, params![kb_id])
            .map_err(|e| Error::VectorSearchError(e.to_string()))?;
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
            .map_err(|e| Error::VectorSearchError(e.to_string()))?;
        let knn_rows: Vec<(String, f32)> = knn_stmt
            .query_map(params![bytes, limit], |row| {
                let kb_id: String = row.get(0)?;
                let distance: f64 = row.get(1)?;
                Ok((kb_id, distance as f32))
            })
            .map_err(|e| Error::VectorSearchError(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| Error::VectorSearchError(e.to_string()))?;

        if knn_rows.is_empty() {
            return Ok(vec![]);
        }

        // Step 2: Fetch KB item metadata for each matched id, applying threshold filter
        let mut results = Vec::with_capacity(knn_rows.len());
        for (kb_id, score) in knn_rows {
            if let Some(threshold) = query.threshold {
                if score > threshold {
                    continue;
                }
            }
            let mut item_stmt = conn
                .prepare(GET_KB_ITEM_BY_ID)
                .map_err(|e| Error::VectorSearchError(e.to_string()))?;
            let item = item_stmt.query_row(params![kb_id], row_to_kb_item).ok();
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
#[path = "store_tests.rs"]
mod tests;
