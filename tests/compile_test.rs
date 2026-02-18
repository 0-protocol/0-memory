use zero_memory::compiler::emitter::compile;
use zero_memory::types::{CompilerInput, ContextMeta, SemanticTuple};

fn sample_input() -> CompilerInput {
    CompilerInput {
        utterance: Some("Agent needs long-term memory, 0-memory solves it with 0-lang".to_string()),
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
fn compile_produces_correct_concept_count() {
    let output = compile(&sample_input());
    // "agent", "longtermmemory", "0-memory", "0-lang" = 4 unique concepts
    assert_eq!(
        output.record.concepts.len(),
        4,
        "Should dedup shared concepts: got {:?}",
        output
            .record
            .concepts
            .iter()
            .map(|c| &c.label)
            .collect::<Vec<_>>()
    );
}

#[test]
fn compile_produces_correct_relation_count() {
    let output = compile(&sample_input());
    assert_eq!(
        output.record.relations.len(),
        3,
        "Should have one relation per tuple"
    );
}

#[test]
fn compile_graph_text_uses_mergemap_not_aggregate() {
    let output = compile(&sample_input());
    assert!(
        output.graph_text.contains("MergeMap"),
        "Graph text must use MergeMap, got:\n{}",
        output.graph_text
    );
    assert!(
        !output.graph_text.contains("Aggregate"),
        "Graph text must NOT contain Aggregate"
    );
}

#[test]
fn compile_graph_text_contains_expected_labels() {
    let output = compile(&sample_input());
    let text = &output.graph_text;
    assert!(
        text.contains("agent"),
        "Must contain normalized 'agent' label"
    );
    assert!(text.contains("0-memory"), "Must contain '0-memory' label");
    assert!(text.contains("0-lang"), "Must contain '0-lang' label");
}

#[test]
fn compile_graph_text_is_valid_structure() {
    let output = compile(&sample_input());
    let text = &output.graph_text;
    assert!(text.starts_with("Graph {"), "Must start with 'Graph {{'");
    assert!(
        text.contains("\"nodes\""),
        "Must contain '\"nodes\"' section"
    );
    assert!(text.contains("\"entry_point\""), "Must contain entry_point");
    assert!(text.contains("\"outputs\""), "Must contain outputs");
}

#[test]
fn compile_context_node_is_populated() {
    let output = compile(&sample_input());
    assert_eq!(
        output.record.context.meta.event_time,
        "2026-02-18T00:00:00Z"
    );
    assert_eq!(output.record.context.meta.source, "user_prompt");
    assert_eq!(output.record.context.meta.scope, "0-memory_design");
}

#[test]
fn compile_relations_have_valid_hashes() {
    let output = compile(&sample_input());
    for rel in &output.record.relations {
        assert_ne!(rel.fact_hash.0, [0u8; 32], "FactHash must not be zero");
        assert_ne!(
            rel.episode_hash.0, [0u8; 32],
            "EpisodeHash must not be zero"
        );
        assert_ne!(
            rel.context_hash.0, [0u8; 32],
            "ContextHash must not be zero"
        );
    }
}

#[test]
fn compile_all_relations_share_same_context_hash() {
    let output = compile(&sample_input());
    let first_ctx = &output.record.relations[0].context_hash;
    for rel in &output.record.relations {
        assert_eq!(
            &rel.context_hash, first_ctx,
            "All relations in one compile call share the same context"
        );
    }
}
