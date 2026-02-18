use super::index::LabelIndex;
use crate::types::*;
use std::collections::{HashMap, HashSet};

/// In-memory graph store for concepts, relations, and contexts.
#[derive(Debug, Clone)]
pub struct MemoryStore {
    concepts: HashMap<ConceptHash, ConceptNode>,
    relations_by_fact: HashMap<FactHash, Vec<RelationNode>>,
    relations_by_episode: HashMap<EpisodeHash, RelationNode>,
    contexts: HashMap<ContextHash, ContextNode>,
    adjacency: HashMap<ConceptHash, HashSet<FactHash>>,
    label_index: LabelIndex,
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            concepts: HashMap::new(),
            relations_by_fact: HashMap::new(),
            relations_by_episode: HashMap::new(),
            contexts: HashMap::new(),
            adjacency: HashMap::new(),
            label_index: LabelIndex::new(),
        }
    }

    /// Insert a full memory record. Deduplicates concepts by hash
    /// and relations by episode hash. Same fact from different contexts
    /// produces multiple episodes under the same FactHash.
    ///
    /// When a concept is re-inserted with the same hash, the store merges
    /// the new data: `updated_at` is refreshed, confidence takes the max
    /// of old and new, and any new aliases are appended.
    pub fn insert_record(&mut self, record: MemoryRecord) -> InsertResult {
        let mut result = InsertResult::default();

        for concept in record.concepts {
            if let Some(existing) = self.concepts.get_mut(&concept.hash) {
                existing.updated_at = concept.updated_at;
                if concept.confidence > existing.confidence {
                    existing.confidence = concept.confidence;
                }
                for alias in concept.aliases {
                    if !existing.aliases.contains(&alias) {
                        existing.aliases.push(alias);
                    }
                }
                result.dupes_skipped += 1;
            } else {
                self.label_index
                    .insert(&concept.label, concept.hash.clone());
                self.concepts.insert(concept.hash.clone(), concept);
                result.new_concepts += 1;
            }
        }

        for relation in record.relations {
            if self
                .relations_by_episode
                .contains_key(&relation.episode_hash)
            {
                result.dupes_skipped += 1;
                continue;
            }

            let is_new_fact = !self.relations_by_fact.contains_key(&relation.fact_hash);
            if is_new_fact {
                result.new_facts += 1;
            }

            self.adjacency
                .entry(relation.subject_hash.clone())
                .or_default()
                .insert(relation.fact_hash.clone());
            self.adjacency
                .entry(relation.object_hash.clone())
                .or_default()
                .insert(relation.fact_hash.clone());

            self.relations_by_fact
                .entry(relation.fact_hash.clone())
                .or_default()
                .push(relation.clone());
            self.relations_by_episode
                .insert(relation.episode_hash.clone(), relation);

            result.new_episodes += 1;
        }

        if !self.contexts.contains_key(&record.context.hash) {
            self.contexts
                .insert(record.context.hash.clone(), record.context);
        }

        result
    }

    pub fn get_concept(&self, hash: &ConceptHash) -> Option<&ConceptNode> {
        self.concepts.get(hash)
    }

    pub fn get_concept_by_label(&self, label: &str) -> Option<&ConceptNode> {
        let hash = self.label_index.lookup(label)?;
        self.concepts.get(hash)
    }

    /// Return all relation episodes that reference the given concept
    /// (as subject or object). Each episode is returned at most once,
    /// deduplicated by `EpisodeHash`.
    pub fn get_relations(&self, concept_hash: &ConceptHash) -> Vec<&RelationNode> {
        let Some(fact_hashes) = self.adjacency.get(concept_hash) else {
            return Vec::new();
        };
        let mut seen = HashSet::new();
        let mut results = Vec::new();
        for fh in fact_hashes {
            if let Some(episodes) = self.relations_by_fact.get(fh) {
                for rel in episodes {
                    if seen.insert(&rel.episode_hash) {
                        results.push(rel);
                    }
                }
            }
        }
        results
    }

    pub fn get_relations_by_fact(&self, fact_hash: &FactHash) -> Vec<&RelationNode> {
        self.relations_by_fact
            .get(fact_hash)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    pub fn get_context(&self, hash: &ContextHash) -> Option<&ContextNode> {
        self.contexts.get(hash)
    }

    pub fn concept_count(&self) -> usize {
        self.concepts.len()
    }

    pub fn relation_count(&self) -> usize {
        self.relations_by_episode.len()
    }

    pub fn label_index(&self) -> &LabelIndex {
        &self.label_index
    }
}
