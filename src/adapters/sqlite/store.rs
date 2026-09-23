use std::sync::{Arc, Mutex, Once};

use rusqlite::{Connection, params};

use crate::domain::{
    EdgeDirection, EmbeddingInput, GraphNode, IncomingEdge, Kb, KbEdge, KbFilter, KbItem,
    OutgoingEdge, RelatedEdges, RelatedQuery, RemoveEdgeParams, ScoredKbItem, SemanticQuery,
    TreeNode, TreeQuery,
};
use crate::errors::Error;
use crate::ports::{KbGraph, KbStore, VectorStore};

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
                             REFERENCE, TAG_VALUES, CREATED_ON, PARENT_KB_ID, KB_PATH, MEDIA_EXTENSION, METADATA \
                             FROM kbs WHERE KB_ID = ?1";

const GET_KB_BY_KEY: &str = "SELECT KB_ID, KB_KEY, KB_VALUE, NOTES, CATEGORY, NAMESPACE, \
                              REFERENCE, TAG_VALUES, CREATED_ON, PARENT_KB_ID, KB_PATH, MEDIA_EXTENSION, METADATA \
                              FROM kbs WHERE KB_KEY = ?1";

const INSERT_KB: &str = "INSERT INTO kbs \
                          (KB_ID, KB_KEY, KB_VALUE, NOTES, CATEGORY, NAMESPACE, REFERENCE, TAG_VALUES, CREATED_ON, PARENT_KB_ID, KB_PATH, MEDIA_EXTENSION, METADATA) \
                          VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)";

const UPDATE_KB: &str = "UPDATE kbs SET KB_KEY=?1, KB_VALUE=?2, NOTES=?3, CATEGORY=?4, \
                          NAMESPACE=?5, REFERENCE=?6, TAG_VALUES=?7, PARENT_KB_ID=?8, KB_PATH=?9, MEDIA_EXTENSION=?10, METADATA=?11 \
                          WHERE KB_ID=?12";

const DELETE_KB: &str = "DELETE FROM kbs WHERE KB_ID=?1";

const GET_RANDOM_BY_CATEGORY: &str = "SELECT KB_ID, KB_KEY, KB_VALUE, NOTES, CATEGORY, NAMESPACE, \
                                      REFERENCE, TAG_VALUES, CREATED_ON, PARENT_KB_ID, KB_PATH, MEDIA_EXTENSION, METADATA \
                                      FROM kbs WHERE LOWER(CATEGORY) = LOWER(?1) \
                                      ORDER BY RANDOM() LIMIT 1";

const GET_RANDOM_BY_CATEGORY_AND_NAMESPACE: &str = "SELECT KB_ID, KB_KEY, KB_VALUE, NOTES, CATEGORY, NAMESPACE, \
                                                     REFERENCE, TAG_VALUES, CREATED_ON, PARENT_KB_ID, KB_PATH, MEDIA_EXTENSION, METADATA \
                                                     FROM kbs WHERE LOWER(CATEGORY) = LOWER(?1) AND NAMESPACE = ?2 \
                                                     ORDER BY RANDOM() LIMIT 1";

const GET_CHILDREN_IDS: &str = "SELECT KB_ID FROM kbs WHERE PARENT_KB_ID = ?1";

const GET_DISTINCT_CATEGORIES: &str =
    "SELECT DISTINCT CATEGORY FROM kbs WHERE CATEGORY != '' ORDER BY CATEGORY ASC";

const GET_DISTINCT_CATEGORIES_BY_NAMESPACE: &str = "SELECT DISTINCT CATEGORY FROM kbs WHERE CATEGORY != '' AND NAMESPACE = ?1 \
     ORDER BY CATEGORY ASC";

const GET_DISTINCT_NAMESPACES: &str =
    "SELECT DISTINCT NAMESPACE FROM kbs WHERE NAMESPACE != '' ORDER BY NAMESPACE ASC";

const GET_DISTINCT_NAMESPACES_FILTERED: &str = "SELECT DISTINCT NAMESPACE FROM kbs WHERE NAMESPACE != '' AND LOWER(NAMESPACE) LIKE ?1 \
     ORDER BY NAMESPACE ASC";

const LIST_KBS_FULL_BASE: &str = "SELECT KB_ID, KB_KEY, KB_VALUE, NOTES, CATEGORY, NAMESPACE, \
                                   REFERENCE, TAG_VALUES, CREATED_ON, PARENT_KB_ID, KB_PATH, MEDIA_EXTENSION, METADATA FROM kbs";

const SEARCH_FTS: &str = "SELECT k.KB_ID, k.KB_KEY, k.CATEGORY, k.NAMESPACE, k.TAG_VALUES \
                           FROM kbs k \
                           JOIN tags_idx ON tags_idx.rowid = k.INTERNAL_ID \
                           WHERE tags_idx MATCH ?1 \
                           ORDER BY k.CREATED_ON DESC";

const SEARCH_FTS_WITH_REF: &str = "SELECT k.KB_ID, k.KB_KEY, k.CATEGORY, k.NAMESPACE, k.TAG_VALUES \
                                    FROM kbs k \
                                    JOIN tags_idx ON tags_idx.rowid = k.INTERNAL_ID \
                                    WHERE tags_idx MATCH ?1 \
                                    AND LOWER(k.REFERENCE) LIKE ?2 \
                                    ORDER BY k.CREATED_ON DESC";

// ---------------------------------------------------------------------------
// Vector DDL / DML constants
// ---------------------------------------------------------------------------

/// Template for vec0 virtual table DDL — `{}` is replaced with the dimension count.
/// `namespace` is a partition key: it physically shards the vector index so a query
/// scoped to one namespace only scans that partition, rather than the whole table.
const CREATE_EMBEDDINGS_TABLE_TPL: &str = "CREATE VIRTUAL TABLE IF NOT EXISTS kb_embeddings \
     USING vec0(kb_id TEXT PRIMARY KEY, namespace TEXT PARTITION KEY, embedding float[{}])";

/// Detects the pre-partition-key `kb_embeddings` schema so `initialize_vectors` can
/// migrate it. `vec0` tables can't be `ALTER`ed to add a partition key column.
const GET_EMBEDDINGS_TABLE_SQL: &str =
    "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'kb_embeddings'";

const DROP_EMBEDDINGS_TABLE: &str = "DROP TABLE kb_embeddings";

const INSERT_EMBEDDING: &str =
    "INSERT INTO kb_embeddings(kb_id, namespace, embedding) VALUES (?1, ?2, ?3)";

const DELETE_EMBEDDING: &str = "DELETE FROM kb_embeddings WHERE kb_id = ?1";

/// Step 1 of semantic search: get nearest kb_ids from the vec0 virtual table.
/// Joined queries with MATCH are not supported by vec0; two separate queries are used.
const SEARCH_KNN: &str =
    "SELECT kb_id, distance FROM kb_embeddings WHERE embedding MATCH ?1 ORDER BY distance LIMIT ?2";

/// Same as `SEARCH_KNN` but scoped to one `namespace` partition — narrows the KNN
/// scan itself rather than filtering results afterward.
const SEARCH_KNN_BY_NAMESPACE: &str = "SELECT kb_id, distance FROM kb_embeddings \
     WHERE namespace = ?1 AND embedding MATCH ?2 ORDER BY distance LIMIT ?3";

/// Step 2 of semantic search: fetch KB item metadata by ID.
const GET_KB_ITEM_BY_ID: &str =
    "SELECT KB_ID, KB_KEY, CATEGORY, NAMESPACE, TAG_VALUES FROM kbs WHERE KB_ID = ?1";

/// `category` is not a partition key (see `SEARCH_KNN`'s doc comment on why), so
/// candidates are over-fetched by this multiplier, capped, before post-filtering.
const CATEGORY_FILTER_OVERFETCH_MULTIPLIER: i64 = 5;
const CATEGORY_FILTER_OVERFETCH_CAP: i64 = 200;

// ---------------------------------------------------------------------------
// Graph DDL constants
// ---------------------------------------------------------------------------

const CREATE_KB_EDGES_TABLE: &str = "
CREATE TABLE IF NOT EXISTS kb_edges (
    INTERNAL_ID   INTEGER PRIMARY KEY AUTOINCREMENT,
    EDGE_ID       TEXT NOT NULL UNIQUE,
    FROM_KB_ID    TEXT NOT NULL,
    TO_KB_ID      TEXT NOT NULL,
    NOTE          TEXT NOT NULL DEFAULT '',
    CREATED_ON    TEXT NOT NULL,
    UNIQUE(FROM_KB_ID, TO_KB_ID)
)";

const CREATE_INDEX_EDGES_FROM: &str =
    "CREATE INDEX IF NOT EXISTS idx_edges_from ON kb_edges(FROM_KB_ID)";

const CREATE_INDEX_EDGES_TO: &str = "CREATE INDEX IF NOT EXISTS idx_edges_to ON kb_edges(TO_KB_ID)";

// No PRAGMA foreign_keys is enabled, so cascade cleanup is done via trigger,
// mirroring the existing kbs_ad trigger (used for tags_idx cleanup). SQLite
// allows multiple AFTER DELETE triggers on one table — this coexists with kbs_ad.
const CREATE_TRIGGER_AD_EDGES: &str = "
CREATE TRIGGER IF NOT EXISTS kbs_ad_edges
AFTER DELETE ON kbs BEGIN
    DELETE FROM kb_edges WHERE FROM_KB_ID = old.KB_ID OR TO_KB_ID = old.KB_ID;
END";

// ---------------------------------------------------------------------------
// Graph CRUD / traversal constants
// ---------------------------------------------------------------------------

const INSERT_EDGE: &str = "INSERT INTO kb_edges \
                            (EDGE_ID, FROM_KB_ID, TO_KB_ID, NOTE, CREATED_ON) \
                            VALUES (?1, ?2, ?3, ?4, ?5)";

const DELETE_EDGE: &str = "DELETE FROM kb_edges WHERE FROM_KB_ID = ?1 AND TO_KB_ID = ?2";

const GET_OUTGOING_EDGES: &str = "SELECT e.EDGE_ID, e.NOTE, e.CREATED_ON, \
                                   k.KB_ID, k.KB_KEY, k.CATEGORY, k.NAMESPACE \
                                   FROM kb_edges e JOIN kbs k ON k.KB_ID = e.TO_KB_ID \
                                   WHERE e.FROM_KB_ID = ?1 ORDER BY e.CREATED_ON ASC";

const GET_INCOMING_EDGES: &str = "SELECT e.EDGE_ID, e.NOTE, e.CREATED_ON, \
                                   k.KB_ID, k.KB_KEY, k.CATEGORY, k.NAMESPACE \
                                   FROM kb_edges e JOIN kbs k ON k.KB_ID = e.FROM_KB_ID \
                                   WHERE e.TO_KB_ID = ?1 ORDER BY e.CREATED_ON ASC";

/// Template for the transitive-traversal recursive CTE. `{child_col}`/`{parent_col}`
/// are swapped by direction: `out` => (TO_KB_ID, FROM_KB_ID), `in` => (FROM_KB_ID, TO_KB_ID).
///
/// Note on termination: the recursive `UNION` here dedups whole rows, not
/// visited node ids — since `depth` is part of every row, a node revisited at
/// a different depth is *not* a duplicate row and *is* re-emitted. The `?2`
/// depth bound (`WHERE w.depth < ?2`) is what actually guarantees termination
/// on a cyclic graph, not the `UNION` by itself.
const GET_TREE_TPL: &str = "
WITH RECURSIVE walk(id, depth, parent_id, note) AS (
    SELECT e.{child_col}, 1, e.{parent_col}, e.NOTE
    FROM kb_edges e WHERE e.{parent_col} = ?1
    UNION
    SELECT e.{child_col}, w.depth + 1, e.{parent_col}, e.NOTE
    FROM kb_edges e JOIN walk w ON e.{parent_col} = w.id
    WHERE w.depth < ?2
)
SELECT w.id, k.KB_KEY, w.depth, w.parent_id, w.note
FROM walk w JOIN kbs k ON k.KB_ID = w.id
ORDER BY w.depth ASC";

/// Template for `get_edges_among_ids` — `{placeholders}` is replaced with a
/// comma-separated list of numbered params (`?1,?2,...,?n`) at runtime.
/// SQLite numbered parameters may be referenced more than once in the same
/// statement and are bound only once, so the same placeholder list is reused
/// for both the FROM and TO clauses: an edge is only returned when both
/// endpoints are present in the bound id list.
const GET_EDGES_AMONG_IDS_TPL: &str = "SELECT EDGE_ID, FROM_KB_ID, TO_KB_ID, NOTE, CREATED_ON \
                                        FROM kb_edges \
                                        WHERE FROM_KB_ID IN ({placeholders}) \
                                        AND TO_KB_ID IN ({placeholders}) \
                                        ORDER BY CREATED_ON ASC";

// ---------------------------------------------------------------------------
// SqliteStore helpers
// ---------------------------------------------------------------------------

/// Registers the sqlite-vec extension as an auto-extension once per process.
/// Must be called before opening any SQLite connection.
fn register_vec_extension() {
    type SqliteInitFn = unsafe extern "C" fn(
        *mut rusqlite::ffi::sqlite3,
        *mut *mut i8,
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

/// Parses one `SEARCH_KNN`/`SEARCH_KNN_BY_NAMESPACE` row into a `(kb_id, distance)` pair.
fn row_to_knn_match(row: &rusqlite::Row) -> rusqlite::Result<(String, f32)> {
    let kb_id: String = row.get(0)?;
    let distance: f64 = row.get(1)?;
    Ok((kb_id, distance as f32))
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
            && !e.to_string().contains("duplicate column name")
        {
            return Err(Error::StorageInitError(e.to_string()));
        }
        // Idempotent migration: add KB_PATH column if it does not exist yet.
        if let Err(e) = conn.execute_batch("ALTER TABLE kbs ADD COLUMN KB_PATH TEXT DEFAULT NULL")
            && !e.to_string().contains("duplicate column name")
        {
            return Err(Error::StorageInitError(e.to_string()));
        }
        // Idempotent migration: add MEDIA_EXTENSION column if it does not exist yet.
        if let Err(e) =
            conn.execute_batch("ALTER TABLE kbs ADD COLUMN MEDIA_EXTENSION TEXT DEFAULT NULL")
            && !e.to_string().contains("duplicate column name")
        {
            return Err(Error::StorageInitError(e.to_string()));
        }
        // Idempotent migration: add METADATA column if it does not exist yet.
        if let Err(e) = conn.execute_batch("ALTER TABLE kbs ADD COLUMN METADATA TEXT DEFAULT NULL")
            && !e.to_string().contains("duplicate column name")
        {
            return Err(Error::StorageInitError(e.to_string()));
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
        let metadata_json =
            serde_json::to_string(&kb.metadata).map_err(|e| Error::CreateKBError(e.to_string()))?;
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
                metadata_json,
            ],
        )
        .map_err(|e| Error::CreateKBError(e.to_string()))?;
        Ok(())
    }

    fn update_kb(&self, kb: &Kb) -> Result<bool, Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        let metadata_json =
            serde_json::to_string(&kb.metadata).map_err(|e| Error::UpdateKBError(e.to_string()))?;
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
                    metadata_json,
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

    fn random_by_category(&self, category: &str, namespace: Option<&str>) -> Result<Kb, Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        let mut stmt = match namespace {
            Some(_) => conn
                .prepare(GET_RANDOM_BY_CATEGORY_AND_NAMESPACE)
                .map_err(|e| Error::RandomError(e.to_string()))?,
            None => conn
                .prepare(GET_RANDOM_BY_CATEGORY)
                .map_err(|e| Error::RandomError(e.to_string()))?,
        };

        let mut rows = match namespace {
            Some(ns) => stmt
                .query(rusqlite::params![category, ns])
                .map_err(|e| Error::RandomError(e.to_string()))?,
            None => stmt
                .query(rusqlite::params![category])
                .map_err(|e| Error::RandomError(e.to_string()))?,
        };

        match rows.next().map_err(|e| Error::RandomError(e.to_string()))? {
            Some(row) => row_to_kb(row),
            None => Err(Error::RandomNotFound(category.to_string())),
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
                stmt.query_map(params![ns], |row| row.get(0))
                    .map_err(|e| Error::ListError(e.to_string()))?
                    .collect::<Result<Vec<String>, _>>()
                    .map_err(|e| Error::ListError(e.to_string()))?
            }
            None => {
                let mut stmt = conn
                    .prepare(GET_DISTINCT_CATEGORIES)
                    .map_err(|e| Error::ListError(e.to_string()))?;
                stmt.query_map([], |row| row.get(0))
                    .map_err(|e| Error::ListError(e.to_string()))?
                    .collect::<Result<Vec<String>, _>>()
                    .map_err(|e| Error::ListError(e.to_string()))?
            }
        };
        Ok(categories)
    }

    fn get_namespaces(&self, filter: Option<&str>) -> Result<Vec<String>, Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        let namespaces = match filter {
            Some(f) if !f.is_empty() => {
                let mut stmt = conn
                    .prepare(GET_DISTINCT_NAMESPACES_FILTERED)
                    .map_err(|e| Error::ListError(e.to_string()))?;
                stmt.query_map(
                    rusqlite::params![format!("%{}%", f.to_lowercase())],
                    |row| row.get(0),
                )
                .map_err(|e| Error::ListError(e.to_string()))?
                .collect::<Result<Vec<String>, _>>()
                .map_err(|e| Error::ListError(e.to_string()))?
            }
            _ => {
                let mut stmt = conn
                    .prepare(GET_DISTINCT_NAMESPACES)
                    .map_err(|e| Error::ListError(e.to_string()))?;
                stmt.query_map([], |row| row.get(0))
                    .map_err(|e| Error::ListError(e.to_string()))?
                    .collect::<Result<Vec<String>, _>>()
                    .map_err(|e| Error::ListError(e.to_string()))?
            }
        };
        Ok(namespaces)
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
    let metadata_raw: Option<String> = row.get(12).map_err(|e| Error::GetKBError(e.to_string()))?;
    let metadata: std::collections::BTreeMap<String, String> = match metadata_raw {
        Some(s) if !s.trim().is_empty() => {
            serde_json::from_str(&s).map_err(|e| Error::GetKBError(e.to_string()))?
        }
        _ => std::collections::BTreeMap::new(),
    };
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
        metadata,
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

    if let Some(r) = &filter.reference
        && !r.is_empty()
    {
        conditions.push(format!("LOWER(REFERENCE) LIKE ?{}", idx));
        params.push(format!("%{}%", r.to_lowercase()));
    }

    (conditions, params)
}

// ---------------------------------------------------------------------------
// VectorStore implementation
// ---------------------------------------------------------------------------

impl VectorStore for SqliteStore {
    fn initialize_vectors(&self, dimensions: usize) -> Result<(), Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        let existing_sql: Option<String> = conn
            .query_row(GET_EMBEDDINGS_TABLE_SQL, [], |row| row.get(0))
            .ok();
        if let Some(sql) = existing_sql
            && !sql.to_lowercase().contains("partition key")
        {
            eprintln!(
                "Migrating kb_embeddings to the new namespace-partitioned schema — \
                 run `kb reindex` afterward to rebuild embeddings."
            );
            conn.execute(DROP_EMBEDDINGS_TABLE, [])
                .map_err(|e| Error::VectorStoreInitError(e.to_string()))?;
        }

        let ddl = CREATE_EMBEDDINGS_TABLE_TPL.replace("{}", &dimensions.to_string());
        conn.execute_batch(&ddl)
            .map_err(|e| Error::VectorStoreInitError(e.to_string()))?;
        Ok(())
    }

    fn save_embedding(&self, input: &EmbeddingInput, embedding: &[f32]) -> Result<(), Error> {
        let bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
        let conn = self.conn.lock().expect("mutex poisoned");
        conn.execute(DELETE_EMBEDDING, params![input.kb_id])
            .map_err(|e| Error::VectorSearchError(e.to_string()))?;
        conn.execute(
            INSERT_EMBEDDING,
            params![input.kb_id, input.namespace, bytes],
        )
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
        let requested_limit = query.limit.unwrap_or(10);
        // `category` is a post-filter (not a partition key), so over-fetch candidates
        // to avoid under-returning; `namespace` is a real partition filter and needs no
        // over-fetch — it already narrows the KNN scan itself.
        let knn_limit = if query.category.is_some() {
            (requested_limit * CATEGORY_FILTER_OVERFETCH_MULTIPLIER)
                .min(CATEGORY_FILTER_OVERFETCH_CAP)
        } else {
            requested_limit
        };
        let bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
        let conn = self.conn.lock().expect("mutex poisoned");

        // Step 1: KNN search — vec0 MATCH queries do not support JOINs
        let knn_rows: Vec<(String, f32)> = match &query.namespace {
            Some(namespace) => {
                let mut knn_stmt = conn
                    .prepare(SEARCH_KNN_BY_NAMESPACE)
                    .map_err(|e| Error::VectorSearchError(e.to_string()))?;
                knn_stmt
                    .query_map(params![namespace, bytes, knn_limit], row_to_knn_match)
                    .map_err(|e| Error::VectorSearchError(e.to_string()))?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| Error::VectorSearchError(e.to_string()))?
            }
            None => {
                let mut knn_stmt = conn
                    .prepare(SEARCH_KNN)
                    .map_err(|e| Error::VectorSearchError(e.to_string()))?;
                knn_stmt
                    .query_map(params![bytes, knn_limit], row_to_knn_match)
                    .map_err(|e| Error::VectorSearchError(e.to_string()))?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| Error::VectorSearchError(e.to_string()))?
            }
        };

        if knn_rows.is_empty() {
            return Ok(vec![]);
        }

        // Step 2: Fetch KB item metadata for each matched id, applying threshold/category filters
        let mut results = Vec::with_capacity(knn_rows.len());
        for (kb_id, score) in knn_rows {
            if let Some(threshold) = query.threshold
                && score > threshold
            {
                continue;
            }
            let mut item_stmt = conn
                .prepare(GET_KB_ITEM_BY_ID)
                .map_err(|e| Error::VectorSearchError(e.to_string()))?;
            let item = item_stmt.query_row(params![kb_id], row_to_kb_item).ok();
            if let Some(item) = item {
                if let Some(category) = &query.category
                    && &item.category != category
                {
                    continue;
                }
                results.push(ScoredKbItem { item, score });
            }
        }
        results.truncate(requested_limit as usize);

        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// KbGraph implementation
// ---------------------------------------------------------------------------

impl KbGraph for SqliteStore {
    fn initialize_graph(&self) -> Result<(), Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        for ddl in &[
            CREATE_KB_EDGES_TABLE,
            CREATE_INDEX_EDGES_FROM,
            CREATE_INDEX_EDGES_TO,
            CREATE_TRIGGER_AD_EDGES,
        ] {
            conn.execute_batch(ddl)
                .map_err(|e| Error::StorageInitError(e.to_string()))?;
        }
        Ok(())
    }

    fn add_edge(&self, edge: &KbEdge) -> Result<(), Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        conn.execute(
            INSERT_EDGE,
            params![
                edge.id,
                edge.from_id,
                edge.to_id,
                edge.note,
                edge.created_on
            ],
        )
        .map_err(|e| {
            if e.to_string().contains("UNIQUE constraint failed") {
                Error::DuplicateEdgeError
            } else {
                Error::AddEdgeError(e.to_string())
            }
        })?;
        Ok(())
    }

    fn remove_edge(&self, params: &RemoveEdgeParams) -> Result<bool, Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        let rows = conn
            .execute(DELETE_EDGE, params![params.from_id, params.to_id])
            .map_err(|e| Error::RemoveEdgeError(e.to_string()))?;
        Ok(rows > 0)
    }

    fn get_related(&self, query: &RelatedQuery) -> Result<RelatedEdges, Error> {
        let conn = self.conn.lock().expect("mutex poisoned");
        let mut result = RelatedEdges::default();

        if query.direction != EdgeDirection::In {
            let mut stmt = conn
                .prepare(GET_OUTGOING_EDGES)
                .map_err(|e| Error::GraphQueryError(e.to_string()))?;
            result.outgoing = stmt
                .query_map(params![query.kb_id], row_to_outgoing_edge)
                .map_err(|e| Error::GraphQueryError(e.to_string()))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| Error::GraphQueryError(e.to_string()))?;
        }

        if query.direction != EdgeDirection::Out {
            let mut stmt = conn
                .prepare(GET_INCOMING_EDGES)
                .map_err(|e| Error::GraphQueryError(e.to_string()))?;
            result.incoming = stmt
                .query_map(params![query.kb_id], row_to_incoming_edge)
                .map_err(|e| Error::GraphQueryError(e.to_string()))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| Error::GraphQueryError(e.to_string()))?;
        }

        Ok(result)
    }

    fn get_tree(&self, query: &TreeQuery) -> Result<Vec<TreeNode>, Error> {
        let (child_col, parent_col) = match query.direction {
            EdgeDirection::Out => ("TO_KB_ID", "FROM_KB_ID"),
            EdgeDirection::In => ("FROM_KB_ID", "TO_KB_ID"),
            EdgeDirection::Both => {
                return Err(Error::GraphQueryError(
                    "direction must be 'out' or 'in' for tree traversal".to_string(),
                ));
            }
        };
        let sql = GET_TREE_TPL
            .replace("{child_col}", child_col)
            .replace("{parent_col}", parent_col);

        let conn = self.conn.lock().expect("mutex poisoned");
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| Error::GraphQueryError(e.to_string()))?;
        let nodes = stmt
            .query_map(params![query.kb_id, query.depth], row_to_tree_node)
            .map_err(|e| Error::GraphQueryError(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| Error::GraphQueryError(e.to_string()))?;
        Ok(nodes)
    }

    fn get_edges_among_ids(&self, ids: &[String]) -> Result<Vec<KbEdge>, Error> {
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let placeholders = (1..=ids.len())
            .map(|i| format!("?{}", i))
            .collect::<Vec<_>>()
            .join(",");
        let sql = GET_EDGES_AMONG_IDS_TPL.replace("{placeholders}", &placeholders);

        let conn = self.conn.lock().expect("mutex poisoned");
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| Error::GraphQueryError(e.to_string()))?;

        let sql_params: Vec<&dyn rusqlite::types::ToSql> = ids
            .iter()
            .map(|s| s as &dyn rusqlite::types::ToSql)
            .collect();

        let edges = stmt
            .query_map(sql_params.as_slice(), row_to_kb_edge)
            .map_err(|e| Error::GraphQueryError(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| Error::GraphQueryError(e.to_string()))?;

        Ok(edges)
    }
}

// ---------------------------------------------------------------------------
// Graph row helpers
// ---------------------------------------------------------------------------

fn row_to_outgoing_edge(row: &rusqlite::Row) -> rusqlite::Result<OutgoingEdge> {
    Ok(OutgoingEdge {
        edge_id: row.get(0)?,
        note: row.get(1)?,
        created_on: row.get(2)?,
        to: GraphNode {
            id: row.get(3)?,
            key: row.get(4)?,
            category: row.get(5)?,
            namespace: row.get(6)?,
        },
    })
}

fn row_to_incoming_edge(row: &rusqlite::Row) -> rusqlite::Result<IncomingEdge> {
    Ok(IncomingEdge {
        edge_id: row.get(0)?,
        note: row.get(1)?,
        created_on: row.get(2)?,
        from: GraphNode {
            id: row.get(3)?,
            key: row.get(4)?,
            category: row.get(5)?,
            namespace: row.get(6)?,
        },
    })
}

fn row_to_tree_node(row: &rusqlite::Row) -> rusqlite::Result<TreeNode> {
    Ok(TreeNode {
        id: row.get(0)?,
        key: row.get(1)?,
        depth: row.get(2)?,
        parent_id: row.get(3)?,
        note: row.get(4)?,
    })
}

fn row_to_kb_edge(row: &rusqlite::Row) -> rusqlite::Result<KbEdge> {
    Ok(KbEdge {
        id: row.get(0)?,
        from_id: row.get(1)?,
        to_id: row.get(2)?,
        note: row.get(3)?,
        created_on: row.get(4)?,
    })
}

// ---------------------------------------------------------------------------
// Integration tests (in-memory SQLite)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
