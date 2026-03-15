use crate::domain::{EmbeddingInput, KbFilter, KbUpdate, NewKb, ScoredKbItem, SemanticQuery};
use crate::errors::Error;
use crate::ports::{EmbeddingProvider, KbStore, VectorStore};
use crate::service::{SemanticService, Service};

// ---------------------------------------------------------------------------
// Services container — groups kb + semantic services to satisfy the 2-param rule
// ---------------------------------------------------------------------------

pub struct Services<S: KbStore, V: VectorStore, E: EmbeddingProvider> {
    pub kb: Service<S>,
    pub semantic: SemanticService<V, E>,
}

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
    services: &Services<S, V, E>,
    new_kb: NewKb,
) -> Result<(), Error> {
    let kb = services.kb.add_kb(new_kb)?;
    let input = EmbeddingInput {
        kb_id: kb.id.clone(),
        text: kb.embedding_text(),
    };
    if let Err(e) = services.semantic.index_kb(&input) {
        eprintln!("Warning: could not index embedding for '{}': {}", kb.key, e);
    }
    println!("Created: {}", kb.id);
    Ok(())
}

pub fn handle_get<T: KbStore>(svc: &Service<T>, params: GetParams) -> Result<(), Error> {
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

pub fn handle_list<T: KbStore>(svc: &Service<T>, params: ListParams) -> Result<(), Error> {
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
    let items = svc.list_kbs(filter)?;
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
    services: &Services<S, V, E>,
    update: KbUpdate,
) -> Result<(), Error> {
    let existing = services
        .kb
        .get_kb_by_id(&update.id)?
        .ok_or(Error::KBNotFound)?;

    let old_embed_text = existing.embedding_text();

    let updated = crate::domain::Kb {
        id: existing.id,
        key: update.key.unwrap_or(existing.key),
        value: update.value.unwrap_or(existing.value),
        notes: update.notes.unwrap_or(existing.notes),
        category: update.category.unwrap_or(existing.category),
        namespace: update.namespace.unwrap_or(existing.namespace),
        reference: update.reference.unwrap_or(existing.reference),
        tags: update.tags.unwrap_or(existing.tags),
        created_on: existing.created_on,
    };
    let embed_text = updated.embedding_text();
    let kb_id = updated.id.clone();

    if old_embed_text != embed_text {
        let input = EmbeddingInput {
            kb_id,
            text: embed_text,
        };
        if let Err(e) = services.semantic.index_kb(&input) {
            eprintln!(
                "Warning: could not update embedding for '{}': {}",
                update.id, e
            );
        }
    }

    services.kb.update_kb(updated)?;
    println!("Updated: {}", update.id);
    Ok(())
}

pub fn handle_delete<S: KbStore, V: VectorStore, E: EmbeddingProvider>(
    services: &Services<S, V, E>,
    id: String,
) -> Result<(), Error> {
    services.kb.delete_kb(&id)?;
    if let Err(e) = services.semantic.remove_index(&id) {
        eprintln!("Warning: could not remove embedding for '{}': {}", id, e);
    }
    println!("Deleted: {}", id);
    Ok(())
}

pub fn handle_search<T: KbStore>(svc: &Service<T>, keyword: String) -> Result<(), Error> {
    let items = svc.search_kbs(&keyword)?;
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

pub fn handle_ask<V: VectorStore, E: EmbeddingProvider>(
    sem_svc: &SemanticService<V, E>,
    query: SemanticQuery,
) -> Result<(), Error> {
    let results = sem_svc.ask(&query)?;
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

pub fn handle_reindex<S: KbStore, V: VectorStore, E: EmbeddingProvider>(
    services: &Services<S, V, E>,
) -> Result<(), Error> {
    let items = services.kb.list_kbs(KbFilter::default())?;
    let total = items.len();
    println!("Reindexing {} entries...", total);

    let mut success = 0usize;
    let mut failed = 0usize;

    for item in items {
        match services.kb.get_kb_by_id(&item.id)? {
            Some(kb) => {
                let input = EmbeddingInput {
                    kb_id: kb.id.clone(),
                    text: kb.embedding_text(),
                };
                match services.semantic.index_kb(&input) {
                    Ok(_) => {
                        success += 1;
                        println!("  [OK] {}", kb.key);
                    }
                    Err(e) => {
                        failed += 1;
                        eprintln!("  [FAIL] {}: {}", kb.key, e);
                    }
                }
            }
            None => {
                failed += 1;
                eprintln!("  [FAIL] id={} not found", item.id);
            }
        }
    }

    println!("Done: {} indexed, {} failed.", success, failed);
    Ok(())
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
