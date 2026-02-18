use sha2::{Digest, Sha256};

use crate::types::{ConceptHash, ContextHash, ContextMeta, EpisodeHash, FactHash};

/// sha256(normalized_label)
pub fn concept_hash(normalized_label: &str) -> ConceptHash {
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&Sha256::digest(normalized_label.as_bytes()));
    ConceptHash(hash)
}

/// sha256(subject_label + "|" + predicate + "|" + object_label)
///
/// The pipe separator prevents ambiguity when labels contain parts of other
/// labels (e.g., "a|b" vs "a" "|" "b").
pub fn fact_hash(subject_label: &str, predicate: &str, object_label: &str) -> FactHash {
    let input = format!("{}|{}|{}", subject_label, predicate, object_label);
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&Sha256::digest(input.as_bytes()));
    FactHash(hash)
}

/// sha256(event_time + "|" + source + "|" + scope)
pub fn context_hash(meta: &ContextMeta) -> ContextHash {
    let input = format!("{}|{}|{}", meta.event_time, meta.source, meta.scope);
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&Sha256::digest(input.as_bytes()));
    ContextHash(hash)
}

/// sha256(fact_hash_bytes ++ context_hash_bytes)
///
/// Concatenates the raw 32-byte arrays (64 bytes total) before hashing.
pub fn episode_hash(fact: &FactHash, ctx: &ContextHash) -> EpisodeHash {
    let mut combined = [0u8; 64];
    combined[..32].copy_from_slice(&fact.0);
    combined[32..].copy_from_slice(&ctx.0);
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&Sha256::digest(combined));
    EpisodeHash(hash)
}

/// Return the first `n` hex characters of a 32-byte hash.
/// Used for short display references (e.g., first 8 chars in .0c format).
pub fn short_hex(hash: &[u8; 32], n: usize) -> String {
    let n = n.min(64);
    let bytes_needed = (n + 1) / 2;
    let mut s = String::with_capacity(n);
    for &b in &hash[..bytes_needed] {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s.truncate(n);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_concept_hash() {
        let a = concept_hash("agent");
        let b = concept_hash("agent");
        assert_eq!(a, b, "same label must produce identical hash");
    }

    #[test]
    fn different_labels_differ() {
        let a = concept_hash("agent");
        let b = concept_hash("memory");
        assert_ne!(a, b);
    }

    #[test]
    fn fact_hash_context_free() {
        let f1 = fact_hash("agent", "needs", "long-term-memory");
        let f2 = fact_hash("agent", "needs", "long-term-memory");
        assert_eq!(f1, f2, "same (s,p,o) must produce identical FactHash");
    }

    #[test]
    fn episode_hash_varies_with_context() {
        let fh = fact_hash("agent", "needs", "long-term-memory");
        let ctx1 = ContextMeta {
            event_time: "2026-02-18T00:00:00Z".into(),
            source: "user_prompt".into(),
            scope: "conversation_1".into(),
            agent_id: None,
            session_id: None,
            metadata: None,
        };
        let ctx2 = ContextMeta {
            event_time: "2026-02-18T01:00:00Z".into(),
            source: "user_prompt".into(),
            scope: "conversation_2".into(),
            agent_id: None,
            session_id: None,
            metadata: None,
        };
        let ch1 = context_hash(&ctx1);
        let ch2 = context_hash(&ctx2);
        let e1 = episode_hash(&fh, &ch1);
        let e2 = episode_hash(&fh, &ch2);
        assert_ne!(
            e1, e2,
            "same fact + different context = different EpisodeHash"
        );
    }

    #[test]
    fn hash_stability() {
        let h = concept_hash("agent");
        let hex_str = format!("{}", h);
        assert_eq!(hex_str.len(), 64, "sha256 hex must be 64 chars");
        // Hardcoded known value for regression detection
        assert_eq!(
            hex_str,
            "d4f0bc5a29de06b510f9aa428f1eedba926012b591fef7a518e776a7c9bd1824"
        );
    }
}
