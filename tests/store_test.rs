use zero_memory::compiler::hasher;
use zero_memory::store::MemoryStore;
use zero_memory::types::*;

fn make_context(scope: &str) -> (ContextHash, ContextNode) {
    let meta = ContextMeta {
        event_time: "2026-02-18T00:00:00Z".to_string(),
        source: "test".to_string(),
        scope: scope.to_string(),
        agent_id: None,
        session_id: None,
        metadata: None,
    };
    let hash = hasher::context_hash(&meta);
    let node = ContextNode {
        hash: hash.clone(),
        meta,
    };
    (hash, node)
}

fn make_record(
    concepts: Vec<(&str, f64)>,
    relations: Vec<(&str, &str, &str, f64)>,
    scope: &str,
) -> MemoryRecord {
    let (ctx_hash, ctx_node) = make_context(scope);
    let now = "2026-02-18T00:00:00Z".to_string();

    let concept_nodes: Vec<ConceptNode> = concepts
        .iter()
        .map(|(label, conf)| ConceptNode {
            hash: hasher::concept_hash(label),
            label: label.to_string(),
            aliases: vec![],
            confidence: *conf,
            created_at: now.clone(),
            updated_at: now.clone(),
        })
        .collect();

    let relation_nodes: Vec<RelationNode> = relations
        .iter()
        .map(|(s, p, o, conf)| {
            let fh = hasher::fact_hash(s, p, o);
            let eh = hasher::episode_hash(&fh, &ctx_hash);
            RelationNode {
                fact_hash: fh,
                episode_hash: eh,
                subject_hash: hasher::concept_hash(s),
                predicate: p.to_string(),
                object_hash: hasher::concept_hash(o),
                confidence: *conf,
                context_hash: ctx_hash.clone(),
                created_at: now.clone(),
            }
        })
        .collect();

    MemoryRecord {
        concepts: concept_nodes,
        relations: relation_nodes,
        context: ctx_node,
    }
}

#[test]
fn insert_and_retrieve_concept() {
    let mut store = MemoryStore::new();
    let record = make_record(vec![("agent", 0.9)], vec![], "test_scope");
    store.insert_record(record);

    let hash = hasher::concept_hash("agent");
    let concept = store.get_concept(&hash);
    assert!(
        concept.is_some(),
        "Must be able to retrieve inserted concept"
    );
    assert_eq!(concept.unwrap().label, "agent");
}

#[test]
fn insert_same_concept_twice_deduplicates() {
    let mut store = MemoryStore::new();

    let record1 = make_record(vec![("agent", 0.9)], vec![], "scope_1");
    let result1 = store.insert_record(record1);
    assert_eq!(result1.new_concepts, 1);

    let record2 = make_record(vec![("agent", 0.95)], vec![], "scope_2");
    let result2 = store.insert_record(record2);
    assert_eq!(result2.dupes_skipped, 1);
    assert_eq!(result2.new_concepts, 0);

    assert_eq!(
        store.concept_count(),
        1,
        "Duplicate concept must not increase count"
    );

    let concept = store.get_concept_by_label("agent").unwrap();
    assert_eq!(
        concept.confidence, 0.95,
        "Re-inserted concept with higher confidence should update to max"
    );
}

#[test]
fn same_fact_different_context_produces_two_episodes() {
    let mut store = MemoryStore::new();

    let record1 = make_record(
        vec![("agent", 0.9), ("memory", 0.9)],
        vec![("agent", "needs", "memory", 0.98)],
        "session_1",
    );
    store.insert_record(record1);

    let record2 = make_record(
        vec![("agent", 0.9), ("memory", 0.9)],
        vec![("agent", "needs", "memory", 0.95)],
        "session_2",
    );
    store.insert_record(record2);

    let fh = hasher::fact_hash("agent", "needs", "memory");
    let episodes = store.get_relations_by_fact(&fh);
    assert_eq!(
        episodes.len(),
        2,
        "Same fact from two contexts should yield two episodes"
    );
}

#[test]
fn get_relations_returns_correct_neighbors() {
    let mut store = MemoryStore::new();
    let record = make_record(
        vec![("agent", 0.9), ("memory", 0.9), ("0-lang", 0.9)],
        vec![
            ("agent", "needs", "memory", 0.98),
            ("agent", "uses", "0-lang", 0.95),
        ],
        "test_scope",
    );
    store.insert_record(record);

    let agent_hash = hasher::concept_hash("agent");
    let rels = store.get_relations(&agent_hash);
    assert_eq!(rels.len(), 2, "Agent should be connected to 2 relations");

    let memory_hash = hasher::concept_hash("memory");
    let rels = store.get_relations(&memory_hash);
    assert_eq!(rels.len(), 1, "Memory should be connected to 1 relation");
}

#[test]
fn get_concept_by_label_works() {
    let mut store = MemoryStore::new();
    let record = make_record(vec![("agent", 0.9)], vec![], "test_scope");
    store.insert_record(record);

    let concept = store.get_concept_by_label("agent");
    assert!(concept.is_some());
    assert_eq!(concept.unwrap().confidence, 0.9);
}

#[test]
fn get_context_works() {
    let mut store = MemoryStore::new();
    let (ctx_hash, _) = make_context("test_scope");
    let record = make_record(vec![("agent", 0.9)], vec![], "test_scope");
    store.insert_record(record);

    let ctx = store.get_context(&ctx_hash);
    assert!(ctx.is_some());
    assert_eq!(ctx.unwrap().meta.scope, "test_scope");
}

#[test]
fn empty_store_counts_are_zero() {
    let store = MemoryStore::new();
    assert_eq!(store.concept_count(), 0);
    assert_eq!(store.relation_count(), 0);
}

#[test]
fn concept_reinsert_merges_confidence_and_aliases() {
    let mut store = MemoryStore::new();

    let record1 = make_record(vec![("agent", 0.8)], vec![], "scope_1");
    store.insert_record(record1);

    let mut record2 = make_record(vec![("agent", 0.95)], vec![], "scope_2");
    record2.concepts[0].aliases = vec!["bot".to_string()];
    record2.concepts[0].updated_at = "2026-02-19T00:00:00Z".to_string();
    store.insert_record(record2);

    let concept = store.get_concept_by_label("agent").unwrap();
    assert_eq!(concept.confidence, 0.95, "Confidence should take the max");
    assert_eq!(
        concept.updated_at, "2026-02-19T00:00:00Z",
        "updated_at should be refreshed"
    );
    assert!(
        concept.aliases.contains(&"bot".to_string()),
        "New alias should be merged"
    );
    assert_eq!(store.concept_count(), 1, "Still only one concept");
}

#[test]
fn label_index_normalizes_on_lookup() {
    let mut store = MemoryStore::new();
    let record = make_record(vec![("agent", 0.9)], vec![], "test_scope");
    store.insert_record(record);

    assert!(
        store.get_concept_by_label("Agent").is_some(),
        "Lookup with uppercase should find normalized lowercase entry"
    );
    assert!(
        store.get_concept_by_label("  agent  ").is_some(),
        "Lookup with whitespace should find trimmed entry"
    );
    assert!(
        store.get_concept_by_label("AGENT").is_some(),
        "Lookup with all-caps should find entry"
    );
}

#[test]
fn insert_result_tracks_new_facts_vs_episodes() {
    let mut store = MemoryStore::new();

    let record1 = make_record(
        vec![("agent", 0.9), ("memory", 0.9)],
        vec![("agent", "needs", "memory", 0.98)],
        "session_1",
    );
    let r1 = store.insert_record(record1);
    assert_eq!(r1.new_facts, 1);
    assert_eq!(r1.new_episodes, 1);

    let record2 = make_record(
        vec![("agent", 0.9), ("memory", 0.9)],
        vec![("agent", "needs", "memory", 0.95)],
        "session_2",
    );
    let r2 = store.insert_record(record2);
    assert_eq!(r2.new_facts, 0, "Same fact should not count as new");
    assert_eq!(
        r2.new_episodes, 1,
        "Different context should produce new episode"
    );
}
