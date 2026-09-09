use super::{TagSuggestionInput, suggest_tags};

fn make_input(texts: Vec<&str>, existing: Vec<&str>) -> TagSuggestionInput {
    TagSuggestionInput {
        text_fields: texts.into_iter().map(String::from).collect(),
        existing_tags: existing.into_iter().map(String::from).collect(),
    }
}

#[test]
fn suggest_tags_returns_empty_for_empty_input() {
    let input = make_input(vec![], vec![]);
    let result = suggest_tags(&input, 5);
    assert!(result.is_empty());
}

#[test]
fn suggest_tags_excludes_stop_words() {
    let input = make_input(vec!["the rust programming language"], vec![]);
    let result = suggest_tags(&input, 10);
    assert!(!result.contains(&"the".to_string()));
    assert!(result.contains(&"rust".to_string()));
}

#[test]
fn suggest_tags_excludes_short_words() {
    // "go" and "is" are 2 chars or less — excluded by length rule
    let input = make_input(vec!["go is a language by google"], vec![]);
    let result = suggest_tags(&input, 10);
    assert!(!result.contains(&"go".to_string()));
    assert!(!result.contains(&"is".to_string()));
}

#[test]
fn suggest_tags_excludes_existing_tags() {
    let input = make_input(vec!["rust ownership memory"], vec!["rust"]);
    let result = suggest_tags(&input, 10);
    assert!(!result.contains(&"rust".to_string()));
    assert!(result.contains(&"ownership".to_string()));
}

#[test]
fn suggest_tags_ranks_by_frequency() {
    let input = make_input(vec!["rust rust rust memory ownership memory"], vec![]);
    let result = suggest_tags(&input, 3);
    assert!(!result.is_empty());
    assert_eq!(result[0], "rust");
    assert_eq!(result[1], "memory");
}

#[test]
fn suggest_tags_respects_max_limit() {
    let input = make_input(vec!["alpha bravo charlie delta echo"], vec![]);
    let result = suggest_tags(&input, 2);
    assert!(result.len() <= 2);
}

#[test]
fn suggest_tags_lowercases_words() {
    let input = make_input(vec!["Rust OWNERSHIP"], vec![]);
    let result = suggest_tags(&input, 10);
    assert!(result.contains(&"rust".to_string()));
    assert!(result.contains(&"ownership".to_string()));
}

#[test]
fn suggest_tags_handles_multiple_text_fields() {
    let input = make_input(
        vec!["rust programming", "memory management", "rust language"],
        vec![],
    );
    let result = suggest_tags(&input, 10);
    // "rust" appears in 2 fields — should be first
    assert!(!result.is_empty());
    assert_eq!(result[0], "rust");
}
