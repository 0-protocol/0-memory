# 0-memory

**Agent-native memory, compiled with 0-lang.**

0-memory gives AI agents a structured, content-addressed memory system. Instead of stuffing raw text into vector stores, 0-memory compiles observations into executable graph records — where every concept is hashed, every relation is typed, and every recall is a graph traversal, not a full-context replay.

```
utterance ──► semantic tuples ──► compiler ──► .0 graph ──► store
                                                              │
                          recall ◄── traversal ◄── query ◄────┘
```

---

## Why Not Just RAG?

| | RAG / Embedding Store | 0-memory |
|---|---|---|
| **Storage** | Chunked text + float vectors | Content-addressed concept graphs |
| **Dedup** | Approximate (cosine similarity) | Exact (SHA-256 hash identity) |
| **Recall** | Top-K nearest neighbor | Ranked graph traversal with confidence decay |
| **Structure** | Flat document chunks | `Concept → Relation → Context` triples |
| **Verifiability** | None | Proof-carrying execution traces |
| **Format** | Opaque embeddings | Executable `.0` graphs (readable, diffable) |

0-memory doesn't replace embeddings — it gives agents a **semantic backbone** that embeddings can attach to.

---

## Core Data Model

```
┌─────────────┐      predicate       ┌─────────────┐
│   Concept    │─────────────────────►│   Concept    │
│  hash: H(l)  │    confidence: 0.98   │  hash: H(l)  │
└──────┬───────┘                      └──────────────┘
       │
       │  observed in
       ▼
┌─────────────────┐
│     Context      │
│  time, source,   │
│  scope, agent    │
└─────────────────┘
```

| Unit | Description | Identity |
|------|-------------|----------|
| **Concept** | An entity or idea atom (`"agent"`, `"0-memory"`, `"long-term-memory"`) | `ConceptHash = sha256(normalized_label)` |
| **Relation** | A typed, confidence-weighted edge between two concepts | `FactHash = sha256(subject \| predicate \| object)` |
| **Context** | When, where, and how an observation was made | `ContextHash = sha256(time \| source \| scope)` |
| **Episode** | A specific observation of a fact in a context | `EpisodeHash = sha256(FactHash + ContextHash)` |
| **MemoryRecord** | A batch of concepts + relations + context + proof | Compiled `.0` graph |

### Two-Layer Hash Design

The same fact observed twice produces **one FactHash** but **two EpisodeHashes** — separating "what was learned" from "when it was learned." This enables semantic dedup without losing temporal history.

---

## Project Structure

```
0-memory/
├── Cargo.toml
├── src/
│   ├── lib.rs                       # crate root
│   ├── types.rs                     # shared type contract (all hash newtypes, nodes, records)
│   ├── compiler/
│   │   ├── normalizer.rs            # label canonicalization + alias table
│   │   ├── hasher.rs                # two-layer hashing (ConceptHash, FactHash, EpisodeHash)
│   │   └── emitter.rs               # SemanticTuple[] → .0 graph text + MemoryRecord
│   ├── store/
│   │   ├── graph.rs                 # in-memory concept/relation/context store with dedup
│   │   └── index.rs                 # label → hash reverse index
│   ├── runtime_trait.rs             # MemoryRuntime trait (runtime-agnostic interface)
│   └── adapters/
│       └── openclaw.rs              # MemoryRuntime impl for 0-openclaw (feature-gated)
├── schema/
│   ├── schema.0                     # canonical schema graph (executable)
│   └── compatibility.md             # 0-memory ↔ 0-openclaw node mapping
├── examples/
│   └── example_memory.0             # compiled memory example (executable)
└── tests/
    ├── hash_test.rs                 # hash determinism, stability, separation
    ├── compile_test.rs              # compiler output correctness
    ├── store_test.rs                # insert, dedup, retrieval, adjacency
    ├── compat_test.rs               # .0 files parse + execute on 0-openclaw
    └── integration_test.rs          # full pipeline: compile → store → recall
```

---

## Quick Start

### Build

```bash
cargo build
```

### Test

```bash
# Run all tests (requires 0-openclaw as sibling directory)
cargo test

# Run without 0-openclaw integration
cargo test --lib
```

### Use as a Library

```rust
use zero_memory::compiler::emitter::compile;
use zero_memory::store::MemoryStore;
use zero_memory::types::*;

// 1. Build input from semantic tuples
let input = CompilerInput {
    utterance: Some("An agent needs long-term memory".into()),
    tuples: vec![
        SemanticTuple {
            subject: "Agent".into(),
            predicate: "needs".into(),
            object: "LongTermMemory".into(),
            confidence: 0.98,
        },
    ],
    context: ContextMeta {
        event_time: "2026-02-18T00:00:00Z".into(),
        source: "user_prompt".into(),
        scope: "conversation_42".into(),
        agent_id: None,
        session_id: None,
        metadata: None,
    },
};

// 2. Compile → .0 graph text + structured MemoryRecord
let output = compile(&input);

// 3. Store (deduplicates by hash)
let mut store = MemoryStore::new();
let result = store.insert_record(output.record);
// result.new_concepts = 2, result.new_facts = 1, result.dupes_skipped = 0

// 4. Recall by label
let concept = store.get_concept_by_label("agent").unwrap();
let relations = store.get_relations(&concept.hash);
```

---

## Compilation Pipeline

```
   SemanticTuple[]        ContextMeta
        │                      │
        ▼                      ▼
  ┌───────────┐         ┌───────────┐
  │ normalize  │         │  context  │
  │  labels &  │         │   hash    │
  │ predicates │         └─────┬─────┘
  └─────┬──────┘               │
        ▼                      │
  ┌───────────┐                │
  │  concept   │                │
  │  hashes    │                │
  └─────┬──────┘               │
        ▼                      │
  ┌───────────┐                │
  │   fact     │◄──────────────┘
  │  hashes    │   (episode_hash = sha256(fact + ctx))
  └─────┬──────┘
        ▼
  ┌───────────┐
  │   emit    │──► .0 graph text  (Constant + Operation nodes, MergeMap output)
  │  record   │──► MemoryRecord   (structured, ready for store)
  └───────────┘
```

All `.0` graphs use only node types that exist in the 0-openclaw runtime: `Constant` and `Operation` (with builtins `Hash`, `CreateMap`, `SetField`, `MergeMap`). No custom node types required.

---

## Runtime Compatibility

0-memory is designed to be runtime-agnostic via the `MemoryRuntime` trait:

```rust
pub trait MemoryRuntime {
    type Value: Clone + std::fmt::Debug;
    type Hash: AsRef<[u8]> + Clone;
    type Error: std::fmt::Display;

    fn hash(&self, input: &[u8]) -> Self::Hash;
    fn execute_graph(&self, source: &str, inputs: HashMap<String, Self::Value>)
        -> Result<HashMap<String, Self::Value>, Self::Error>;
    fn load_state(&self, key: &str) -> Result<Option<Self::Value>, Self::Error>;
    fn save_state(&self, key: &str, value: &Self::Value) -> Result<(), Self::Error>;
}
```

| Runtime | Status | Feature Flag |
|---------|--------|--------------|
| **0-openclaw** | Implemented | `--features openclaw` |
| **0-chain** | Planned (awaiting executor) | — |

---

## Roadmap

| Phase | Scope | Status |
|-------|-------|--------|
| **Compat Fix** | Rewrite `.0` files: `Aggregate` → `MergeMap` | Done |
| **MVP** | Compile → Store → Recall by hash | Done |
| **V1** | Memory hierarchy (working / episodic / semantic), decay, ranking | Planned |
| **V2** | `.0c` compact format, progressive detail levels, delta encoding | Planned |
| **V3** | 0-chain persistence, query DSL, multi-agent shared memory | Planned |

See [`EXECUTION_PLAN.md`](EXECUTION_PLAN.md) for full technical design and milestone breakdown.

---

## License

Apache-2.0
