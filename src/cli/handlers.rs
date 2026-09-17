use std::collections::HashMap;

use crate::cli::{browser, graph_view};
use crate::domain::{
    AddJsonInput, CategoriesErrorResponse, DeleteConfirmation, DeleteErrorResponse, EdgeDirection,
    ExportDocument, ExportMediaParams, GraphViewParams, ImportDocument, ImportEdgeItem,
    ImportKbItem, KbFilter, KbRelationships, KbUpdate, KbWithRelationships, LinkConfirmation,
    LinkErrorResponse, LinkParams, MediaPathParams, NewKb, OutputFormat, RelatedResult,
    ScoredKbItem, SemanticQuery, TagSuggestionInput, TreeNode, TreeResult, TreeWalkParams,
    build_metadata, format_metadata, is_media_category, media_file_path, parse_metadata_input,
    suggest_tags,
};
use crate::errors::Error;
use crate::ports::{EmbeddingProvider, KbGraph, KbStore, MediaFetcher, MediaStore, VectorStore};
use crate::service::{GraphService, KBService};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Parameter structs (satisfy the 2-param rule)
// ---------------------------------------------------------------------------

pub struct GetParams {
    pub key: Option<String>,
    pub id: Option<String>,
    pub base_dir: String,
    pub out: Option<String>,
    pub with_out_connections: bool,
    pub with_in_connections: bool,
    pub with_all_connections: bool,
}

pub struct UpdateParams {
    pub update: KbUpdate,
    pub out: Option<String>,
}

pub struct DeleteParams {
    pub id: String,
    pub out: Option<String>,
}

pub struct CategoriesParams {
    pub namespace: Option<String>,
    pub out: Option<String>,
}

pub struct ImportParams {
    pub file: String,
    pub failed_items_file: String,
    pub failed_edges_file: String,
}

pub struct SearchParams {
    pub keyword: Option<String>,
    pub category: Option<String>,
    pub namespace: Option<String>,
    pub tags: Vec<String>,
    pub reference: Option<String>,
    pub limit: i64,
    pub offset: i64,
    pub out: Option<String>,
}

pub struct AskParams {
    pub query: SemanticQuery,
    pub out: Option<String>,
}

pub struct AddParams {
    pub key: Option<String>,
    pub value: Option<String>,
    pub notes: String,
    pub category: String,
    pub namespace: String,
    pub reference: String,
    pub tags: Vec<String>,
    pub metadata: Vec<(String, String)>,
    pub interactive: bool,
    pub parent: Option<String>,
    pub path: Option<String>,
    pub media_url: Option<String>,
    pub json: Option<String>,
}

pub struct ExportParams {
    pub file_name: Option<String>,
    pub folder_output: String,
    pub category: Option<String>,
    pub namespace: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Bundles the two services `kb export` needs (`KBService` for entries,
/// `GraphService` for relationships) into a single argument, satisfying the
/// 2-parameter rule for `handle_export`. `S` is shared: the same concrete
/// store type backs both `KbStore` (for `KBService`) and `GraphService`'s
/// `store` field, mirroring how `App::build()` wires
/// `GraphService::new(store.clone(), store)`.
pub struct ExportServices<'a, S, V, E, M, F, G>
where
    S: KbStore,
    V: VectorStore,
    E: EmbeddingProvider,
    M: MediaStore,
    F: MediaFetcher,
    G: KbGraph,
{
    pub svc: &'a KBService<S, V, E, M, F>,
    pub graph_svc: &'a GraphService<S, G>,
}

/// Bundles the two services `kb import` needs (`KBService` for entries,
/// `GraphService` for relationships) into a single argument, satisfying the
/// 2-parameter rule for `handle_import`. Mirrors `ExportServices`.
pub struct ImportServices<'a, S, V, E, M, F, G>
where
    S: KbStore,
    V: VectorStore,
    E: EmbeddingProvider,
    M: MediaStore,
    F: MediaFetcher,
    G: KbGraph,
{
    pub svc: &'a KBService<S, V, E, M, F>,
    pub graph_svc: &'a GraphService<S, G>,
}

/// Bundles the two services `kb get` needs (`KBService` to resolve the
/// entry, `GraphService` to fetch its relationships when requested) into a
/// single argument, satisfying the 2-parameter rule for `handle_get`.
/// Mirrors `ExportServices`/`ImportServices`.
pub struct GetServices<'a, S, V, E, M, F, G>
where
    S: KbStore,
    V: VectorStore,
    E: EmbeddingProvider,
    M: MediaStore,
    F: MediaFetcher,
    G: KbGraph,
{
    pub svc: &'a KBService<S, V, E, M, F>,
    pub graph_svc: &'a GraphService<S, G>,
}

pub struct UnlinkParams {
    pub from: String,
    pub to: String,
}

pub struct RelatedParams {
    pub key_or_id: String,
    pub direction: String,
    pub json: bool,
}

pub struct TreeParams {
    pub key_or_id: String,
    pub direction: String,
    pub depth: i64,
    pub json: bool,
}

pub struct GraphViewCliParams {
    pub key_or_id: String,
    pub direction: String,
    pub depth: i64,
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
    if let Some(json_str) = params.json.clone() {
        assert_no_conflicting_add_flags(&params)?;
        return handle_add_json(svc, &json_str);
    }
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
    let suggestions = suggested_tags_for(
        &[
            &kb.key,
            &kb.value,
            &kb.notes,
            &kb.reference,
            &kb.category,
            &kb.namespace,
        ],
        &kb.tags,
    );
    if !suggestions.is_empty() {
        println!("Suggested tags : {}", suggestions.join(", "));
    }
    Ok(())
}

/// Rejects `--json` combined with any individual `add` field flag. Checked
/// manually (rather than via clap `conflicts_with_all`) because several
/// sibling flags default to `""`/an empty `Vec`, which clap can't reliably
/// distinguish from "the user explicitly passed the default value".
fn assert_no_conflicting_add_flags(params: &AddParams) -> Result<(), Error> {
    let conflicts = params.key.is_some()
        || params.value.is_some()
        || !params.notes.is_empty()
        || !params.category.is_empty()
        || !params.namespace.is_empty()
        || !params.reference.is_empty()
        || !params.tags.is_empty()
        || !params.metadata.is_empty()
        || params.interactive
        || params.parent.is_some()
        || params.path.is_some()
        || params.media_url.is_some();
    if conflicts {
        return Err(Error::ConflictingAddFlags(
            "--json cannot be combined with --key/--value/--notes/--category/--namespace/\
             --reference/--tags/--metadata/--interactive/--parent/--path/--media-url"
                .to_string(),
        ));
    }
    Ok(())
}

/// One-shot, non-interactive path for `kb add --json`: parses, validates and
/// saves the entry, then prints only the created entry as JSON — no prompts,
/// no "Created successfully" header, no tag-suggestion hint.
fn handle_add_json<
    S: KbStore,
    V: VectorStore,
    E: EmbeddingProvider,
    M: MediaStore,
    F: MediaFetcher,
>(
    svc: &KBService<S, V, E, M, F>,
    json_str: &str,
) -> Result<(), Error> {
    let input: AddJsonInput =
        serde_json::from_str(json_str).map_err(|e| Error::InvalidJsonInput(e.to_string()))?;
    let new_kb = NewKb::try_from(input)?;
    let kb = svc.add_kb(new_kb)?;
    let json =
        serde_json::to_string_pretty(&kb).map_err(|e| Error::InvalidJsonInput(e.to_string()))?;
    println!("{json}");
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
        let suggestions = suggested_tags_for(
            &[
                &key,
                &value,
                &params.notes,
                &reference,
                &params.category,
                &params.namespace,
            ],
            &[],
        );
        if !suggestions.is_empty() {
            eprintln!("Suggested tags : {}", suggestions.join(", "));
        }
        let input = prompt_for("Tags comma-separated (optional)", false)?;
        parse_tags(&input)
    } else {
        params.tags
    };
    let metadata = build_metadata(params.metadata)?;
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
        metadata,
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
        let suggestions = suggested_tags_for(
            &[&key, &value, &notes, &category, &namespace, &reference],
            &[],
        );
        if !suggestions.is_empty() {
            eprintln!("Suggested tags : {}", suggestions.join(", "));
        }
        let tags_input = prompt_for("Tags comma-separated (optional)", false)?;
        parse_tags(&tags_input)
    } else {
        params.tags
    };
    let metadata = if params.metadata.is_empty() {
        let input = prompt_for(
            "Metadata as key=value pairs, comma-separated (optional)",
            false,
        )?;
        if input.is_empty() {
            BTreeMap::new()
        } else {
            build_metadata(parse_metadata_input(&input))?
        }
    } else {
        build_metadata(params.metadata)?
    };
    let path = match params.path.filter(|p| !p.is_empty()) {
        Some(p) => Some(p),
        None => {
            let input = prompt_for("Path (optional, e.g. /personal/rust)", false)?;
            if input.is_empty() { None } else { Some(input) }
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
        metadata,
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

/// Computes up to 5 tag suggestions from the given text fields, excluding
/// any tag already in `existing_tags`.
fn suggested_tags_for(fields: &[&str], existing_tags: &[String]) -> Vec<String> {
    let suggestion_input = TagSuggestionInput {
        text_fields: fields.iter().map(|s| s.to_string()).collect(),
        existing_tags: existing_tags.to_vec(),
    };
    suggest_tags(&suggestion_input, 5)
}

fn format_preview(kb: &NewKb) -> String {
    let mut s = format!(
        "  key       : {}\n  value     : {}\n  notes     : {}\n  category  : {}\n  namespace : {}\n  reference : {}\n  tags      : {}\n  metadata  : {}\n  path      : {}\n  parent    : {}",
        kb.key,
        kb.value,
        kb.notes,
        kb.category,
        kb.namespace,
        kb.reference,
        kb.tags.join(", "),
        format_metadata(&kb.metadata),
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
    let metadata_current = format_metadata(&kb.metadata);
    let metadata_input = prompt_adjust("metadata (comma-separated key=value)", &metadata_current)?;
    let metadata = if metadata_input.is_empty() {
        kb.metadata
    } else {
        build_metadata(parse_metadata_input(&metadata_input))?
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
        if input.is_empty() { None } else { Some(input) }
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
        metadata,
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

/// The three `kb get` relationship flags, bundled to satisfy the 2-param
/// rule for [`resolve_relationship_direction`].
struct RelationshipFlags {
    with_out: bool,
    with_in: bool,
    with_all: bool,
}

/// Maps `kb get`'s three relationship flags to the direction to fetch, or
/// `None` if the caller asked for no relationship info at all (the
/// default — preserves `kb get`'s original output exactly).
/// `--with-all-connections` always wins, regardless of the other two flags.
fn resolve_relationship_direction(flags: RelationshipFlags) -> Option<EdgeDirection> {
    match (flags.with_out, flags.with_in, flags.with_all) {
        (_, _, true) => Some(EdgeDirection::Both),
        (true, true, false) => Some(EdgeDirection::Both),
        (true, false, false) => Some(EdgeDirection::Out),
        (false, true, false) => Some(EdgeDirection::In),
        (false, false, false) => None,
    }
}

pub fn handle_get<S, V, E, M, F, G>(
    services: GetServices<S, V, E, M, F, G>,
    params: GetParams,
) -> Result<(), Error>
where
    S: KbStore,
    V: VectorStore,
    E: EmbeddingProvider,
    M: MediaStore,
    F: MediaFetcher,
    G: KbGraph,
{
    let format: Option<OutputFormat> = params
        .out
        .as_deref()
        .map(str::parse)
        .transpose()
        .map_err(Error::GetKBError)?;

    let kb = match (params.key, params.id) {
        (Some(k), _) => services.svc.get_kb_by_key(&k)?,
        (_, Some(i)) => services.svc.get_kb_by_id(&i)?,
        _ => {
            eprintln!("error: provide --key or --id");
            return Err(Error::GetKBError("no lookup key provided".to_string()));
        }
    };

    let kb = match kb {
        Some(kb) => kb,
        None => {
            println!("Not found.");
            return Ok(());
        }
    };

    let direction = resolve_relationship_direction(RelationshipFlags {
        with_out: params.with_out_connections,
        with_in: params.with_in_connections,
        with_all: params.with_all_connections,
    });
    // `RelatedResult` (with its `node` field) is what `render_related` needs
    // for the plain-text branch; the JSON/YAML branches drop `node` via
    // `KbRelationships::from` since the root entry is already flattened at
    // the top level of `KbWithRelationships` — repeating it under
    // `relationships.node` would just duplicate those fields.
    let related_result = direction
        .map(|d| services.graph_svc.related(&kb.id, d))
        .transpose()?;

    match format {
        Some(OutputFormat::Json) => {
            let dto = KbWithRelationships {
                kb,
                relationships: related_result.map(KbRelationships::from),
            };
            let json =
                serde_json::to_string_pretty(&dto).map_err(|e| Error::GetKBError(e.to_string()))?;
            println!("{json}");
        }
        Some(OutputFormat::Yaml) => {
            let dto = KbWithRelationships {
                kb,
                relationships: related_result.map(KbRelationships::from),
            };
            let yaml = serde_yaml::to_string(&dto).map_err(|e| Error::GetKBError(e.to_string()))?;
            print!("{yaml}");
        }
        None => {
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
            if let (Some(d), Some(result)) = (direction, &related_result) {
                print!("{}", render_related(result, d));
            }
        }
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
    params: UpdateParams,
) -> Result<(), Error> {
    let format: Option<OutputFormat> = params
        .out
        .as_deref()
        .map(str::parse)
        .transpose()
        .map_err(Error::UpdateKBError)?;

    let id = params.update.id.clone();
    svc.update_kb(params.update)?;

    match format {
        Some(OutputFormat::Json) => {
            let kb = svc.get_kb_by_id(&id)?.ok_or(Error::KBNotFound)?;
            let json = serde_json::to_string_pretty(&kb)
                .map_err(|e| Error::UpdateKBError(e.to_string()))?;
            println!("{json}");
        }
        Some(OutputFormat::Yaml) => {
            let kb = svc.get_kb_by_id(&id)?.ok_or(Error::KBNotFound)?;
            let yaml =
                serde_yaml::to_string(&kb).map_err(|e| Error::UpdateKBError(e.to_string()))?;
            print!("{yaml}");
        }
        None => {
            println!("Updated: {}", id);
        }
    }
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
    params: DeleteParams,
) -> Result<(), Error> {
    let json_output = match params.out.as_deref() {
        Some("json") => true,
        Some(other) => {
            return Err(Error::DeleteKBError(format!(
                "invalid output format: {other} (expected json)"
            )));
        }
        None => false,
    };

    match svc.delete_kb(&params.id) {
        Ok(()) => {
            if json_output {
                let confirmation = DeleteConfirmation {
                    id: params.id.clone(),
                    deleted: true,
                };
                let json = serde_json::to_string_pretty(&confirmation)
                    .map_err(|e| Error::DeleteKBError(e.to_string()))?;
                println!("{json}");
            } else {
                println!("Deleted: {}", params.id);
            }
            Ok(())
        }
        Err(e) => {
            if json_output {
                let err_resp = DeleteErrorResponse {
                    error: e.to_string(),
                };
                if let Ok(json) = serde_json::to_string_pretty(&err_resp) {
                    println!("{json}");
                }
            } else if let Error::KBHasChildrenError(_) = &e {
                eprintln!("Cannot delete: KB has children. Delete them first: {}", e);
            }
            Err(e)
        }
    }
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
    let format: Option<OutputFormat> = params
        .out
        .as_deref()
        .map(str::parse)
        .transpose()
        .map_err(Error::SearchError)?;

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

    match format {
        Some(OutputFormat::Json) => {
            let json = serde_json::to_string_pretty(&items)
                .map_err(|e| Error::SearchError(e.to_string()))?;
            println!("{json}");
            return Ok(());
        }
        Some(OutputFormat::Yaml) => {
            let yaml =
                serde_yaml::to_string(&items).map_err(|e| Error::SearchError(e.to_string()))?;
            print!("{yaml}");
            return Ok(());
        }
        None => {}
    }

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
    params: AskParams,
) -> Result<(), Error> {
    let format: Option<OutputFormat> = params
        .out
        .as_deref()
        .map(str::parse)
        .transpose()
        .map_err(Error::VectorSearchError)?;

    let query = params.query;
    let limit_display = query
        .limit
        .map_or_else(|| "default".to_string(), |l| l.to_string());
    let threshold_display = query
        .threshold
        .map_or_else(|| "none".to_string(), |t| format!("{:.2}", t));
    let category_display = query.category.clone().unwrap_or_else(|| "any".to_string());
    let namespace_display = query.namespace.clone().unwrap_or_else(|| "any".to_string());
    let start = std::time::Instant::now();
    let results = svc.ask(&query)?;
    let elapsed = start.elapsed();

    match format {
        Some(OutputFormat::Json) => {
            let json = serde_json::to_string_pretty(&results)
                .map_err(|e| Error::VectorSearchError(e.to_string()))?;
            println!("{json}");
            return Ok(());
        }
        Some(OutputFormat::Yaml) => {
            let yaml = serde_yaml::to_string(&results)
                .map_err(|e| Error::VectorSearchError(e.to_string()))?;
            print!("{yaml}");
            return Ok(());
        }
        None => {}
    }

    println!(
        "Query: \"{}\"  Limit: {}  Threshold: {}  Category: {}  Namespace: {}",
        query.text, limit_display, threshold_display, category_display, namespace_display
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
    params: CategoriesParams,
) -> Result<(), Error> {
    let json_output = match params.out.as_deref() {
        Some("json") => true,
        Some(other) => {
            return Err(Error::ListError(format!(
                "invalid output format: {other} (expected json)"
            )));
        }
        None => false,
    };

    match svc.categories(params.namespace.as_deref()) {
        Ok(categories) => {
            if json_output {
                let json = serde_json::to_string_pretty(&categories)
                    .map_err(|e| Error::ListError(e.to_string()))?;
                println!("{json}");
            } else {
                for category in &categories {
                    println!("{}", category);
                }
            }
            Ok(())
        }
        Err(e) => {
            if json_output {
                let err_resp = CategoriesErrorResponse {
                    error: e.to_string(),
                };
                if let Ok(json) = serde_json::to_string_pretty(&err_resp) {
                    println!("{json}");
                }
            }
            Err(e)
        }
    }
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

pub fn handle_import<S, V, E, M, F, G>(
    services: ImportServices<S, V, E, M, F, G>,
    params: ImportParams,
) -> Result<(), Error>
where
    S: KbStore,
    V: VectorStore,
    E: EmbeddingProvider,
    M: MediaStore,
    F: MediaFetcher,
    G: KbGraph,
{
    let start = std::time::Instant::now();

    let content =
        std::fs::read_to_string(&params.file).map_err(|e| Error::ImportFileError(e.to_string()))?;

    let document: ImportDocument =
        serde_yaml::from_str(&content).map_err(|e| Error::ParseImportFileError(e.to_string()))?;

    let kb_result = services.svc.import_kbs(document.kbs);
    let saved_count = kb_result.saved.len();
    let failed_count = kb_result.failed.len();

    if !kb_result.failed.is_empty() {
        let failed_items: Vec<ImportKbItem> =
            kb_result.failed.into_iter().map(|f| f.item).collect();
        match serialize_failed_items(&failed_items) {
            Ok(content) => {
                if let Err(e) = write_failed_items(&params.failed_items_file, &content) {
                    eprintln!("Warning: could not write failed items file: {}", e);
                }
            }
            Err(e) => eprintln!("Warning: could not serialize failed items: {}", e),
        }
    }

    // Edges are imported strictly after kbs complete — the graph section may
    // reference kbs from this same document, so the store must already
    // reflect which entries actually landed before resolution is attempted.
    let edge_result = services.graph_svc.import_edges(document.graph);
    let edges_saved_count = edge_result.saved.len();
    let edges_failed_count = edge_result.failed.len();

    if !edge_result.failed.is_empty() {
        let failed_edges: Vec<ImportEdgeItem> =
            edge_result.failed.into_iter().map(|f| f.item).collect();
        match serialize_failed_edges(&failed_edges) {
            Ok(content) => {
                if let Err(e) = write_failed_items(&params.failed_edges_file, &content) {
                    eprintln!("Warning: could not write failed edges file: {}", e);
                }
            }
            Err(e) => eprintln!("Warning: could not serialize failed edges: {}", e),
        }
    }

    let elapsed = start.elapsed();
    println!("Import complete.");
    println!("  Imported : {}", saved_count);
    println!("  Failed   : {}", failed_count);
    println!(
        "  Relationships: {} imported, {} failed",
        edges_saved_count, edges_failed_count
    );
    println!("  Duration : {:.2?}", elapsed);
    if failed_count > 0 {
        println!("  Failed items: {}", params.failed_items_file);
    }
    if edges_failed_count > 0 {
        println!("  Failed edges: {}", params.failed_edges_file);
    }

    Ok(())
}

pub fn handle_export<S, V, E, M, F, G>(
    services: ExportServices<S, V, E, M, F, G>,
    params: ExportParams,
) -> Result<(), Error>
where
    S: KbStore,
    V: VectorStore,
    E: EmbeddingProvider,
    M: MediaStore,
    F: MediaFetcher,
    G: KbGraph,
{
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

    let items = services.svc.export_kbs(filter.clone())?;
    if items.is_empty() {
        println!("No entries to export.");
        return Ok(());
    }

    let edges = services.graph_svc.export_edges(&filter)?;
    let document = ExportDocument {
        kbs: items,
        graph: edges,
    };

    let content =
        serde_yaml::to_string(&document).map_err(|e| Error::ExportError(e.to_string()))?;
    std::fs::write(&output_path, &content).map_err(|e| Error::ExportError(e.to_string()))?;

    let entry_count = document.kbs.len();
    let edge_count = document.graph.len();
    let media_count = services.svc.export_media(ExportMediaParams {
        target_dir: params.folder_output,
        category: params.category,
        namespace: params.namespace,
        items: document.kbs,
    })?;

    println!(
        "Exported {} entries and {} relationship(s) to {}",
        entry_count, edge_count, output_path
    );
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

fn serialize_failed_edges(items: &[ImportEdgeItem]) -> Result<String, Error> {
    serialize_yaml_docs(items, Error::WriteFailedItemsError)
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
// Graph handlers
// ---------------------------------------------------------------------------

pub fn handle_link<S: KbStore, G: KbGraph>(
    graph_svc: &GraphService<S, G>,
    params: LinkParams,
) -> Result<(), Error> {
    let json_output = match params.out.as_deref() {
        Some("json") => true,
        Some(other) => {
            return Err(Error::AddEdgeError(format!(
                "invalid output format: {other} (expected json)"
            )));
        }
        None => false,
    };

    match graph_svc.link(params) {
        Ok(edge) => {
            if json_output {
                let confirmation = LinkConfirmation::from(&edge);
                let json = serde_json::to_string_pretty(&confirmation)
                    .map_err(|e| Error::AddEdgeError(e.to_string()))?;
                println!("{json}");
            } else {
                println!("Linked: {} -> {} ({})", edge.from_id, edge.to_id, edge.id);
            }
            Ok(())
        }
        Err(e) => {
            if json_output {
                let err_resp = LinkErrorResponse {
                    error: e.to_string(),
                };
                if let Ok(json) = serde_json::to_string_pretty(&err_resp) {
                    println!("{json}");
                }
            }
            Err(e)
        }
    }
}

pub fn handle_unlink<S: KbStore, G: KbGraph>(
    graph_svc: &GraphService<S, G>,
    params: UnlinkParams,
) -> Result<(), Error> {
    graph_svc.unlink(&params.from, &params.to)?;
    println!("Unlinked: {} -> {}", params.from, params.to);
    Ok(())
}

pub fn handle_related<S: KbStore, G: KbGraph>(
    graph_svc: &GraphService<S, G>,
    params: RelatedParams,
) -> Result<(), Error> {
    let direction: EdgeDirection = params.direction.parse().map_err(Error::GraphQueryError)?;
    let result = graph_svc.related(&params.key_or_id, direction)?;
    if params.json {
        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| Error::GraphQueryError(e.to_string()))?;
        println!("{json}");
    } else {
        print!("{}", render_related(&result, direction));
    }
    Ok(())
}

pub fn handle_tree<S: KbStore, G: KbGraph>(
    graph_svc: &GraphService<S, G>,
    params: TreeParams,
) -> Result<(), Error> {
    let direction: EdgeDirection = params.direction.parse().map_err(Error::GraphQueryError)?;
    let result = graph_svc.tree(TreeWalkParams {
        key_or_id: params.key_or_id,
        direction,
        depth: params.depth,
    })?;
    if params.json {
        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| Error::GraphQueryError(e.to_string()))?;
        println!("{json}");
    } else {
        print!("{}", render_tree(&result));
    }
    Ok(())
}

pub fn handle_graph<S: KbStore, G: KbGraph>(
    graph_svc: &GraphService<S, G>,
    params: GraphViewCliParams,
) -> Result<(), Error> {
    let direction: EdgeDirection = params.direction.parse().map_err(Error::GraphQueryError)?;
    let export = graph_svc.export_graph(GraphViewParams {
        key_or_id: params.key_or_id,
        direction,
        depth: params.depth,
    })?;
    let html = graph_view::render_graph_html(&export)?;
    browser::serve_once_and_open(&html)
}

// ---------------------------------------------------------------------------
// Graph render helpers (pure — unit-tested without capturing stdout)
// ---------------------------------------------------------------------------

/// Max NOTE length in human-readable output before truncation with an
/// ellipsis. `--json` output always carries the full text.
const NOTE_TRUNCATE_LEN: usize = 40;
const TREE_COL_KEY: usize = 12;

/// Truncates to `NOTE_TRUNCATE_LEN` *characters* (not bytes), char-safe via
/// `.chars()` — same technique as `Kb::embedding_text`'s `.chars().take(200)`.
fn truncate_note(note: &str) -> String {
    if note.chars().count() <= NOTE_TRUNCATE_LEN {
        return note.to_string();
    }
    let head: String = note.chars().take(NOTE_TRUNCATE_LEN - 1).collect();
    format!("{head}…")
}

fn render_related(result: &RelatedResult, direction: EdgeDirection) -> String {
    let mut out = String::new();
    if direction != EdgeDirection::In {
        out.push_str(&format!(
            "→ {} points to ({})\n",
            result.node.key,
            result.outgoing.len()
        ));
        for e in &result.outgoing {
            out.push_str(&format!(
                "  {:<key$}  {:<cat$}  \"{}\"\n",
                e.to.key,
                e.to.category,
                truncate_note(&e.note),
                key = COL_KEY,
                cat = COL_CAT,
            ));
        }
        out.push('\n');
    }
    if direction != EdgeDirection::Out {
        out.push_str(&format!(
            "← pointed to by {} ({})\n",
            result.node.key,
            result.incoming.len()
        ));
        for e in &result.incoming {
            out.push_str(&format!(
                "  {:<key$}  {:<cat$}  \"{}\"\n",
                e.from.key,
                e.from.category,
                truncate_note(&e.note),
                key = COL_KEY,
                cat = COL_CAT,
            ));
        }
    }
    out
}

fn render_tree(result: &TreeResult) -> String {
    TreeRenderer::new(&result.nodes).render(&result.root.key, &result.root.id)
}

/// Groups the flat `TreeNode` list by `parent_id` once, then walks it
/// depth-first drawing `├── `/`└── `/`│   ` branches. A struct (not a
/// free function with 4 params) keeps every method within the 2-param rule.
struct TreeRenderer<'a> {
    children: HashMap<&'a str, Vec<&'a TreeNode>>,
    out: String,
}

impl<'a> TreeRenderer<'a> {
    fn new(nodes: &'a [TreeNode]) -> Self {
        let mut children: HashMap<&str, Vec<&TreeNode>> = HashMap::new();
        for node in nodes {
            children
                .entry(node.parent_id.as_str())
                .or_default()
                .push(node);
        }
        Self {
            children,
            out: String::new(),
        }
    }

    fn render(mut self, root_key: &str, root_id: &str) -> String {
        self.out.push_str(root_key);
        self.out.push('\n');
        self.render_children(root_id, "");
        self.out
    }

    fn render_children(&mut self, parent_id: &str, prefix: &str) {
        let kids: Vec<&TreeNode> = match self.children.get(parent_id) {
            Some(k) => k.clone(),
            None => return,
        };
        let last_idx = kids.len().saturating_sub(1);
        for (i, node) in kids.iter().enumerate() {
            let is_last = i == last_idx;
            let branch = if is_last { "└── " } else { "├── " };
            self.out.push_str(&format!(
                "{prefix}{branch}{:<width$} \"{}\"\n",
                node.key,
                truncate_note(&node.note),
                width = TREE_COL_KEY,
            ));
            let child_prefix = format!("{prefix}{}", if is_last { "    " } else { "│   " });
            self.render_children(&node.id, &child_prefix);
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "handlers_tests.rs"]
mod tests;
