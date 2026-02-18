use std::collections::HashMap;

/// Normalize a concept label to canonical form.
///
/// Rules:
/// - lowercase
/// - trim whitespace
/// - collapse multiple spaces to single space
/// - replace common separators (`_`, `.`) with `-`
pub fn normalize_label(raw: &str) -> String {
    let s = raw.trim().to_lowercase();
    let s: String = s
        .chars()
        .map(|c| match c {
            '_' | '.' => '-',
            _ => c,
        })
        .collect();
    let mut result = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        if c == ' ' {
            if !prev_space {
                result.push(' ');
            }
            prev_space = true;
        } else {
            prev_space = false;
            result.push(c);
        }
    }
    result
}

/// Bidirectional alias table mapping variant labels to a canonical form.
pub struct AliasTable {
    map: HashMap<String, String>,
}

impl Default for AliasTable {
    fn default() -> Self {
        Self::new()
    }
}

impl AliasTable {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Create an alias table pre-populated with common 0-memory domain aliases.
    pub fn with_defaults() -> Self {
        let mut table = Self::new();
        table.insert("long_term_memory", "long-term-memory");
        table.insert("LTM", "long-term-memory");
        table.insert("short_term_memory", "short-term-memory");
        table.insert("STM", "short-term-memory");
        table.insert("working_memory", "working-memory");
        table.insert("WM", "working-memory");
        table.insert("semantic_memory", "semantic-memory");
        table.insert("episodic_memory", "episodic-memory");
        table
    }

    pub fn insert(&mut self, alias: &str, canonical: &str) {
        self.map
            .insert(normalize_label(alias), normalize_label(canonical));
    }

    /// Resolve a label through the alias table. Returns the canonical label
    /// if an alias exists, otherwise returns the normalized input.
    pub fn resolve(&self, label: &str) -> String {
        let normalized = normalize_label(label);
        self.map.get(&normalized).cloned().unwrap_or(normalized)
    }
}

/// Normalize a predicate string.
///
/// Rules:
/// - lowercase
/// - trim whitespace
/// - collapse multiple spaces
/// - convert spaces, hyphens, and dots to underscores (predicate convention: `is_a`, `has_part`)
pub fn normalize_predicate(raw: &str) -> String {
    let s = raw.trim().to_lowercase();
    let mut result = String::with_capacity(s.len());
    let mut prev_sep = false;
    for c in s.chars() {
        match c {
            ' ' | '-' | '.' => {
                if !prev_sep {
                    result.push('_');
                }
                prev_sep = true;
            }
            _ => {
                prev_sep = false;
                result.push(c);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_label_basic() {
        assert_eq!(normalize_label("  Agent  "), "agent");
        assert_eq!(normalize_label("Long_Term_Memory"), "long-term-memory");
        assert_eq!(normalize_label("some.dotted.name"), "some-dotted-name");
        assert_eq!(normalize_label("  multiple   spaces  "), "multiple spaces");
    }

    #[test]
    fn alias_table_resolves() {
        let table = AliasTable::with_defaults();
        assert_eq!(table.resolve("LTM"), "long-term-memory");
        assert_eq!(table.resolve("long_term_memory"), "long-term-memory");
        assert_eq!(table.resolve("unknown-concept"), "unknown-concept");
    }

    #[test]
    fn normalize_predicate_basic() {
        assert_eq!(normalize_predicate("is a"), "is_a");
        assert_eq!(normalize_predicate("  Has  Part  "), "has_part");
        assert_eq!(normalize_predicate("NEEDS"), "needs");
    }

    #[test]
    fn normalize_predicate_handles_separators() {
        assert_eq!(normalize_predicate("has-part"), "has_part");
        assert_eq!(normalize_predicate("is.a"), "is_a");
        assert_eq!(normalize_predicate("multi - dash"), "multi_dash");
    }
}
