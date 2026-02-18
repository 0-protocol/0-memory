use zero_memory::compiler::hasher;
use zero_memory::types::ContextMeta;

#[test]
fn same_label_produces_same_concept_hash() {
    let h1 = hasher::concept_hash("agent");
    let h2 = hasher::concept_hash("agent");
    assert_eq!(h1, h2, "Same label must produce identical ConceptHash");
}

#[test]
fn different_labels_produce_different_concept_hash() {
    let h1 = hasher::concept_hash("agent");
    let h2 = hasher::concept_hash("memory");
    assert_ne!(
        h1, h2,
        "Different labels must produce different ConceptHash"
    );
}

#[test]
fn same_spo_produces_same_fact_hash_regardless_of_context() {
    let f1 = hasher::fact_hash("agent", "needs", "memory");
    let f2 = hasher::fact_hash("agent", "needs", "memory");
    assert_eq!(f1, f2, "Same (s,p,o) must produce identical FactHash");
}

#[test]
fn same_fact_different_context_produces_different_episode_hash() {
    let fh = hasher::fact_hash("agent", "needs", "memory");

    let ctx1 = ContextMeta {
        event_time: "2026-02-18T00:00:00Z".to_string(),
        source: "user_prompt".to_string(),
        scope: "session_1".to_string(),
        agent_id: None,
        session_id: None,
        metadata: None,
    };
    let ctx2 = ContextMeta {
        event_time: "2026-02-19T00:00:00Z".to_string(),
        source: "observation".to_string(),
        scope: "session_2".to_string(),
        agent_id: None,
        session_id: None,
        metadata: None,
    };

    let ch1 = hasher::context_hash(&ctx1);
    let ch2 = hasher::context_hash(&ctx2);
    assert_ne!(ch1, ch2);

    let eh1 = hasher::episode_hash(&fh, &ch1);
    let eh2 = hasher::episode_hash(&fh, &ch2);
    assert_ne!(
        eh1, eh2,
        "Same fact + different context must produce different EpisodeHash"
    );
}

#[test]
fn concept_hash_is_stable_known_value() {
    use sha2::{Digest, Sha256};
    let expected = Sha256::digest(b"agent");
    let h = hasher::concept_hash("agent");
    assert_eq!(
        h.0,
        expected.as_slice(),
        "ConceptHash must be raw sha256 of label bytes"
    );
}

#[test]
fn fact_hash_uses_pipe_separator() {
    use sha2::{Digest, Sha256};
    let expected = Sha256::digest(b"agent|needs|memory");
    let h = hasher::fact_hash("agent", "needs", "memory");
    assert_eq!(
        h.0,
        expected.as_slice(),
        "FactHash must be sha256 of 's|p|o'"
    );
}

#[test]
fn episode_hash_concatenates_raw_bytes() {
    use sha2::{Digest, Sha256};
    let fh = hasher::fact_hash("agent", "needs", "memory");
    let ctx = ContextMeta {
        event_time: "2026-02-18T00:00:00Z".to_string(),
        source: "test".to_string(),
        scope: "test_scope".to_string(),
        agent_id: None,
        session_id: None,
        metadata: None,
    };
    let ch = hasher::context_hash(&ctx);
    let eh = hasher::episode_hash(&fh, &ch);

    let mut combined = Vec::new();
    combined.extend_from_slice(&fh.0);
    combined.extend_from_slice(&ch.0);
    let expected = Sha256::digest(&combined);
    assert_eq!(
        eh.0,
        expected.as_slice(),
        "EpisodeHash must be sha256(fact_bytes + ctx_bytes)"
    );
}

#[test]
fn hash_display_format_is_lowercase_hex() {
    let h = hasher::concept_hash("agent");
    let display = format!("{}", h);
    assert_eq!(display.len(), 64, "Hex display must be 64 chars");
    assert!(
        display.chars().all(|c| c.is_ascii_hexdigit()),
        "Must be valid hex"
    );
}
