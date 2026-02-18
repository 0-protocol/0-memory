use crate::compiler::normalizer::normalize_label;
use crate::types::ConceptHash;
use std::collections::HashMap;

/// Reverse index from normalized label strings to ConceptHash.
///
/// Both `insert` and `lookup` normalize the label before accessing
/// the map, so callers do not need to pre-normalize.
#[derive(Debug, Clone, Default)]
pub struct LabelIndex {
    label_to_hash: HashMap<String, ConceptHash>,
}

impl LabelIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, label: &str, hash: ConceptHash) {
        self.label_to_hash.insert(normalize_label(label), hash);
    }

    pub fn lookup(&self, label: &str) -> Option<&ConceptHash> {
        self.label_to_hash.get(&normalize_label(label))
    }

    pub fn len(&self) -> usize {
        self.label_to_hash.len()
    }

    pub fn is_empty(&self) -> bool {
        self.label_to_hash.is_empty()
    }
}
