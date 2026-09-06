use serde::Deserialize;

use crate::domain::{
    is_media_category, media_file_path, suggest_tags, ExportKbItem, ExportMediaParams,
    ImportKbItem, KbFilter, KbUpdate, MediaPathParams, NewKb, ScoredKbItem, SemanticQuery,
    TagSuggestionInput,
};
use crate::errors::Error;
use crate::ports::{EmbeddingProvider, KbStore, MediaFetcher, MediaStore, VectorStore};
use crate::service::KBService;

// ---------------------------------------------------------------------------
// Parameter structs (satisfy the 2-param rule)
// ---------------------------------------------------------------------------

pub struct GetParams {
    pub key: Option<String>,
    pub id: Option<String>,
    pub base_dir: String,
}

pub struct ImportParams {
    pub file: String,
    pub failed_items_file: String,
}

pub struct SearchParams {
    pub keyword: Option<String>,
    pub category: Option<String>,
    pub namespace: Option<String>,
    pub tags: Vec<String>,
    pub reference: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

pub struct AddParams {
    pub key: Option<String>,
    pub value: Option<String>,
    pub notes: String,
    pub category: String,
    pub namespace: String,
    pub reference: String,
    pub tags: Vec<String>,
    pub interactive: bool,
    pub parent: Option<String>,
    pub path: Option<String>,
    pub media_url: Option<String>,
}

pub struct ExportParams {
    pub file_name: Option<String>,
    pub folder_output: String,
    pub category: Option<String>,
    pub namespace: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
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

pub fn handle_add<
    S: KbStore,
    V: VectorStore,
    E: EmbeddingProvider,
    M: MediaStore,
    F: MediaFetcher,
>(
    svc: &KBService<S, V, E, M, F>,
    params: AddParams,
) -> Result<(), Error> {
    let new_kb = if params.interactive {
        build_new_kb_interactive(params)?
    } else {
        build_new_kb_non_interactive(params)?
    };
    let decision = confirm_or_adjust(new_kb)?;
    let new_kb = match decision {
        AddDecision::Cancel => {
            println!("Cancelled.");
            return Ok(());
        }
        AddDecision::Save(kb) => *kb,
    };
    let kb = svc.add_kb(new_kb)?;
    println!("--- Created successfully ---");
    print!("{kb}");
    let suggestion_input = TagSuggestionInput {
        text_fields: vec![
            kb.key.clone(),
            kb.value.clone(),
            kb.notes.clone(),
            kb.reference.clone(),
            kb.category.clone(),
            kb.namespace.clone(),
        ],
        existing_tags: kb.tags.clone(),
    };
    let suggestions = suggest_tags(&suggestion_input, 5);
    if !suggestions.is_empty() {
        println!("Suggested tags : {}", suggestions.join(", "));
    }
    Ok(())
}

fn resolve_media_url(category: &str, provided: Option<String>) -> Result<Option<String>, Error> {
    if is_media_category(category) {
        match provided.filter(|u| !u.is_empty()) {
            Some(u) => Ok(Some(u)),
            None => {
                let input = prompt_for("Media URL or file path (required for media)", true)?;
                Ok(Some(input))
            }
        }
    } else {
        Ok(provided)
    }
}

fn build_new_kb_non_interactive(params: AddParams) -> Result<NewKb, Error> {
    let key = params
        .key
        .ok_or_else(|| Error::MissingRequiredField("key".to_string()))?;
    let value = params
        .value
        .ok_or_else(|| Error::MissingRequiredField("value".to_string()))?;
    let reference = if params.reference.is_empty() {
        prompt_for("Reference (optional)", false)?
    } else {
        params.reference
    };
    let tags = if params.tags.is_empty() {
        let input = prompt_for("Tags comma-separated (optional)", false)?;
        parse_tags(&input)
    } else {
        params.tags
    };
    let path = params.path.filter(|p| !p.is_empty());
    let media_url = resolve_media_url(&params.category, params.media_url)?;
    Ok(NewKb {
        key,
        value,
        notes: params.notes,
        category: params.category,
        reference,
        namespace: params.namespace,
        tags,
        parent: params.parent,
        path,
        media_url,
        media_extension: None,
    })
}

fn build_new_kb_interactive(params: AddParams) -> Result<NewKb, Error> {
    let key = params
        .key
        .filter(|s| !s.is_empty())
        .map_or_else(|| prompt_for("Key", true), Ok)?;
    let value = params
        .value
        .filter(|s| !s.is_empty())
        .map_or_else(|| prompt_for("Value", true), Ok)?;
    let notes = if params.notes.is_empty() {
        prompt_for("Notes (optional)", false)?
    } else {
        params.notes
    };
    let category = if params.category.is_empty() {
        prompt_for("Category (optional)", false)?
    } else {
        params.category
    };
    let namespace = if params.namespace.is_empty() {
        prompt_for("Namespace (optional)", false)?
    } else {
        params.namespace
    };
    let reference = if params.reference.is_empty() {
        prompt_for("Reference (optional)", false)?
    } else {
        params.reference
    };
    let tags = if params.tags.is_empty() {
        let tags_input = prompt_for("Tags comma-separated (optional)", false)?;
        parse_tags(&tags_input)
    } else {
        params.tags
    };
    let path = match params.path.filter(|p| !p.is_empty()) {
        Some(p) => Some(p),
        None => {
            let input = prompt_for("Path (optional, e.g. /personal/rust)", false)?;
            if input.is_empty() {
                None
            } else {
                Some(input)
            }
        }
    };
    let media_url = resolve_media_url(&category, params.media_url)?;
    Ok(NewKb {
        key,
        value,
        notes,
        category,
        reference,
        namespace,
        tags,
        parent: params.parent,
        path,
        media_url,
        media_extension: None,
    })
}

// ---------------------------------------------------------------------------
// Add-flow helpers
// ---------------------------------------------------------------------------

enum AddDecision {
    Save(Box<NewKb>),
    Cancel,
}

fn parse_tags(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn format_preview(kb: &NewKb) -> String {
    let mut s = format!(
        "  key       : {}\n  value     : {}\n  notes     : {}\n  category  : {}\n  namespace : {}\n  reference : {}\n  tags      : {}\n  path      : {}\n  parent    : {}",
        kb.key,
        kb.value,
        kb.notes,
        kb.category,
        kb.namespace,
        kb.reference,
        kb.tags.join(", "),
        kb.path.as_deref().unwrap_or("-"),
        kb.parent.as_deref().unwrap_or("(none)"),
    );
    if let Some(ref url) = kb.media_url {
        s.push_str(&format!("\n  media_url : {}", url));
    }
    s
}

fn confirm_or_adjust(mut new_kb: NewKb) -> Result<AddDecision, Error> {
    loop {
        println!("\n--- Preview ---");
        println!("{}", format_preview(&new_kb));
        println!();
        let choice = prompt_for("Save (s), Cancel (c), or Adjust (a)?", true)?;
        match choice.to_lowercase().as_str() {
            "s" => return Ok(AddDecision::Save(Box::new(new_kb))),
            "c" => return Ok(AddDecision::Cancel),
            "a" => {
                new_kb = adjust_fields(new_kb)?;
            }
            _ => eprintln!("Please enter s, c, or a."),
        }
    }
}

fn adjust_fields(kb: NewKb) -> Result<NewKb, Error> {
    let key = prompt_adjust("key", &kb.key)?;
    let value = prompt_adjust("value", &kb.value)?;
    let notes = prompt_adjust("notes", &kb.notes)?;
    let category = prompt_adjust("category", &kb.category)?;
    let namespace = prompt_adjust("namespace", &kb.namespace)?;
    let reference = prompt_adjust("reference", &kb.reference)?;
    let tags_current = kb.tags.join(", ");
    let tags_input = prompt_adjust("tags (comma-separated)", &tags_current)?;
    let tags = if tags_input.is_empty() {
        kb.tags
    } else {
        parse_tags(&tags_input)
    };
    let path_current = kb.path.as_deref().unwrap_or("");
    let path_input = prompt_adjust("path (optional, e.g. /personal/rust)", path_current)?;
    let path = if path_input.is_empty() {
        None
    } else {
        Some(path_input)
    };
    let parent_current = kb.parent.as_deref().unwrap_or("");
    let parent_input = prompt_adjust("parent UUID (optional)", parent_current)?;
    let parent = if parent_input.is_empty() {
        None
    } else {
        Some(parent_input)
    };
    let media_url = if is_media_category(&category) {
        let current = kb.media_url.as_deref().unwrap_or("");
        let input = prompt_adjust("media URL or file path", current)?;
        if input.is_empty() {
            None
        } else {
            Some(input)
        }
    } else {
        kb.media_url
    };
    Ok(NewKb {
        key,
        value,
        notes,
        category,
        namespace,
        reference,
        tags,
        path,
        parent,
        media_url,
        media_extension: kb.media_extension,
    })
}

fn prompt_adjust(label: &str, current: &str) -> Result<String, Error> {
    eprint!("  {} [{}]: ", label, current);
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .map_err(|e| Error::InteractiveInputError(e.to_string()))?;
    let trimmed = input.trim().to_string();
    if trimmed.is_empty() {
        Ok(current.to_string())
    } else {
        Ok(trimmed)
    }
}

fn prompt_for(label: &str, required: bool) -> Result<String, Error> {
    eprint!("{}: ", label);
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .map_err(|e| Error::InteractiveInputError(e.to_string()))?;
    let trimmed = input.trim().to_string();
    if required && trimmed.is_empty() {
        return Err(Error::MissingRequiredField(label.to_string()));
    }
    Ok(trimmed)
}

pub fn handle_get<
    S: KbStore,
    V: VectorStore,
    E: EmbeddingProvider,
    M: MediaStore,
    F: MediaFetcher,
>(
    svc: &KBService<S, V, E, M, F>,
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
        Some(kb) => {
            print!("{kb}");
            if is_media_category(&kb.category) {
                let path = media_file_path(&MediaPathParams {
                    base_dir: &params.base_dir,
                    namespace: &kb.namespace,
                    path: kb.path.as_deref(),
                    key: &kb.key,
                    extension: kb.media_extension.as_deref(),
                });
                println!("Media File : {}", path);
            }
        }
        None => println!("Not found."),
    }
    Ok(())
}

pub fn handle_update<
    S: KbStore,
    V: VectorStore,
    E: EmbeddingProvider,
    M: MediaStore,
    F: MediaFetcher,
>(
    svc: &KBService<S, V, E, M, F>,
    update: KbUpdate,
) -> Result<(), Error> {
    let id = update.id.clone();
    svc.update_kb(update)?;
    println!("Updated: {}", id);
    Ok(())
}

pub fn handle_delete<
    S: KbStore,
    V: VectorStore,
    E: EmbeddingProvider,
    M: MediaStore,
    F: MediaFetcher,
>(
    svc: &KBService<S, V, E, M, F>,
    id: String,
) -> Result<(), Error> {
    match svc.delete_kb(&id) {
        Ok(()) => println!("Deleted: {}", id),
        Err(e @ Error::KBHasChildrenError(_)) => {
            eprintln!("Cannot delete: KB has children. Delete them first: {}", e);
            return Err(e);
        }
        Err(e) => return Err(e),
    }
    Ok(())
}

pub fn handle_search<
    S: KbStore,
    V: VectorStore,
    E: EmbeddingProvider,
    M: MediaStore,
    F: MediaFetcher,
>(
    svc: &KBService<S, V, E, M, F>,
    params: SearchParams,
) -> Result<(), Error> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(ref k) = params.keyword {
        parts.push(format!("keyword={k}"));
    }
    if let Some(ref c) = params.category {
        parts.push(format!("category={c}"));
    }
    if let Some(ref n) = params.namespace {
        parts.push(format!("namespace={n}"));
    }
    if !params.tags.is_empty() {
        parts.push(format!("tags={}", params.tags.join(",")));
    }
    if let Some(ref r) = params.reference {
        parts.push(format!("reference={r}"));
    }
    let filters = if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(" ")
    };

    let filter = KbFilter {
        keyword: params.keyword,
        category: params.category,
        namespace: params.namespace,
        tags: if params.tags.is_empty() {
            None
        } else {
            Some(params.tags)
        },
        reference: params.reference,
        limit: Some(params.limit),
        offset: Some(params.offset),
    };
    let start = std::time::Instant::now();
    let items = svc.get_kbs(filter)?;
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

pub fn handle_ask<
    S: KbStore,
    V: VectorStore,
    E: EmbeddingProvider,
    M: MediaStore,
    F: MediaFetcher,
>(
    svc: &KBService<S, V, E, M, F>,
    query: SemanticQuery,
) -> Result<(), Error> {
    let limit_display = query
        .limit
        .map_or_else(|| "default".to_string(), |l| l.to_string());
    let threshold_display = query
        .threshold
        .map_or_else(|| "none".to_string(), |t| format!("{:.2}", t));
    let start = std::time::Instant::now();
    let results = svc.ask(&query)?;
    let elapsed = start.elapsed();
    println!(
        "Query: \"{}\"  Limit: {}  Threshold: {}",
        query.text, limit_display, threshold_display
    );
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

pub fn handle_quote<
    S: KbStore,
    V: VectorStore,
    E: EmbeddingProvider,
    M: MediaStore,
    F: MediaFetcher,
>(
    svc: &KBService<S, V, E, M, F>,
) -> Result<(), Error> {
    let kb = svc.quote()?;
    println!("\"{}\"", kb.value);
    if !kb.reference.is_empty() {
        println!("  — {}", kb.reference);
    }
    Ok(())
}

pub fn handle_categories<
    S: KbStore,
    V: VectorStore,
    E: EmbeddingProvider,
    M: MediaStore,
    F: MediaFetcher,
>(
    svc: &KBService<S, V, E, M, F>,
    namespace: Option<&str>,
) -> Result<(), Error> {
    let categories = svc.categories(namespace)?;
    for category in &categories {
        println!("{}", category);
    }
    Ok(())
}

pub fn handle_reindex<
    S: KbStore,
    V: VectorStore,
    E: EmbeddingProvider,
    M: MediaStore,
    F: MediaFetcher,
>(
    svc: &KBService<S, V, E, M, F>,
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

pub fn handle_import<
    S: KbStore,
    V: VectorStore,
    E: EmbeddingProvider,
    M: MediaStore,
    F: MediaFetcher,
>(
    svc: &KBService<S, V, E, M, F>,
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

pub fn handle_export<
    S: KbStore,
    V: VectorStore,
    E: EmbeddingProvider,
    M: MediaStore,
    F: MediaFetcher,
>(
    svc: &KBService<S, V, E, M, F>,
    params: ExportParams,
) -> Result<(), Error> {
    std::fs::create_dir_all(&params.folder_output)
        .map_err(|e| Error::ExportError(e.to_string()))?;

    let file_name = params.file_name.unwrap_or_else(default_export_filename);
    let output_path = format!("{}/{}", params.folder_output, file_name);

    let filter = KbFilter {
        category: params.category.clone(),
        namespace: params.namespace.clone(),
        limit: params.limit,
        offset: params.offset,
        ..KbFilter::default()
    };

    let items = svc.export_kbs(filter)?;
    if items.is_empty() {
        println!("No entries to export.");
        return Ok(());
    }

    let content = serialize_export_items(&items)?;
    std::fs::write(&output_path, &content).map_err(|e| Error::ExportError(e.to_string()))?;

    let entry_count = items.len();
    let media_count = svc.export_media(ExportMediaParams {
        target_dir: params.folder_output,
        category: params.category,
        namespace: params.namespace,
        items,
    })?;

    println!("Exported {} entries to {}", entry_count, output_path);
    if media_count > 0 {
        println!("Copied {} media file(s).", media_count);
    }
    Ok(())
}

fn default_export_filename() -> String {
    let now = chrono::Local::now();
    format!("exported-kb-{}.yaml", now.format("%Y-%m-%d-%H-%M-%S"))
}

pub fn handle_version() -> Result<(), Error> {
    println!("version:    {}", KB_VERSION);
    println!("git hash:   {}", KB_GIT_HASH);
    println!("build date: {}", KB_BUILD_DATE);
    Ok(())
}

fn serialize_yaml_docs<T: serde::Serialize>(
    items: &[T],
    map_err: impl Fn(String) -> Error,
) -> Result<String, Error> {
    items
        .iter()
        .map(|item| serde_yaml::to_string(item).map_err(|e| map_err(e.to_string())))
        .collect::<Result<Vec<String>, _>>()
        .map(|docs| docs.join("---\n"))
}

fn serialize_failed_items(items: &[ImportKbItem]) -> Result<String, Error> {
    serialize_yaml_docs(items, Error::WriteFailedItemsError)
}

fn write_failed_items(path: &str, content: &str) -> Result<(), Error> {
    std::fs::write(path, content).map_err(|e| Error::WriteFailedItemsError(e.to_string()))
}

fn serialize_export_items(items: &[ExportKbItem]) -> Result<String, Error> {
    serialize_yaml_docs(items, Error::ExportError)
}

// ---------------------------------------------------------------------------
// Scored-result table helpers
// ---------------------------------------------------------------------------

fn print_scored_table_header() {
    println!(
        "{:<score$}  {:<id$}  {:<key$}  {:<cat$}  {:<ns$}  {:<tags$}",
        "DISTANCE",
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
