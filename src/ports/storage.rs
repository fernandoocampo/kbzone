use std::fmt::Debug;

use crate::domain::{Kb, KbFilter, KbItem};
use crate::errors::Error;

/// Outbound port — all storage adapters must implement this trait.
///
/// The `Clone` bound exists because `SqliteStore` wraps `Arc<Mutex<Connection>>`
/// and must be cheaply cloneable so both `Service` and `SemanticService` can
/// share the same connection without requiring `Arc<dyn KbStore>` at the wiring layer.
pub trait KbStore: Debug + Clone {
    /// Runs DDL to ensure the schema exists (idempotent).
    fn initialize(&self) -> Result<(), Error>;

    fn get_kb_by_id(&self, id: &str) -> Result<Option<Kb>, Error>;

    fn get_kb_by_key(&self, key: &str) -> Result<Option<Kb>, Error>;

    fn get_kbs(&self, filter: &KbFilter) -> Result<Vec<KbItem>, Error>;

    fn save_kb(&self, kb: &Kb) -> Result<(), Error>;

    /// Returns `true` if a row was updated.
    fn update_kb(&self, kb: &Kb) -> Result<bool, Error>;

    /// Returns `true` if a row was deleted.
    fn delete_kb(&self, id: &str) -> Result<bool, Error>;

    /// Returns a random KB entry whose category is `"quote"`.
    fn random_quote(&self) -> Result<Kb, Error>;

    /// Returns the IDs of all KB items whose parent is `parent_id`.
    fn get_children_ids(&self, parent_id: &str) -> Result<Vec<String>, Error>;

    /// Returns full [`Kb`] objects (all fields) matching `filter`, with LIMIT/OFFSET support.
    /// Used by the export path where every field is needed without N+1 queries.
    fn get_kbs_full(&self, filter: &KbFilter) -> Result<Vec<Kb>, Error>;
}
