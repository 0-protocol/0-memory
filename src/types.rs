use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

// ---------------------------------------------------------------------------
// Hex serde helper — serializes [u8; 32] as a hex string for readability
// ---------------------------------------------------------------------------

mod hex_serde {
    use super::*;

    pub fn serialize<S: Serializer>(bytes: &[u8; 32], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 32], D::Error> {
        let hex_str = String::deserialize(d)?;
        let bytes = hex::decode(&hex_str).map_err(serde::de::Error::custom)?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| serde::de::Error::custom("expected 32 bytes"))?;
        Ok(arr)
    }
}

// ---------------------------------------------------------------------------
// Hash newtypes
// ---------------------------------------------------------------------------

/// Content-addressed concept identity.
/// `ConceptHash = sha256(normalized_label)`
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ConceptHash(pub [u8; 32]);

/// Context-free semantic identity of a fact.
/// `FactHash = sha256(normalized_subject | predicate | normalized_object)`
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct FactHash(pub [u8; 32]);

/// Context-bound event identity.
/// `EpisodeHash = sha256(FactHash + context_hash)`
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct EpisodeHash(pub [u8; 32]);

/// Hash of a context block.
/// `ContextHash = sha256(event_time | source | scope)`
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ContextHash(pub [u8; 32]);

macro_rules! impl_hash_type {
    ($t:ty) => {
        impl fmt::Debug for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($t), hex::encode(self.0))
            }
        }

        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", hex::encode(self.0))
            }
        }

        impl AsRef<[u8]> for $t {
            fn as_ref(&self) -> &[u8] {
                &self.0
            }
        }

        impl Serialize for $t {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                hex_serde::serialize(&self.0, s)
            }
        }

        impl<'de> Deserialize<'de> for $t {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                hex_serde::deserialize(d).map(Self)
            }
        }
    };
}

impl_hash_type!(ConceptHash);
impl_hash_type!(FactHash);
impl_hash_type!(EpisodeHash);
impl_hash_type!(ContextHash);

// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

/// Input tuple from upstream (LLM or structured source).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticTuple {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
}

/// Metadata about the observation context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMeta {
    /// ISO 8601 timestamp of the observed event.
    pub event_time: String,
    /// Origin of the observation, e.g. `"user_prompt"`, `"observation"`.
    pub source: String,
    /// Scope identifier, e.g. `"conversation_123"`.
    pub scope: String,
    /// Optional agent identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// Optional session identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Optional free-form metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

/// Full input to the compiler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilerInput {
    /// The original utterance (if available).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub utterance: Option<String>,
    /// Extracted semantic tuples.
    pub tuples: Vec<SemanticTuple>,
    /// Observation context.
    pub context: ContextMeta,
}

// ---------------------------------------------------------------------------
// Stored node types
// ---------------------------------------------------------------------------

/// A stored concept node.
///
/// When the same concept (same `hash`) is inserted multiple times, the store
/// should merge rather than discard: take the max `confidence`, union the
/// `aliases`, and refresh `updated_at`. The initial insert sets `confidence`
/// to the first-seen value from the originating `SemanticTuple`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptNode {
    pub hash: ConceptHash,
    pub label: String,
    pub aliases: Vec<String>,
    pub confidence: f64,
    pub created_at: String,
    pub updated_at: String,
}

/// A stored relation node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationNode {
    pub fact_hash: FactHash,
    pub episode_hash: EpisodeHash,
    pub subject_hash: ConceptHash,
    pub predicate: String,
    pub object_hash: ConceptHash,
    pub confidence: f64,
    pub context_hash: ContextHash,
    pub created_at: String,
}

/// A stored context node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextNode {
    pub hash: ContextHash,
    pub meta: ContextMeta,
}

// ---------------------------------------------------------------------------
// Memory record
// ---------------------------------------------------------------------------

/// A complete memory record produced by the compiler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub concepts: Vec<ConceptNode>,
    pub relations: Vec<RelationNode>,
    pub context: ContextNode,
}

// ---------------------------------------------------------------------------
// Compiler output
// ---------------------------------------------------------------------------

/// Output of the compiler pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilerOutput {
    /// The `.0` format graph text.
    pub graph_text: String,
    /// The structured in-memory record.
    pub record: MemoryRecord,
}

// ---------------------------------------------------------------------------
// Store result types
// ---------------------------------------------------------------------------

/// Result of inserting a `MemoryRecord` into the store.
#[derive(Debug, Clone, Default)]
pub struct InsertResult {
    pub new_concepts: usize,
    pub new_facts: usize,
    pub new_episodes: usize,
    pub dupes_skipped: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concept_hash_hex_display() {
        let hash = ConceptHash([0xab; 32]);
        let hex = format!("{}", hash);
        assert_eq!(hex.len(), 64);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_newtypes_are_distinct() {
        let bytes = [0u8; 32];
        let _c = ConceptHash(bytes);
        let _f = FactHash(bytes);
        let _e = EpisodeHash(bytes);
        let _x = ContextHash(bytes);
    }

    #[test]
    fn semantic_tuple_serialization_roundtrip() {
        let tuple = SemanticTuple {
            subject: "Agent".to_string(),
            predicate: "needs".to_string(),
            object: "LongTermMemory".to_string(),
            confidence: 0.98,
        };
        let json = serde_json::to_string(&tuple).unwrap();
        let parsed: SemanticTuple = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.subject, "Agent");
        assert_eq!(parsed.confidence, 0.98);
    }

    #[test]
    fn context_meta_optional_fields() {
        let meta = ContextMeta {
            event_time: "2026-02-18T00:00:00Z".to_string(),
            source: "user_prompt".to_string(),
            scope: "test".to_string(),
            agent_id: None,
            session_id: None,
            metadata: None,
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert!(!json.contains("agent_id"));
        assert!(!json.contains("session_id"));
        assert!(!json.contains("metadata"));
    }

    #[test]
    fn memory_record_serialization() {
        let record = MemoryRecord {
            concepts: vec![ConceptNode {
                hash: ConceptHash([1; 32]),
                label: "test".to_string(),
                aliases: vec![],
                confidence: 1.0,
                created_at: "2026-02-18T00:00:00Z".to_string(),
                updated_at: "2026-02-18T00:00:00Z".to_string(),
            }],
            relations: vec![],
            context: ContextNode {
                hash: ContextHash([2; 32]),
                meta: ContextMeta {
                    event_time: "2026-02-18T00:00:00Z".to_string(),
                    source: "test".to_string(),
                    scope: "unit_test".to_string(),
                    agent_id: None,
                    session_id: None,
                    metadata: None,
                },
            },
        };
        let json = serde_json::to_string(&record).unwrap();
        let parsed: MemoryRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.concepts.len(), 1);
        assert_eq!(parsed.concepts[0].label, "test");
    }

    #[test]
    fn hash_serializes_as_hex_string() {
        let hash = ConceptHash([0xab; 32]);
        let json = serde_json::to_string(&hash).unwrap();
        assert!(
            json.starts_with('"'),
            "Hash must serialize as a JSON string, got: {}",
            json
        );
        assert!(json.ends_with('"'));
        let inner = &json[1..json.len() - 1];
        assert_eq!(inner.len(), 64);
        assert!(inner.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_serde_roundtrip() {
        let original = FactHash([0x42; 32]);
        let json = serde_json::to_string(&original).unwrap();
        let parsed: FactHash = serde_json::from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }
}
