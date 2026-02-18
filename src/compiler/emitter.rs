use std::collections::HashMap;

use crate::compiler::hasher;
use crate::compiler::normalizer::{normalize_predicate, AliasTable};
use crate::types::*;

pub use crate::types::CompilerOutput;

/// Strip colons from a string value before embedding in `.0` graph text.
///
/// The 0-openclaw parser applies a `word:` → `"word":` regex across the entire
/// source, including the interior of quoted strings.  Any colon inside a value
/// (e.g., ISO 8601 `T00:00:00Z`) triggers false-positive key quoting and
/// produces unparseable JSON.  Stripping colons is the simplest safe workaround
/// (see `compatibility.md` Gotcha #5).
fn sanitize_for_graph(s: &str) -> String {
    s.replace(':', "")
}

/// Compile raw semantic tuples + context into a `.0` graph and structured record.
///
/// Pipeline:
/// 1. Resolve aliases and normalize all concept labels and predicates
/// 2. Compute ConceptHash, FactHash, ContextHash, EpisodeHash
/// 3. Deduplicate concepts by label
/// 4. Build MemoryRecord
/// 5. Emit `.0` graph text using only Constant, Operation, SetField nodes
pub fn compile(input: &CompilerInput) -> CompilerOutput {
    let alias_table = AliasTable::with_defaults();
    let ctx_hash = hasher::context_hash(&input.context);
    let mut concept_map: HashMap<String, ConceptNode> = HashMap::new();
    let mut relations = Vec::new();
    let now = input.context.event_time.clone();

    for tuple in &input.tuples {
        let subj_label = alias_table.resolve(&tuple.subject);
        let obj_label = alias_table.resolve(&tuple.object);
        let pred = normalize_predicate(&tuple.predicate);

        let subj_hash = hasher::concept_hash(&subj_label);
        let obj_hash = hasher::concept_hash(&obj_label);

        concept_map
            .entry(subj_label.clone())
            .or_insert_with(|| ConceptNode {
                hash: subj_hash.clone(),
                label: subj_label.clone(),
                aliases: vec![],
                confidence: tuple.confidence,
                created_at: now.clone(),
                updated_at: now.clone(),
            });

        concept_map
            .entry(obj_label.clone())
            .or_insert_with(|| ConceptNode {
                hash: obj_hash.clone(),
                label: obj_label.clone(),
                aliases: vec![],
                confidence: tuple.confidence,
                created_at: now.clone(),
                updated_at: now.clone(),
            });

        let fh = hasher::fact_hash(&subj_label, &pred, &obj_label);
        let eh = hasher::episode_hash(&fh, &ctx_hash);

        relations.push(RelationNode {
            fact_hash: fh,
            episode_hash: eh,
            subject_hash: subj_hash,
            predicate: pred,
            object_hash: obj_hash,
            confidence: tuple.confidence,
            context_hash: ctx_hash.clone(),
            created_at: now.clone(),
        });
    }

    let mut concepts: Vec<ConceptNode> = concept_map.into_values().collect();
    concepts.sort_by(|a, b| a.label.cmp(&b.label));

    let context_node = ContextNode {
        hash: ctx_hash,
        meta: input.context.clone(),
    };

    let record = MemoryRecord {
        concepts,
        relations,
        context: context_node,
    };

    let graph_text = emit_graph_text(&record, &input.context);

    CompilerOutput { graph_text, record }
}

/// Serialize a MemoryRecord into `.0` graph text format.
///
/// All inputs to `MergeMap` must be `Value::Map`.  The emitter wraps every
/// value via `SetField` (from a shared `empty_map` created by `CreateMap`)
/// under a unique key so that merged maps never collide.
///
/// String values are JSON-escaped via [`serde_json::to_string`] and sanitized
/// for the 0-openclaw parser (colons stripped).
pub fn emit_graph_text(record: &MemoryRecord, context: &ContextMeta) -> String {
    let mut nodes = Vec::<String>::new();
    let mut merge_inputs = Vec::<String>::new();

    // Shared empty map — every SetField chain starts from this node
    nodes.push(
        r#"        { "id": "empty_map", "type": "Operation", "op": "CreateMap", "inputs": [], "params": {} }"#
            .to_string(),
    );

    // --- Concept nodes ---
    for (i, c) in record.concepts.iter().enumerate() {
        let label_id = format!("concept_label_{}", i);
        let hash_id = format!("concept_hash_{}", i);
        let set_label_id = format!("concept_slabel_{}", i);
        let data_id = format!("concept_data_{}", i);
        let wrap_id = format!("concept_{}", i);

        let escaped_label = serde_json::to_string(&sanitize_for_graph(&c.label)).unwrap();

        nodes.push(format!(
            r#"        {{ "id": "{label_id}", "type": "Constant", "value": {escaped_label} }}"#
        ));

        nodes.push(format!(
            r#"        {{ "id": "{hash_id}", "type": "Operation", "op": "Hash", "inputs": ["{label_id}"] }}"#
        ));

        // Build { "label": <label>, "hash": <hash> } via chained SetField
        nodes.push(format!(
            r#"        {{ "id": "{set_label_id}", "type": "Operation", "op": "SetField", "inputs": ["empty_map", "{label_id}"], "params": {{ "field": "label" }} }}"#
        ));
        nodes.push(format!(
            r#"        {{ "id": "{data_id}", "type": "Operation", "op": "SetField", "inputs": ["{set_label_id}", "{hash_id}"], "params": {{ "field": "hash" }} }}"#
        ));

        // Wrap under unique key so MergeMap inputs don't collide
        nodes.push(format!(
            r#"        {{ "id": "{wrap_id}", "type": "Operation", "op": "SetField", "inputs": ["empty_map", "{data_id}"], "params": {{ "field": "{wrap_id}" }} }}"#
        ));

        merge_inputs.push(wrap_id);
    }

    // --- Context node ---
    let ctx_value = serde_json::json!({
        "event_time": sanitize_for_graph(&context.event_time),
        "source": sanitize_for_graph(&context.source),
        "scope": sanitize_for_graph(&context.scope),
    });
    nodes.push(format!(
        r#"        {{ "id": "context", "type": "Constant", "value": {} }}"#,
        serde_json::to_string(&ctx_value).unwrap()
    ));

    nodes.push(
        r#"        { "id": "context_hash", "type": "Operation", "op": "Hash", "inputs": ["context"] }"#
            .to_string(),
    );

    nodes.push(
        r#"        { "id": "context_map", "type": "Operation", "op": "SetField", "inputs": ["empty_map", "context"], "params": { "field": "context" } }"#
            .to_string(),
    );
    nodes.push(
        r#"        { "id": "context_wrapped", "type": "Operation", "op": "SetField", "inputs": ["context_map", "context_hash"], "params": { "field": "context_hash" } }"#
            .to_string(),
    );
    merge_inputs.push("context_wrapped".to_string());

    // --- Relation nodes ---
    for (i, r) in record.relations.iter().enumerate() {
        let rel_id = format!("rel_{}", i);
        let wrap_id = format!("wrap_rel_{}", i);

        let rel_value = serde_json::json!({
            "subject_hash": r.subject_hash.to_string(),
            "predicate": sanitize_for_graph(&r.predicate),
            "object_hash": r.object_hash.to_string(),
            "confidence": r.confidence,
            "fact_hash": r.fact_hash.to_string(),
            "episode_hash": r.episode_hash.to_string(),
        });
        nodes.push(format!(
            r#"        {{ "id": "{rel_id}", "type": "Constant", "value": {} }}"#,
            serde_json::to_string(&rel_value).unwrap()
        ));

        nodes.push(format!(
            r#"        {{ "id": "{wrap_id}", "type": "Operation", "op": "SetField", "inputs": ["empty_map", "{rel_id}"], "params": {{ "field": "{rel_id}" }} }}"#
        ));
        merge_inputs.push(wrap_id);
    }

    // --- Proof placeholder ---
    nodes.push(
        r#"        { "id": "proof", "type": "Constant", "value": { "trace_hash": "pending", "signer": "0-memory-compiler", "signature": "pending" } }"#
            .to_string(),
    );
    nodes.push(
        r#"        { "id": "wrap_proof", "type": "Operation", "op": "SetField", "inputs": ["empty_map", "proof"], "params": { "field": "proof" } }"#
            .to_string(),
    );
    merge_inputs.push("wrap_proof".to_string());

    // --- MergeMap output ---
    let inputs_str = merge_inputs
        .iter()
        .map(|s| format!("\"{}\"", s))
        .collect::<Vec<_>>()
        .join(", ");

    nodes.push(format!(
        r#"        {{ "id": "output", "type": "Operation", "op": "MergeMap", "inputs": [{}] }}"#,
        inputs_str
    ));

    let nodes_block = nodes.join(",\n");

    let entry = if record.concepts.is_empty() {
        "context"
    } else {
        "concept_label_0"
    };

    format!(
        r#"Graph {{
    "name": "zero_memory_compiled",
    "version": 1,
    "description": "Compiled memory graph from 0-memory",
    "nodes": [
{}
    ],
    "entry_point": "{entry}",
    "outputs": ["output"],
    "metadata": {{ "author": "0-memory", "tags": ["memory", "compiled"] }}
}}"#,
        nodes_block
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::normalizer::normalize_label;

    fn sample_input() -> CompilerInput {
        CompilerInput {
            utterance: Some("An agent needs long-term memory".into()),
            tuples: vec![
                SemanticTuple {
                    subject: "Agent".into(),
                    predicate: "needs".into(),
                    object: "Long Term Memory".into(),
                    confidence: 0.98,
                },
                SemanticTuple {
                    subject: "0-memory".into(),
                    predicate: "solves".into(),
                    object: "Long Term Memory".into(),
                    confidence: 0.97,
                },
                SemanticTuple {
                    subject: "0-memory".into(),
                    predicate: "uses".into(),
                    object: "Content Addressing".into(),
                    confidence: 0.95,
                },
            ],
            context: ContextMeta {
                event_time: "2026-02-18T00:00:00Z".into(),
                source: "user_prompt".into(),
                scope: "design_session".into(),
                agent_id: None,
                session_id: None,
                metadata: None,
            },
        }
    }

    #[test]
    fn compile_deduplicates_concepts() {
        let output = compile(&sample_input());
        let labels: Vec<&str> = output
            .record
            .concepts
            .iter()
            .map(|c| c.label.as_str())
            .collect();
        let unique: std::collections::HashSet<&&str> = labels.iter().collect();
        assert_eq!(labels.len(), unique.len(), "concepts must be deduplicated");
        assert_eq!(output.record.concepts.len(), 4);
    }

    #[test]
    fn compile_produces_correct_relation_count() {
        let output = compile(&sample_input());
        assert_eq!(output.record.relations.len(), 3);
    }

    #[test]
    fn graph_text_uses_mergemap() {
        let output = compile(&sample_input());
        assert!(
            output.graph_text.contains("MergeMap"),
            "must use MergeMap, not Aggregate"
        );
        assert!(
            !output.graph_text.contains("Aggregate"),
            "must not contain Aggregate"
        );
    }

    #[test]
    fn graph_text_uses_setfield_not_createmap_for_wrapping() {
        let output = compile(&sample_input());
        assert!(
            output.graph_text.contains("SetField"),
            "concept wrapping must use SetField"
        );
        let createmap_count = output.graph_text.matches("CreateMap").count();
        assert_eq!(
            createmap_count, 1,
            "only one CreateMap (the shared empty_map) should exist"
        );
    }

    #[test]
    fn graph_text_contains_all_concepts() {
        let output = compile(&sample_input());
        assert!(output.graph_text.contains("agent"));
        assert!(output.graph_text.contains("long term memory"));
        assert!(output.graph_text.contains("0-memory"));
        assert!(output.graph_text.contains("content addressing"));
    }

    #[test]
    fn graph_text_has_proof_node() {
        let output = compile(&sample_input());
        assert!(output.graph_text.contains("trace_hash"));
        assert!(output.graph_text.contains("pending"));
    }

    #[test]
    fn graph_text_sanitizes_colons() {
        let output = compile(&sample_input());
        assert!(
            output.graph_text.contains("2026-02-18T000000Z"),
            "colons must be stripped from timestamps in graph text"
        );
        assert!(
            !output.graph_text.contains("00:00"),
            "no colons should remain in graph text values"
        );
        assert!(output.graph_text.contains("user_prompt"));
        assert!(output.graph_text.contains("design_session"));
    }

    #[test]
    fn graph_text_string_values_are_json_escaped() {
        let mut input = sample_input();
        input.tuples.push(SemanticTuple {
            subject: r#"tricky "quoted" label"#.into(),
            predicate: "tests".into(),
            object: "escaping".into(),
            confidence: 0.5,
        });
        let output = compile(&input);
        let normalized = normalize_label(r#"tricky "quoted" label"#);
        let escaped = serde_json::to_string(&sanitize_for_graph(&normalized)).unwrap();
        assert!(
            output.graph_text.contains(&escaped),
            "label with quotes must be JSON-escaped in graph text"
        );
    }

    #[test]
    fn compile_resolves_aliases() {
        let input = CompilerInput {
            utterance: None,
            tuples: vec![SemanticTuple {
                subject: "Agent".into(),
                predicate: "needs".into(),
                object: "LTM".into(),
                confidence: 0.9,
            }],
            context: ContextMeta {
                event_time: "20260218T000000Z".into(),
                source: "test".into(),
                scope: "alias_test".into(),
                agent_id: None,
                session_id: None,
                metadata: None,
            },
        };
        let output = compile(&input);
        let labels: Vec<&str> = output
            .record
            .concepts
            .iter()
            .map(|c| c.label.as_str())
            .collect();
        assert!(
            labels.contains(&"long-term-memory"),
            "LTM should resolve to long-term-memory, got: {:?}",
            labels
        );
    }

    #[test]
    fn empty_tuples_graph_is_valid() {
        let input = CompilerInput {
            utterance: None,
            tuples: vec![],
            context: ContextMeta {
                event_time: "20260218T000000Z".into(),
                source: "test".into(),
                scope: "empty".into(),
                agent_id: None,
                session_id: None,
                metadata: None,
            },
        };
        let output = compile(&input);
        assert!(output.graph_text.contains("\"entry_point\": \"context\""));
        assert!(output.record.concepts.is_empty());
    }
}
