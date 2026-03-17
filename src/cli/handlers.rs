use serde::Deserialize;

use crate::domain::{ImportKbItem, KbFilter, KbUpdate, NewKb, ScoredKbItem, SemanticQuery};
use crate::errors::Error;
use crate::ports::{EmbeddingProvider, KbStore, VectorStore};
use crate::service::KBService;

// ---------------------------------------------------------------------------
// Parameter structs (satisfy the 2-param rule)
// ---------------------------------------------------------------------------

pub struct ListParams {
    pub category: Option<String>,
    pub namespace: Option<String>,
    pub tags: Vec<String>,
    pub limit: i64,
    pub offset: i64,
}

pub struct GetParams {
    pub key: Option<String>,
    pub id: Option<String>,
}

pub struct ImportParams {
    pub file: String,
    pub failed_items_file: String,
}

// ---------------------------------------------------------------------------
// Version constants (baked in at compile time via Makefile env vars)
// ---------------------------------------------------------------------------

const KB_VERSION: &str = match option_env!("KB_VERSION") {
    Some(v) => v,
    None => "unknown",
};
const KB_BUILD_DATE: &str = match option_env!("KB_BUILD_DATE") {
    Some(v) => v,
    None => "unknown",
};
const KB_GIT_HASH: &str = match option_env!("KB_GIT_HASH") {
    Some(v) => v,
    None => "unknown",
};

// ---------------------------------------------------------------------------
// Column widths for tabular output
// ---------------------------------------------------------------------------

const COL_SCORE: usize = 8;
const COL_ID: usize = 36;
const COL_KEY: usize = 24;
const COL_CAT: usize = 12;
const COL_NS: usize = 14;
const COL_TAGS: usize = 30;

fn print_table_header() {
    println!(
        "{:<id$}  {:<key$}  {:<cat$}  {:<ns$}  {:<tags$}",
        "ID",
        "KEY",
        "CATEGORY",
        "NAMESPACE",
        "TAGS",
        id = COL_ID,
        key = COL_KEY,
        cat = COL_CAT,
        ns = COL_NS,
        tags = COL_TAGS,
    );
    println!(
        "{}",
        "-".repeat(COL_ID + COL_KEY + COL_CAT + COL_NS + COL_TAGS + 8)
    );
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub fn handle_add<S: KbStore, V: VectorStore, E: EmbeddingProvider>(
    svc: &KBService<S, V, E>,
    new_kb: NewKb,
) -> Result<(), Error> {
    let kb = svc.add_kb(new_kb)?;
    println!("Created: {}", kb.id);
    Ok(())
}

pub fn handle_get<S: KbStore, V: VectorStore, E: EmbeddingProvider>(
    svc: &KBService<S, V, E>,
    params: GetParams,
) -> Result<(), Error> {
    let kb = match (params.key, params.id) {
        (Some(k), _) => svc.get_kb_by_key(&k)?,
        (_, Some(i)) => svc.get_kb_by_id(&i)?,
        _ => {
            eprintln!("error: provide --key or --id");
            return Err(Error::GetKBError("no lookup key provided".to_string()));
        }
    };

    match kb {
        Some(kb) => print!("{kb}"),
        None => println!("Not found."),
    }
    Ok(())
}

pub fn handle_list<S: KbStore, V: VectorStore, E: EmbeddingProvider>(
    svc: &KBService<S, V, E>,
    params: ListParams,
) -> Result<(), Error> {
    let filter = KbFilter {
        category: params.category,
        namespace: params.namespace,
        tags: if params.tags.is_empty() {
            None
        } else {
            Some(params.tags)
        },
        limit: Some(params.limit),
        offset: Some(params.offset),
        ..Default::default()
    };
    let mut parts: Vec<String> = Vec::new();
    if let Some(ref c) = filter.category {
        parts.push(format!("category={c}"));
    }
    if let Some(ref n) = filter.namespace {
        parts.push(format!("namespace={n}"));
    }
    if let Some(ref t) = filter.tags {
        parts.push(format!("tags={}", t.join(",")));
    }
    let filters = if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(" ")
    };
    let start = std::time::Instant::now();
    let items = svc.list_kbs(filter)?;
    let elapsed = start.elapsed();
    println!(
        "Offset: {}  Limit: {}  Filters: {}",
        params.offset, params.limit, filters
    );
    println!("Duration: {:.2?}", elapsed);
    println!();
    if items.is_empty() {
        println!("No entries found.");
        return Ok(());
    }
    print_table_header();
    for item in items {
        println!(
            "{:<id$}  {:<key$}  {:<cat$}  {:<ns$}  {:<tags$}",
            item.id,
            item.key,
            item.category,
            item.namespace,
            item.tags.join(", "),
            id = COL_ID,
            key = COL_KEY,
            cat = COL_CAT,
            ns = COL_NS,
            tags = COL_TAGS,
        );
    }
    Ok(())
}

pub fn handle_update<S: KbStore, V: VectorStore, E: EmbeddingProvider>(
    svc: &KBService<S, V, E>,
    update: KbUpdate,
) -> Result<(), Error> {
    let id = update.id.clone();
    svc.update_kb(update)?;
    println!("Updated: {}", id);
    Ok(())
}

pub fn handle_delete<S: KbStore, V: VectorStore, E: EmbeddingProvider>(
    svc: &KBService<S, V, E>,
    id: String,
) -> Result<(), Error> {
    svc.delete_kb(&id)?;
    println!("Deleted: {}", id);
    Ok(())
}

pub fn handle_search<S: KbStore, V: VectorStore, E: EmbeddingProvider>(
    svc: &KBService<S, V, E>,
    keyword: String,
) -> Result<(), Error> {
    let start = std::time::Instant::now();
    let items = svc.search_kbs(&keyword)?;
    let elapsed = start.elapsed();
    println!("Keyword: \"{}\"", keyword);
    println!("Duration: {:.2?}", elapsed);
    println!();
    if items.is_empty() {
        println!("No results for '{}'.", keyword);
        return Ok(());
    }
    print_table_header();
    for item in items {
        println!(
            "{:<id$}  {:<key$}  {:<cat$}  {:<ns$}  {:<tags$}",
            item.id,
            item.key,
            item.category,
            item.namespace,
            item.tags.join(", "),
            id = COL_ID,
            key = COL_KEY,
            cat = COL_CAT,
            ns = COL_NS,
            tags = COL_TAGS,
        );
    }
    Ok(())
}

pub fn handle_ask<S: KbStore, V: VectorStore, E: EmbeddingProvider>(
    svc: &KBService<S, V, E>,
    query: SemanticQuery,
) -> Result<(), Error> {
    let limit_display = query
        .limit
        .map_or_else(|| "default".to_string(), |l| l.to_string());
    let start = std::time::Instant::now();
    let results = svc.ask(&query)?;
    let elapsed = start.elapsed();
    println!("Query: \"{}\"  Limit: {}", query.text, limit_display);
    println!("Duration: {:.2?}", elapsed);
    println!();
    if results.is_empty() {
        println!("No semantic matches found.");
        return Ok(());
    }
    print_scored_table_header();
    for scored in results {
        print_scored_row(&scored);
    }
    Ok(())
}

pub fn handle_quote<S: KbStore, V: VectorStore, E: EmbeddingProvider>(
    svc: &KBService<S, V, E>,
) -> Result<(), Error> {
    let kb = svc.quote()?;
    println!("\"{}\"", kb.value);
    if !kb.reference.is_empty() {
        println!("  — {}", kb.reference);
    }
    Ok(())
}

pub fn handle_reindex<S: KbStore, V: VectorStore, E: EmbeddingProvider>(
    svc: &KBService<S, V, E>,
) -> Result<(), Error> {
    let result = svc.reindex()?;
    println!("Reindexing {} entries...", result.total());
    for (key, _id) in &result.succeeded {
        println!("  [OK] {}", key);
    }
    for (key_or_id, err) in &result.failed {
        eprintln!("  [FAIL] {}: {}", key_or_id, err);
    }
    println!(
        "Done: {} indexed, {} failed.",
        result.succeeded.len(),
        result.failed.len()
    );
    Ok(())
}

pub fn handle_import<S: KbStore, V: VectorStore, E: EmbeddingProvider>(
    svc: &KBService<S, V, E>,
    params: ImportParams,
) -> Result<(), Error> {
    let start = std::time::Instant::now();

    let content =
        std::fs::read_to_string(&params.file).map_err(|e| Error::ImportFileError(e.to_string()))?;

    let items: Vec<ImportKbItem> = serde_yaml::Deserializer::from_str(&content)
        .map(|doc| {
            <ImportKbItem as Deserialize>::deserialize(doc)
                .map_err(|e: serde_yaml::Error| Error::ParseImportFileError(e.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let result = svc.import_kbs(items);
    let saved_count = result.saved.len();
    let failed_count = result.failed.len();

    if !result.failed.is_empty() {
        let failed_items: Vec<ImportKbItem> = result.failed.into_iter().map(|f| f.item).collect();
        match serialize_failed_items(&failed_items) {
            Ok(content) => {
                if let Err(e) = write_failed_items(&params.failed_items_file, &content) {
                    eprintln!("Warning: could not write failed items file: {}", e);
                }
            }
            Err(e) => eprintln!("Warning: could not serialize failed items: {}", e),
        }
    }

    let elapsed = start.elapsed();
    println!("Import complete.");
    println!("  Imported : {}", saved_count);
    println!("  Failed   : {}", failed_count);
    println!("  Duration : {:.2?}", elapsed);
    if failed_count > 0 {
        println!("  Failed items: {}", params.failed_items_file);
    }

    Ok(())
}

pub fn handle_version() -> Result<(), Error> {
    println!("version:    {}", KB_VERSION);
    println!("git hash:   {}", KB_GIT_HASH);
    println!("build date: {}", KB_BUILD_DATE);
    Ok(())
}

fn serialize_failed_items(items: &[ImportKbItem]) -> Result<String, Error> {
    items
        .iter()
        .map(|item| {
            serde_yaml::to_string(item).map_err(|e| Error::WriteFailedItemsError(e.to_string()))
        })
        .collect::<Result<Vec<String>, _>>()
        .map(|docs| docs.join("---\n"))
}

fn write_failed_items(path: &str, content: &str) -> Result<(), Error> {
    std::fs::write(path, content).map_err(|e| Error::WriteFailedItemsError(e.to_string()))
}

// ---------------------------------------------------------------------------
// Scored-result table helpers
// ---------------------------------------------------------------------------

fn print_scored_table_header() {
    println!(
        "{:<score$}  {:<id$}  {:<key$}  {:<cat$}  {:<ns$}  {:<tags$}",
        "SCORE",
        "ID",
        "KEY",
        "CATEGORY",
        "NAMESPACE",
        "TAGS",
        score = COL_SCORE,
        id = COL_ID,
        key = COL_KEY,
        cat = COL_CAT,
        ns = COL_NS,
        tags = COL_TAGS,
    );
    println!(
        "{}",
        "-".repeat(COL_SCORE + COL_ID + COL_KEY + COL_CAT + COL_NS + COL_TAGS + 10)
    );
}

fn print_scored_row(scored: &ScoredKbItem) {
    println!(
        "{:<score$}  {:<id$}  {:<key$}  {:<cat$}  {:<ns$}  {:<tags$}",
        format!("{:.4}", scored.score),
        scored.item.id,
        scored.item.key,
        scored.item.category,
        scored.item.namespace,
        scored.item.tags.join(", "),
        score = COL_SCORE,
        id = COL_ID,
        key = COL_KEY,
        cat = COL_CAT,
        ns = COL_NS,
        tags = COL_TAGS,
    );
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "handlers_tests.rs"]
mod tests;
