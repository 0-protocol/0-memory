use zero_memory::compiler::emitter::compile;
use zero_memory::compiler::hasher;
use zero_memory::compiler::normalizer::normalize_label;
use zero_memory::store::MemoryStore;
use zero_memory::types::{CompilerInput, ContextMeta, SemanticTuple};

fn build_test_input() -> CompilerInput {
    CompilerInput {
        utterance: Some(
            "Build 0-memory, a native agent memory system compiled with 0-lang".to_string(),
        ),
        tuples: vec![
            SemanticTuple {
                subject: "Agent".to_string(),
                predicate: "needs".to_string(),
                object: "LongTermMemory".to_string(),
                confidence: 0.98,
            },
            SemanticTuple {
                subject: "0-memory".to_string(),
                predicate: "solves".to_string(),
                object: "LongTermMemory".to_string(),
                confidence: 0.97,
            },
            SemanticTuple {
                subject: "0-memory".to_string(),
                predicate: "compiled_with".to_string(),
                object: "0-lang".to_string(),
                confidence: 0.99,
            },
        ],
        context: ContextMeta {
            event_time: "2026-02-18T00:00:00Z".to_string(),
            source: "user_prompt".to_string(),
            scope: "0-memory_design".to_string(),
            agent_id: None,
            session_id: None,
            metadata: None,
        },
    }
}

#[test]
fn full_pipeline_compile_store_retrieve() {
    // Step 1: Build compiler input from raw tuples
    let input = build_test_input();

    // Step 2: Compile
    let output = compile(&input);
    assert_eq!(output.record.concepts.len(), 4);
    assert_eq!(output.record.relations.len(), 3);
    assert!(output.graph_text.contains("MergeMap"));

    // Step 3: Store
    let mut store = MemoryStore::new();
    let result = store.insert_record(output.record);
    assert_eq!(result.new_concepts, 4);
    assert_eq!(result.new_facts, 3);
    assert_eq!(result.new_episodes, 3);
    assert_eq!(result.dupes_skipped, 0);

    // Step 4: Retrieve concept by label
    let label_0memory = normalize_label("0-memory");
    let concept = store
        .get_concept_by_label(&label_0memory)
        .expect("0-memory concept must exist in store");
    assert_eq!(concept.label, label_0memory);

    // Step 5: Retrieve relations for the concept
    let rels = store.get_relations(&concept.hash);
    assert!(
        rels.len() >= 2,
        "0-memory should have at least 2 relations (solves + compiled_with), got {}",
        rels.len()
    );

    // Step 6: Verify round-trip semantics
    let predicates: Vec<&str> = rels.iter().map(|r| r.predicate.as_str()).collect();
    assert!(
        predicates.contains(&"solves"),
        "Must contain 'solves' relation"
    );
    assert!(
        predicates.contains(&"compiled_with"),
        "Must contain 'compiled_with' relation"
    );
}

#[test]
fn full_pipeline_dedup_on_second_insert() {
    let input = build_test_input();
    let output1 = compile(&input);
    let output2 = compile(&input);

    let mut store = MemoryStore::new();

    let r1 = store.insert_record(output1.record);
    assert_eq!(r1.new_concepts, 4);

    // Same input compiled again → same hashes → all deduped
    let r2 = store.insert_record(output2.record);
    assert_eq!(r2.new_concepts, 0, "All concepts should be deduped");
    assert_eq!(
        r2.new_episodes, 0,
        "Same context → same episode hashes → deduped"
    );
    assert_eq!(store.concept_count(), 4);
}

#[test]
fn full_pipeline_same_facts_new_context_creates_new_episodes() {
    let input1 = build_test_input();
    let mut input2 = build_test_input();
    input2.context.scope = "different_session".to_string();
    input2.context.event_time = "2026-02-19T00:00:00Z".to_string();

    let output1 = compile(&input1);
    let output2 = compile(&input2);

    let mut store = MemoryStore::new();
    store.insert_record(output1.record);
    let r2 = store.insert_record(output2.record);

    assert_eq!(r2.new_concepts, 0, "Same concepts already exist");
    assert_eq!(
        r2.new_episodes, 3,
        "Different context must produce new episodes"
    );
    assert_eq!(
        store.relation_count(),
        6,
        "3 episodes per context × 2 contexts"
    );
}

#[test]
fn full_pipeline_concept_hash_matches_hasher_output() {
    let input = build_test_input();
    let output = compile(&input);

    let mut store = MemoryStore::new();
    store.insert_record(output.record);

    let label = normalize_label("Agent");
    let expected_hash = hasher::concept_hash(&label);

    let concept = store.get_concept_by_label(&label).unwrap();
    assert_eq!(
        concept.hash, expected_hash,
        "Stored concept hash must match direct hasher output"
    );
}

#[test]
fn full_pipeline_graph_text_nonempty_and_valid() {
    let input = build_test_input();
    let output = compile(&input);

    assert!(!output.graph_text.is_empty());
    assert!(output.graph_text.contains("Graph {"));
    assert!(output.graph_text.contains("\"MergeMap\""));
    assert!(!output.graph_text.contains("Aggregate"));
}
