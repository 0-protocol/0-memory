# 0-memory Execution Plan v2

> Revised after code-level audit of 0-openclaw runtime and feedback review.
> Every decision below is grounded in actual types from `0-openclaw/src/runtime/`.

---

## 0. Baseline Audit — What Actually Exists

### 0-openclaw Runtime (the only working 0-lang runtime)

| Component | Location | Status |
|---|---|---|
| `NodeType` enum | `0-openclaw/src/runtime/types.rs:118` | 6 variants: `External`, `Operation`, `Constant`, `Lookup`, `Route`, `Permission` |
| `Graph` struct | `0-openclaw/src/runtime/types.rs:172` | `name`, `version`, `description`, `nodes: Vec<GraphNode>`, `outputs`, `entry_point`, `metadata` |
| `GraphNode` struct | `0-openclaw/src/runtime/types.rs:152` | `id`, `node_type` (flattened), `inputs: Vec<String>`, `params: serde_json::Value` |
| `Value` enum | `0-openclaw/src/runtime/types.rs:10` | `Null`, `Bool`, `Int`, `Float`, `String`, `Bytes`, `Array`, `Map`, `Hash([u8;32])`, `Confidence(f64)` |
| Parser | `0-openclaw/src/runtime/types.rs:273` | `parse_graph_from_source()` — strips comments, converts to JSON, deserializes |
| Interpreter | `0-openclaw/src/runtime/interpreter.rs` | Topo-sort → execute each node → collect outputs → compute execution hash |
| Builtins (33) | `0-openclaw/src/runtime/builtins.rs` | Includes `Hash` (SHA-256), `Sign`, `Verify`, `CreateMap`, `MergeMap`, `GetField`, `SetField`, `LoadState`, `SaveState`, `Concat`, math, logic, comparison ops |

### 0-chain Runtime

| Component | Status |
|---|---|
| Graph parser | **Does not exist** — graphs stored as raw `Vec<u8>` |
| Node types | **Not defined** — spec only mentions intended ops |
| Executor | Placeholder with TODO comments |

### Current 0-memory Files

| File | Nodes Used | Runtime Compatible? |
|---|---|---|
| `schema.0` | `Constant` ×5, `Aggregate` ×1 | **NO** — `Aggregate` not in `NodeType` |
| `example_memory.0` | `Constant` ×13, `Operation{Hash}` ×7, `Aggregate` ×1 | **PARTIAL** — `Constant` and `Operation{Hash}` work, `Aggregate` does not |

### Verdict

**The `Aggregate` node type used in both `.0` files does not exist in any runtime.**
Everything else (`Constant`, `Operation { op: "Hash" }`) maps 1:1 to 0-openclaw's
`NodeType` and `BuiltinRegistry`.

---

## 1. Compatibility Fix — Before Any New Code

### Decision: Rewrite `Aggregate` → `Operation { op: "MergeMap" }`

The 0-openclaw runtime already has `MergeMap` which merges multiple `Value::Map`
inputs into one. This is semantically equivalent to `Aggregate` for our use case.

**Option A (chosen)**: Rewrite `.0` files to use only existing `NodeType` variants.
Zero changes to 0-openclaw. 0-memory works on the existing runtime today.

**Option B (deferred)**: Add `Aggregate` to 0-openclaw's `NodeType` enum.
This requires changing the shared runtime — unnecessary for MVP.

### Action items

- [ ] **Rewrite `schema.0`**: Replace `Aggregate` output node with
      `Operation { op: "MergeMap" }` that merges all schema constants into a
      single `Value::Map`.
- [ ] **Rewrite `example_memory.0`**: Same — replace `Aggregate` with
      `MergeMap` operation, package all concepts/relations/context/proof into
      one `Value::Map` output.
- [ ] **Validate**: Parse both files through `parse_graph_from_source()` and
      execute with `GraphInterpreter` to confirm they run.
- [ ] **Document**: Add a `compatibility.md` mapping 0-memory concepts to
      0-openclaw `NodeType` variants.

### Node mapping table (0-memory → 0-openclaw)

```
0-memory concept         → 0-openclaw NodeType
─────────────────────────────────────────────────
Concept label            → Constant { value: String }
Concept hash             → Operation { op: "Hash" }
Relation record          → Constant { value: Map { subject_hash_ref, predicate, object_hash_ref, confidence } }
Context block            → Constant { value: Map { event_time, source, scope, ... } }
Context hash             → Operation { op: "Hash" }
Proof placeholder        → Constant { value: Map { trace_hash, signer, signature } }
Memory record aggregate  → Operation { op: "MergeMap" } (replaces Aggregate)
Field extraction         → Operation { op: "GetField" }
Timestamp                → Operation { op: "Timestamp" }
State persistence        → Operation { op: "SaveState" } / Operation { op: "LoadState" }
```

---

## 2. Runtime Strategy — Shared Types via Trait

### Decision: Abstract `MemoryRuntime` trait, implement for 0-openclaw first

0-memory should not hard-depend on 0-openclaw internals. Instead:

```rust
/// Trait that any 0-lang runtime must implement for 0-memory to use it.
pub trait MemoryRuntime {
    type Value;
    type Hash;
    type Error;

    fn hash(&self, input: &[u8]) -> Self::Hash;
    fn execute_graph(&self, graph: &[u8], inputs: HashMap<String, Self::Value>)
        -> Result<HashMap<String, Self::Value>, Self::Error>;
    fn load_state(&self, key: &str) -> Result<Option<Self::Value>, Self::Error>;
    fn save_state(&self, key: &str, value: &Self::Value) -> Result<(), Self::Error>;
}
```

For MVP, implement `impl MemoryRuntime for OpenclawRuntime` using the existing
`GraphInterpreter`. When 0-chain's executor matures, add `impl MemoryRuntime for ChainRuntime`.

### Action items

- [ ] Define `MemoryRuntime` trait in `0-memory/src/runtime_trait.rs`
- [ ] Implement `OpenclawAdapter` in `0-memory/src/adapters/openclaw.rs`
- [ ] 0-memory compiler emits `.0` graphs; adapter feeds them to the runtime
- [ ] 0-chain adapter is a stub until 0-chain executor is implemented

---

## 3. Hash Strategy — Two-Layer Design

### Problem with v1

v1 used a single hash strategy: `sha256(normalized_label)` for concepts,
`sha256(subject_hash + predicate + object_hash + context_hash)` for relations.
This conflates two distinct needs:

1. **Semantic dedup** — "did I already know this fact?"
2. **Episode recording** — "when/where did I learn this fact?"

### Two-layer hash design

```
Layer 1: FactHash (semantic identity — context-free)
  FactHash = sha256(subject_label + predicate + object_label)

  Purpose: Dedup facts across episodes.
  "Agent needs LongTermMemory" always produces the same FactHash
  regardless of when or where it was observed.

Layer 2: EpisodeHash (event identity — context-bound)
  EpisodeHash = sha256(FactHash + context_hash)

  Purpose: Record each observation event.
  Same fact observed in two different conversations = two EpisodeHashes
  pointing to the same FactHash.
```

### Benefits

- **Semantic dedup on FactHash**: If two conversations both produce
  "Agent needs LongTermMemory," only one fact exists in semantic memory.
- **Episode tracking on EpisodeHash**: Each observation is recorded with
  its context, enabling temporal reasoning and decay per-episode.
- **Confidence aggregation**: Multiple EpisodeHashes referencing the same
  FactHash → confidence increases (Bayesian update).
- **Clean consolidation**: Episodic → Semantic promotion operates on
  FactHash groups, not individual episodes.

### Concept hash (unchanged)

```
ConceptHash = sha256(normalized_label)
```

### Action items

- [ ] Define `FactHash`, `EpisodeHash`, `ConceptHash` as distinct newtypes
- [ ] Update schema to reflect two-layer hashing
- [ ] Compiler emits both layers
- [ ] Store indexes on both FactHash (for dedup) and EpisodeHash (for recall)

---

## 4. Milestones — Recut for Deliverability

### MVP — Compile, Store, Recall by Hash

**Scope**: Tuple in → `.0` graph out → in-memory store → retrieve by hash.
**Duration**: 2 weeks.
**Exit criteria**: A Rust binary that takes `SemanticTuple[]` + `ContextMeta`,
emits a valid `.0` graph, stores it, and retrieves concept/relation nodes by hash.
The emitted `.0` graph executes on 0-openclaw's `GraphInterpreter` without error.

```
0-memory/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── types.rs                  # ConceptHash, FactHash, EpisodeHash, SemanticTuple, ContextMeta
│   ├── compiler/
│   │   ├── mod.rs
│   │   ├── normalizer.rs         # label normalization + alias table
│   │   ├── hasher.rs             # two-layer hashing
│   │   └── emitter.rs            # IR → .0 graph text (using only Constant + Operation nodes)
│   ├── store/
│   │   ├── mod.rs
│   │   ├── graph.rs              # in-memory concept/relation/context store
│   │   └── index.rs              # hash → node lookup (HashMap-based)
│   ├── runtime_trait.rs          # MemoryRuntime trait
│   └── adapters/
│       └── openclaw.rs           # impl MemoryRuntime for 0-openclaw
├── schema/
│   ├── schema.0                  # rewritten, Aggregate-free
│   └── compatibility.md          # node mapping documentation
├── examples/
│   └── example_memory.0          # rewritten, Aggregate-free
└── tests/
    ├── compile_test.rs           # tuples → .0 round-trip
    ├── store_test.rs             # insert + retrieve + dedup
    ├── hash_test.rs              # FactHash vs EpisodeHash determinism
    └── compat_test.rs            # parse + execute .0 through 0-openclaw interpreter
```

#### MVP task breakdown

| # | Task | Depends on | Est. |
|---|---|---|---|
| M1 | Rewrite `schema.0` and `example_memory.0` (Aggregate-free) | — | 0.5d |
| M2 | Define core types (`types.rs`) | — | 0.5d |
| M3 | Implement `normalizer.rs` | M2 | 1d |
| M4 | Implement `hasher.rs` (two-layer) | M2 | 1d |
| M5 | Implement `emitter.rs` | M2, M3, M4 | 2d |
| M6 | Implement `store/graph.rs` + `store/index.rs` | M2 | 2d |
| M7 | Define `MemoryRuntime` trait | — | 0.5d |
| M8 | Implement `adapters/openclaw.rs` | M7 | 1d |
| M9 | Write tests (compile, store, hash, compat) | M5, M6, M8 | 2d |
| M10 | Integration: full pipeline end-to-end test | M9 | 1d |

**Total: ~11.5 person-days**

### V1 — Ranking + Decay + Memory Hierarchy

**Scope**: Multi-factor recall ranking, temporal decay, working/episodic/semantic
memory tiers with consolidation.
**Duration**: 3 weeks.
**Depends on**: MVP complete.

```
src/
├── memory/
│   ├── mod.rs
│   ├── working.rs                # bounded LRU, session-scoped
│   ├── episodic.rs               # EpisodeHash-indexed, exponential decay
│   ├── semantic.rs               # FactHash-indexed, permanent, merge on dedup
│   └── consolidation.rs          # episodic → semantic promotion
├── recall/
│   ├── mod.rs
│   ├── traversal.rs              # BFS/DFS from seed concept
│   ├── ranking.rs                # multi-factor scoring
│   └── decay.rs                  # exponential decay function + sweep
```

#### V1 task breakdown

| # | Task | Depends on | Est. |
|---|---|---|---|
| V1.1 | Working memory (ring buffer, LRU eviction) | MVP | 2d |
| V1.2 | Episodic memory (timestamp-indexed, per-episode decay) | MVP | 3d |
| V1.3 | Semantic memory (FactHash-indexed, confidence aggregation) | MVP | 2d |
| V1.4 | Consolidation pipeline (episode group → semantic merge) | V1.2, V1.3 | 3d |
| V1.5 | Graph traversal (depth-limited, edge-weighted) | MVP | 2d |
| V1.6 | Multi-factor ranking (`confidence × recency × frequency × hops`) | V1.5 | 2d |
| V1.7 | Decay sweep (batch, configurable interval) | V1.2 | 1d |
| V1.8 | Tests + integration | V1.1–V1.7 | 3d |

**Total: ~18 person-days**

### V2 — Token-Efficient Formats

**Scope**: `.0c` compact format, progressive detail levels, delta encoding.
**Duration**: 2 weeks.
**Depends on**: V1 complete.

```
src/
├── format/
│   ├── mod.rs
│   ├── compact.rs                # .0c compressed text format
│   ├── levels.rs                 # L0 (hash-only) / L1 (+ predicates) / L2 (full)
│   └── delta.rs                  # checkpoint-based incremental diffs
```

#### V2 task breakdown

| # | Task | Depends on | Est. |
|---|---|---|---|
| V2.1 | Design `.0c` grammar (hash-ref based, no label repetition) | V1 | 1d |
| V2.2 | Implement `compact::serialize` / `compact::deserialize` | V2.1 | 3d |
| V2.3 | Implement progressive levels L0/L1/L2 | V2.2 | 2d |
| V2.4 | Implement delta encoding (diff + apply) | V1 | 3d |
| V2.5 | Benchmark harness (see Section 5) | V2.2, V2.3, V2.4 | 2d |
| V2.6 | Tests + round-trip fidelity validation | V2.2–V2.5 | 2d |

**Total: ~13 person-days**

### V3 — Ecosystem Integration

**Scope**: 0-chain persistence, 0-openclaw agent hooks, multi-agent memory.
**Duration**: 4 weeks.
**Depends on**: V2 complete + 0-chain executor implemented.

```
src/
├── adapters/
│   ├── openclaw.rs               # extend with memory.recall() / memory.store()
│   └── chain.rs                  # persist to 0-chain state
├── transfer/
│   └── protocol.rs               # agent-to-agent knowledge transfer
├── recall/
│   ├── attention.rs              # spreading activation
│   └── query.rs                  # query DSL
```

#### V3 task breakdown

| # | Task | Depends on | Est. |
|---|---|---|---|
| V3.1 | 0-openclaw memory builtins (`MemoryStore`, `MemoryRecall`) | V2 | 3d |
| V3.2 | 0-chain adapter (when executor ready) | V2 + 0-chain executor | 5d |
| V3.3 | Query DSL (grammar + parser + executor) | V1.5, V1.6 | 4d |
| V3.4 | Spreading activation | V1.5 | 3d |
| V3.5 | Multi-agent shared memory spaces | V2 | 3d |
| V3.6 | Knowledge transfer protocol | V2.2, V2.4 | 3d |
| V3.7 | Integration tests + docs | V3.1–V3.6 | 3d |

**Total: ~24 person-days**

---

## 5. Benchmark Harness — Defined Before Targets

### Principle

No target numbers without a reproducible measurement method.

### Harness definition

```rust
// benches/token_bench.rs

struct BenchConfig {
    /// Number of concepts in the test graph
    concept_count: usize,        // default: 100, 1_000, 10_000
    /// Number of relations per concept (avg)
    relation_density: f64,       // default: 3.0
    /// Tokenizer
    tokenizer: &str,             // "cl100k_base" (GPT-4/4o family)
    /// Tokenizer version
    tokenizer_version: &str,     // tiktoken 0.7.x
}

struct BenchResult {
    format: String,              // "verbose_0", "compact_0c", "L0", "L1", "L2", "delta"
    total_tokens: usize,
    tokens_per_concept: f64,
    tokens_per_relation: f64,
    serialize_time_us: u64,
    deserialize_time_us: u64,
    round_trip_fidelity: bool,   // deserialized == original
}
```

### Hardware baseline

```
CPU: Apple M-series or x86_64 (document which)
RAM: ≥ 8 GB
Rust: stable (document version)
Measurement: criterion.rs, 100 iterations, report median
```

### Data scales

| Scale | Concepts | Relations | Use case |
|---|---|---|---|
| Small | 100 | 300 | Single conversation |
| Medium | 1,000 | 3,000 | Day of agent activity |
| Large | 10,000 | 30,000 | Long-running agent |

### Targets (conditional on harness results)

Targets are **hypotheses** until the benchmark runs. After MVP, run the harness
on verbose `.0` format to establish baselines, then set concrete targets.

| Metric | Hypothesis | Validated after |
|---|---|---|
| `.0c` token reduction vs `.0` | ≥50% | V2.5 |
| L0 token reduction vs L2 | ≥80% | V2.5 |
| Recall latency (10K concepts) | <100ms | V1.8 |
| Dedup rate (hash-identical) | 100% | MVP M9 |
| Round-trip fidelity | 100% | V2.6 |

**Process**: After each milestone, run the harness. If a target is missed,
add a follow-up task to the next milestone. Targets ratchet — they only get
tighter, never relaxed.

---

## 6. Dependency Graph

```
                   ┌──────────────────────────┐
                   │  Compatibility Fix (§1)   │
                   │  Rewrite .0 files         │
                   │  ~0.5 day                 │
                   └────────────┬─────────────┘
                                │
                   ┌────────────▼─────────────┐
                   │  MVP (§4)                 │
                   │  Compile → Store → Recall │
                   │  ~11.5 days               │
                   └──────┬───────────┬────────┘
                          │           │
              ┌───────────▼──┐   ┌────▼──────────┐
              │  V1 (§4)     │   │  V2 (§4)      │
              │  Ranking +   │   │  .0c + Delta   │
              │  Decay +     │   │  ~13 days      │
              │  Hierarchy   │   │                │
              │  ~18 days    │   │                │
              └───────┬──────┘   └────┬───────────┘
                      │               │
                      └───────┬───────┘
                              │
                   ┌──────────▼───────────────┐
                   │  V3 (§4)                  │
                   │  Integration + Transfer   │
                   │  ~24 days                 │
                   │  (requires 0-chain exec)  │
                   └───────────────────────────┘
```

V1 and V2 can run in parallel after MVP. V3 waits for both.

---

## 7. Open Questions (Reduced)

| # | Question | Decision needed by | Proposed default |
|---|---|---|---|
| 1 | NL → tuples: who does the parsing? | MVP start | Upstream LLM produces `SemanticTuple[]`; 0-memory takes structured input only. |
| 2 | Embedding integration for `embedding_ref`? | V1 | Opaque reference for now; no vector engine in 0-memory. |
| 3 | Concurrency model for shared memory? | V3 | Event-sourced append-only log; CRDT merge explored in V3. |

---

## Appendix A: Current `.0` File Incompatibilities (Detailed)

### `schema.0` line 88-98 — `Aggregate` node

```
{
    id: "output",
    type: Aggregate,         ← NOT in 0-openclaw NodeType enum
    inputs: [...],
    outputs: ["schema"]
}
```

**Fix**: Replace with:

```
{
    id: "output",
    type: Operation,
    op: "MergeMap",
    inputs: [
        "schema_version",
        "concept_schema",
        "relation_schema",
        "context_schema",
        "memory_record_schema"
    ]
}
```

Requires each input node to produce a `Value::Map`. Current `Constant` nodes
with `value: { ... }` already produce `Value::Map`, so this works.

### `example_memory.0` line 111-131 — same `Aggregate` pattern

Same fix. Additionally, concept label constants (`value: "Agent"`) produce
`Value::String`, not `Value::Map`. To merge them into a single output map,
wrap each in a `SetField` operation or restructure the output to be a
`CreateMap` with field assignments.

Detailed rewrite will be done in task M1.

### Parser quirk

`parse_graph_from_source()` does a regex-based `key: value` → `"key": value`
conversion. Unquoted enum-like values (`type: Constant`, `op: Hash`) will
fail JSON deserialization unless also quoted. The rewritten `.0` files must
ensure all values are JSON-compatible strings.

---

## Appendix B: 0-openclaw Builtins Available for 0-memory

These existing builtins can be used directly in 0-memory `.0` graphs:

| Builtin | Use in 0-memory |
|---|---|
| `Hash` | Content-addressing (ConceptHash, FactHash, EpisodeHash) |
| `CreateMap` | Assemble memory record maps |
| `MergeMap` | Aggregate multiple maps into one (replaces Aggregate) |
| `GetField` | Extract fields during recall |
| `SetField` | Set fields when assembling records |
| `Concat` | Build composite hash inputs (subject + predicate + object) |
| `Timestamp` | Generate `created_at` / `updated_at` |
| `LoadState` | Load persisted memory state |
| `SaveState` | Persist memory state |
| `Sign` | Proof-carrying signatures |
| `Verify` | Verify proof signatures |
| `GreaterThan` | Confidence threshold filtering |
| `If` | Conditional logic in recall |
| `Multiply` | Decay computation (`confidence × decay_factor`) |
