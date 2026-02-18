use zero_openclaw::runtime::{parse_graph, GraphInterpreter};

#[tokio::test]
async fn parse_and_execute_example_memory() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/example_memory.0"
    ))
    .expect("examples/example_memory.0 should exist");

    assert!(!source.is_empty());
    assert!(
        source.contains("MergeMap"),
        "example_memory.0 must use MergeMap (compat fix applied)"
    );
    assert!(
        !source.contains("\"Aggregate\""),
        "example_memory.0 must not use Aggregate node type"
    );

    let graph = parse_graph(&source).expect("example_memory.0 must parse through 0-openclaw");
    assert!(!graph.nodes.is_empty(), "Parsed graph must have nodes");

    let interp = GraphInterpreter::default();
    let inputs = std::collections::HashMap::new();
    let result = interp
        .execute(&graph, inputs)
        .await
        .expect("Graph execution must succeed");

    assert!(
        result.outputs.contains_key("output"),
        "Execution output must contain 'output' key, got keys: {:?}",
        result.outputs.keys().collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn parse_and_execute_schema() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/schema/schema.0"))
        .expect("schema/schema.0 should exist");

    assert!(!source.is_empty());
    assert!(
        source.contains("MergeMap"),
        "schema.0 must use MergeMap (compat fix applied)"
    );

    let graph = parse_graph(&source).expect("schema.0 must parse through 0-openclaw");
    assert!(!graph.nodes.is_empty(), "Schema graph must have nodes");

    let interp = GraphInterpreter::default();
    let inputs = std::collections::HashMap::new();
    let result = interp
        .execute(&graph, inputs)
        .await
        .expect("Schema graph execution must succeed");

    assert!(
        result.outputs.contains_key("output"),
        "Schema execution output must contain 'output' key"
    );
}

#[test]
fn canonical_files_are_aggregate_free() {
    let schema = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/schema/schema.0"))
        .expect("schema/schema.0 must exist");
    let example = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/example_memory.0"
    ))
    .expect("examples/example_memory.0 must exist");

    assert!(
        !schema.contains("Aggregate"),
        "schema/schema.0 must not contain Aggregate in any form"
    );
    assert!(
        !example.contains("Aggregate"),
        "examples/example_memory.0 must not contain Aggregate in any form"
    );
    assert!(schema.contains("MergeMap"));
    assert!(example.contains("MergeMap"));
}

/// Round-trip test: compile a CompilerInput → emit .0 graph text →
/// parse through 0-openclaw → execute.
///
/// This is the critical integration gap identified in code review (X3/F5).
/// It verifies the full pipeline end-to-end through the actual runtime.
#[tokio::test]
async fn compiler_output_parses_and_executes() {
    use zero_memory::compiler::emitter::compile;
    use zero_memory::types::{CompilerInput, ContextMeta, SemanticTuple};

    let input = CompilerInput {
        utterance: Some("Test round-trip".to_string()),
        tuples: vec![
            SemanticTuple {
                subject: "Agent".to_string(),
                predicate: "needs".to_string(),
                object: "Memory".to_string(),
                confidence: 0.95,
            },
            SemanticTuple {
                subject: "0-memory".to_string(),
                predicate: "provides".to_string(),
                object: "Memory".to_string(),
                confidence: 0.90,
            },
        ],
        context: ContextMeta {
            event_time: "20260218T000000Z".to_string(),
            source: "unit_test".to_string(),
            scope: "round_trip_test".to_string(),
            agent_id: None,
            session_id: None,
            metadata: None,
        },
    };

    let output = compile(&input);
    assert!(
        output.graph_text.contains("MergeMap"),
        "Compiler output must use MergeMap"
    );

    // This parse will fail if the emitter has escaping bugs (B1),
    // colon issues (B2/X1), or CreateMap API mismatches (B3/X2).
    let parse_result = parse_graph(&output.graph_text);
    match parse_result {
        Ok(graph) => {
            assert!(!graph.nodes.is_empty(), "Parsed graph must have nodes");

            let interp = GraphInterpreter::default();
            let inputs = std::collections::HashMap::new();
            let exec_result = interp.execute(&graph, inputs).await;
            match exec_result {
                Ok(result) => {
                    assert!(
                        result.outputs.contains_key("output"),
                        "Execution must produce 'output' key, got: {:?}",
                        result.outputs.keys().collect::<Vec<_>>()
                    );
                }
                Err(e) => {
                    panic!(
                        "Compiler-emitted graph parsed but failed to execute: {}\n\nGraph text:\n{}",
                        e, output.graph_text
                    );
                }
            }
        }
        Err(e) => {
            panic!(
                "Compiler-emitted .0 text failed to parse through 0-openclaw: {}\n\nGraph text:\n{}",
                e, output.graph_text
            );
        }
    }
}
