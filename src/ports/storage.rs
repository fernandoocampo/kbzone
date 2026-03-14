use std::fmt::Debug;

use crate::domain::{Kb, KbFilter, KbItem};
use crate::errors::Error;

/// Outbound port — all storage adapters must implement this trait.
pub trait KbStore: Debug + Clone {
    /// Runs DDL to ensure the schema exists (idempotent).
    fn initialize(&self) -> Result<(), Error>;

    fn get_kb_by_id(&self, id: &str) -> Result<Option<Kb>, Error>;

    fn get_kb_by_key(&self, key: &str) -> Result<Option<Kb>, Error>;

    fn list_kbs(&self, filter: &KbFilter) -> Result<Vec<KbItem>, Error>;

    fn search_kbs(&self, filter: &KbFilter) -> Result<Vec<KbItem>, Error>;

    fn save_kb(&self, kb: &Kb) -> Result<(), Error>;

    /// Returns `true` if a row was updated.
    fn update_kb(&self, kb: &Kb) -> Result<bool, Error>;

    /// Returns `true` if a row was deleted.
    fn delete_kb(&self, id: &str) -> Result<bool, Error>;
}
