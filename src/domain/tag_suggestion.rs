use std::collections::HashMap;

/// Input fields from which tags are extracted.
pub struct TagSuggestionInput {
    /// Text fields to extract keywords from (key, value, notes, reference, category, namespace).
    pub text_fields: Vec<String>,
    /// Tags already assigned to the entry — suggestions will exclude these.
    pub existing_tags: Vec<String>,
}

/// Suggests tags based on keyword frequency across all provided text fields.
///
/// Returns up to `max` suggestions, excluding existing tags, stop words, and
/// words shorter than 3 characters. Results are ranked by frequency (descending).
pub fn suggest_tags(input: &TagSuggestionInput, max: usize) -> Vec<String> {
    if max == 0 || input.text_fields.is_empty() {
        return Vec::new();
    }

    let stop_words = build_stop_words();
    let existing: std::collections::HashSet<String> = input
        .existing_tags
        .iter()
        .map(|t| t.to_lowercase())
        .collect();

    let mut freq: HashMap<String, usize> = HashMap::new();

    for field in &input.text_fields {
        for word in tokenize(field) {
            if word.len() < 3 {
                continue;
            }
            if stop_words.contains(&word) {
                continue;
            }
            if existing.contains(&word) {
                continue;
            }
            *freq.entry(word).or_insert(0) += 1;
        }
    }

    let mut pairs: Vec<(String, usize)> = freq.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    pairs.into_iter().take(max).map(|(w, _)| w).collect()
}

/// Splits text into lowercase alphabetic tokens.
fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_alphabetic() {
            current.push(ch.to_lowercase().next().unwrap_or(ch));
        } else if !current.is_empty() {
            tokens.push(current.clone());
            current.clear();
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Returns a set of common English stop words.
fn build_stop_words() -> std::collections::HashSet<String> {
    stop_words::get(stop_words::LANGUAGE::English)
        .iter()
        .map(|s| s.to_string())
        .collect()
}

#[cfg(test)]
#[path = "tag_suggestion_tests.rs"]
mod tests;
