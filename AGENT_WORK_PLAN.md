# 0-memory — Agent Work Plan

> Assign each agent a role. Tell them: "You are Agent X. Read `AGENT_WORK_PLAN.md`."
> They'll know exactly what files they own, what to build, and when to start.

---

## Roles

| Role | Owns (exclusive file access) | Summary |
|---|---|---|
| **Agent Alpha** | `src/types.rs`, `src/lib.rs`, `Cargo.toml`, `schema/`, `examples/`, `compatibility.md` | Bootstraps the project. Writes shared types. Rewrites `.0` files. |
| **Agent Bravo** | `src/compiler/*` | Builds the compiler: normalizer, hasher, emitter. |
| **Agent Charlie** | `src/store/*`, `tests/*` | Builds the store, writes all tests, runs integration. |
| **Agent Delta** | `src/runtime_trait.rs`, `src/adapters/*` | Defines the runtime trait, implements the 0-openclaw adapter. |

---

## Rules — Every Agent Must Follow These

1. **Never edit another agent's files.** If you need a type or function from
   another agent's module, import it — don't create your own copy.

2. **`src/types.rs` is the shared contract.** All public types live here.
   Only Agent Alpha writes this file. If you need a new type added, say
   "BLOCKED: need X type in types.rs" and stop.

3. **Each `mod.rs` is your public API.** Export only what others need to call.
   Keep internals private.

4. **Compile before you commit.** Run `cargo check` after every meaningful change.
   Don't push code that doesn't compile.

5. **Reference the execution plan.** Detailed design decisions (hash strategy,
   node mapping, runtime trait signature) are in `EXECUTION_PLAN.md`. Read it
   before you start coding.

---

## Phase: Compatibility Fix

**Who**: Agent Alpha (solo)
**When**: First — before any Rust code.
**Duration**: Half day.

### Agent Alpha — Compat Fix Tasks

```
TASK: Rewrite schema.0
FILE: schema/schema.0
DO:
  - Replace the `Aggregate` output node with `Operation { op: "MergeMap" }`
  - Ensure every constant node with a nested object uses value: { ... }
    so it deserializes to Value::Map
  - All values must be JSON-compatible (quote strings, no bare identifiers
    except node type keywords that the parser handles)

TASK: Rewrite example_memory.0
FILE: examples/example_memory.0
DO:
  - Replace `Aggregate` with `MergeMap`
  - Concept label constants (value: "Agent") produce Value::String, not Map.
    Wrap them using SetField into a map, or restructure the output node to use
    CreateMap with field params so the final output is a single Value::Map
  - Keep all existing concepts, relations, context, proof

TASK: Write compatibility.md
FILE: schema/compatibility.md
DO:
  - Document the node mapping table (0-memory concept → 0-openclaw NodeType)
  - Copy from EXECUTION_PLAN.md §1 node mapping table
  - Add any gotchas discovered during rewrite

DONE WHEN: Both .0 files parse without error using 0-openclaw's
           parse_graph_from_source() and execute on GraphInterpreter.
```

---

## Phase: MVP

**When**: Starts after compat fix is committed.
**Duration**: ~6 days wall-clock (3 agents in parallel).

### Timeline

```
DAY  1         2         3         4         5         6
     |---------|---------|---------|---------|---------|

Alpha: [M2: types.rs + Cargo.toml + lib.rs              ]
       [        0.5d        ] done → unblocks Bravo & Charlie

Bravo:  wait   [M3: normalizer ][M4: hasher  ][M5: emitter               ]
                    1d              1d              2d

Charlie: wait  [M6: store/graph.rs + store/index.rs      ]
                         2d

Delta: [M7: runtime_trait ][M8: adapters/openclaw.rs     ]
            0.5d                    1d

                              ── converge ──

Charlie:                                      [M9: tests ][M10: integ]
                                                  2d          1d
```

### Agent Alpha — MVP Tasks

```
TASK M2: Project scaffold + shared types
FILES:
  - Cargo.toml         (new)
  - src/lib.rs          (new)
  - src/types.rs        (new)

DO:
  1. Create Cargo.toml:
     - package name: "zero-memory"
     - edition 2021
     - dependencies: sha2, serde, serde_json
     - dev-dependencies: tokio (for adapter tests)

  2. Create src/lib.rs:
     - pub mod types;
     - pub mod compiler;
     - pub mod store;
     - pub mod runtime_trait;
     - pub mod adapters;

  3. Create src/types.rs with these exact types:

     /// Content-addressed concept identity.
     pub struct ConceptHash(pub [u8; 32]);

     /// Context-free semantic identity of a fact.
     /// FactHash = sha256(normalized_subject + predicate + normalized_object)
     pub struct FactHash(pub [u8; 32]);

     /// Context-bound event identity.
     /// EpisodeHash = sha256(FactHash + context_hash)
     pub struct EpisodeHash(pub [u8; 32]);

     /// Hash of a context block.
     pub struct ContextHash(pub [u8; 32]);

     /// Input tuple from upstream (LLM or structured source).
     pub struct SemanticTuple {
         pub subject: String,
         pub predicate: String,
         pub object: String,
         pub confidence: f64,
     }

     /// Metadata about the observation context.
     pub struct ContextMeta {
         pub event_time: String,    // ISO 8601
         pub source: String,        // e.g. "user_prompt", "observation"
         pub scope: String,         // e.g. "conversation_123"
         pub agent_id: Option<String>,
         pub session_id: Option<String>,
         pub metadata: Option<HashMap<String, String>>,
     }

     /// Full input to the compiler.
     pub struct CompilerInput {
         pub utterance: Option<String>,
         pub tuples: Vec<SemanticTuple>,
         pub context: ContextMeta,
     }

     /// A stored concept node.
     pub struct ConceptNode {
         pub hash: ConceptHash,
         pub label: String,
         pub aliases: Vec<String>,
         pub confidence: f64,
         pub created_at: String,
         pub updated_at: String,
     }

     /// A stored relation node.
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
     pub struct ContextNode {
         pub hash: ContextHash,
         pub meta: ContextMeta,
     }

     /// A complete memory record.
     pub struct MemoryRecord {
         pub concepts: Vec<ConceptNode>,
         pub relations: Vec<RelationNode>,
         pub context: ContextNode,
     }

  Add derive macros: Debug, Clone, PartialEq, Eq, Hash (for hash newtypes).
  Add hex display for hash types.
  Add serde Serialize/Deserialize where appropriate.

DONE WHEN: `cargo check` passes. Other agents can import from types.rs.
SIGNAL: Commit with message "M2: core types and project scaffold"
```

After M2, Agent Alpha is free. Can assist other agents or start on V1 design.

---

### Agent Bravo — MVP Tasks

```
PREREQUISITE: Wait for Agent Alpha's M2 commit. You need types.rs to exist.

TASK M3: Normalizer
FILE: src/compiler/normalizer.rs

DO:
  - pub fn normalize_label(raw: &str) -> String
    - lowercase
    - trim whitespace
    - collapse multiple spaces to single space
    - replace common separators (_, .) with -

  - pub struct AliasTable (HashMap<String, String>)
    - maps variant labels to canonical label
    - e.g. "long_term_memory" → "long-term-memory"
    - e.g. "LTM" → "long-term-memory"
    - pub fn resolve(&self, label: &str) -> String
      returns canonical label if alias exists, else returns normalized input

  - pub fn normalize_predicate(raw: &str) -> String
    - lowercase, trim, collapse spaces
    - predicate-specific normalization (e.g. "is a" → "is_a")

TASK M4: Hasher (two-layer)
FILE: src/compiler/hasher.rs

DO:
  - Use sha2::Sha256 directly (same as 0-openclaw's Hash builtin)

  - pub fn concept_hash(normalized_label: &str) -> ConceptHash
    sha256(normalized_label.as_bytes())

  - pub fn fact_hash(subject_label: &str, predicate: &str, object_label: &str) -> FactHash
    sha256(subject_label + "|" + predicate + "|" + object_label)
    Use "|" separator to avoid ambiguity.

  - pub fn context_hash(meta: &ContextMeta) -> ContextHash
    sha256(meta.event_time + "|" + meta.source + "|" + meta.scope)

  - pub fn episode_hash(fact: &FactHash, ctx: &ContextHash) -> EpisodeHash
    sha256(fact.0 + ctx.0)    // concatenate raw bytes

  All functions are pure and deterministic.

TASK M5: Emitter
FILE: src/compiler/emitter.rs

DO:
  - pub fn compile(input: &CompilerInput) -> CompilerOutput
    where CompilerOutput contains:
      - graph_text: String    // the .0 format text
      - record: MemoryRecord  // the structured in-memory record

  Pipeline:
    1. For each tuple in input.tuples:
       a. Normalize subject and object labels (use normalizer)
       b. Compute ConceptHash for subject and object
       c. Compute FactHash
       d. Compute ContextHash from input.context
       e. Compute EpisodeHash from FactHash + ContextHash
       f. Build ConceptNode, RelationNode

    2. Dedup concepts: if two tuples share a concept label, emit once

    3. Build ContextNode from input.context

    4. Emit .0 graph text:
       - Graph { name, version, description, nodes: [...], outputs, entry_point, metadata }
       - Concept labels → Constant { value: "label" } nodes
       - Concept hashes → Operation { op: "Hash", inputs: ["concept_label_node"] } nodes
       - Context → Constant { value: { ... } } node
       - Context hash → Operation { op: "Hash", inputs: ["context_node"] } node
       - Relations → Constant { value: { subject_hash_ref, predicate, ... } } nodes
       - Proof → Constant { value: { trace_hash: "pending", ... } } node
       - Output → Operation { op: "MergeMap", inputs: [...all above...] } node
       ** DO NOT use Aggregate. Use MergeMap only. **

    5. Wrap concept string constants in SetField nodes so MergeMap receives
       Value::Map from every input (MergeMap ignores non-Map inputs, which
       would silently drop data).

  - pub fn emit_graph_text(record: &MemoryRecord, context: &ContextMeta) -> String
    Serializes a MemoryRecord to .0 format string.

ALSO CREATE:
  - src/compiler/mod.rs
    pub mod normalizer;
    pub mod hasher;
    pub mod emitter;
    pub use emitter::{compile, CompilerOutput};

DONE WHEN: compile() produces a .0 graph string that matches the structure
           of examples/example_memory.0 and cargo check passes.
```

---

### Agent Charlie — MVP Tasks

```
PREREQUISITE: Wait for Agent Alpha's M2 commit. You need types.rs to exist.

TASK M6: In-memory graph store
FILES:
  - src/store/mod.rs
  - src/store/graph.rs
  - src/store/index.rs

DO:
  src/store/graph.rs:

  pub struct MemoryStore {
      concepts: HashMap<ConceptHash, ConceptNode>,
      relations_by_fact: HashMap<FactHash, Vec<RelationNode>>,
      relations_by_episode: HashMap<EpisodeHash, RelationNode>,
      contexts: HashMap<ContextHash, ContextNode>,
      adjacency: HashMap<ConceptHash, Vec<FactHash>>,  // concept → related facts
  }

  impl MemoryStore {
      pub fn new() -> Self
      pub fn insert_record(&mut self, record: MemoryRecord) -> InsertResult
        - For each concept: insert if hash not present (dedup)
        - For each relation: insert by EpisodeHash (always new),
          dedup check on FactHash (same fact = update confidence)
        - Insert context
        - Update adjacency index
        - Return InsertResult { new_concepts, new_facts, new_episodes, dupes_skipped }

      pub fn get_concept(&self, hash: &ConceptHash) -> Option<&ConceptNode>
      pub fn get_concept_by_label(&self, label: &str) -> Option<&ConceptNode>
        - requires reverse index (label → hash)

      pub fn get_relations(&self, concept_hash: &ConceptHash) -> Vec<&RelationNode>
        - via adjacency index

      pub fn get_relations_by_fact(&self, fact_hash: &FactHash) -> Vec<&RelationNode>

      pub fn get_context(&self, hash: &ContextHash) -> Option<&ContextNode>

      pub fn concept_count(&self) -> usize
      pub fn relation_count(&self) -> usize
  }

  src/store/index.rs:

  pub struct LabelIndex {
      label_to_hash: HashMap<String, ConceptHash>,
  }

  impl LabelIndex {
      pub fn insert(&mut self, label: &str, hash: ConceptHash)
      pub fn lookup(&self, label: &str) -> Option<&ConceptHash>
  }

  src/store/mod.rs:
    pub mod graph;
    pub mod index;
    pub use graph::MemoryStore;

TASK M9: Write all tests
FILES:
  - tests/compile_test.rs
  - tests/store_test.rs
  - tests/hash_test.rs
  - tests/compat_test.rs

DO:
  tests/hash_test.rs:
    - Same label → same ConceptHash (determinism)
    - Different labels → different ConceptHash
    - Same (s,p,o) → same FactHash regardless of context
    - Same FactHash + different context → different EpisodeHash
    - Hash is stable across runs (hardcode a known sha256 value and assert)

  tests/compile_test.rs:
    - Create a CompilerInput with 3 tuples
    - Call compile()
    - Assert output has correct number of concepts (deduped)
    - Assert output has correct number of relations
    - Assert graph_text contains "MergeMap" (not "Aggregate")
    - Assert graph_text contains all expected concept labels

  tests/store_test.rs:
    - Insert a MemoryRecord → get_concept returns it
    - Insert same concept twice → concept_count stays 1 (dedup)
    - Insert same fact from two contexts → relations_by_fact returns 2 episodes
    - get_relations(concept_hash) returns correct neighbors
    - get_concept_by_label works after insert

  tests/compat_test.rs:
    - Parse examples/example_memory.0 using 0-openclaw's parse_graph_from_source()
    - Execute through GraphInterpreter
    - Assert execution succeeds (no error)
    - Assert output contains expected keys
    NOTE: This test depends on 0-openclaw as a dev-dependency. Add to Cargo.toml:
      [dev-dependencies]
      zero-openclaw = { path = "../0-openclaw" }
    If 0-openclaw doesn't expose parse_graph_from_source publicly, skip this
    test and add TODO comment.

TASK M10: Integration test
FILE: tests/integration_test.rs

DO:
  Full pipeline test:
    1. Build CompilerInput from raw tuples
    2. compile() → CompilerOutput
    3. store.insert_record(output.record)
    4. store.get_concept_by_label("0-memory") → assert exists
    5. store.get_relations(concept_hash) → assert returns expected relations
    6. Assert round-trip: original tuples' semantics preserved in store

DONE WHEN: `cargo test` passes all tests.
```

---

### Agent Delta — MVP Tasks

```
NO PREREQUISITES. Start immediately.

TASK M7: MemoryRuntime trait
FILE: src/runtime_trait.rs

DO:
  use std::collections::HashMap;

  pub trait MemoryRuntime {
      type Value: Clone + std::fmt::Debug;
      type Hash: AsRef<[u8]> + Clone;
      type Error: std::fmt::Display;

      /// Compute SHA-256 hash.
      fn hash(&self, input: &[u8]) -> Self::Hash;

      /// Execute a 0-lang graph with given inputs.
      fn execute_graph(
          &self,
          graph_source: &str,
          inputs: HashMap<String, Self::Value>,
      ) -> Result<HashMap<String, Self::Value>, Self::Error>;

      /// Load persisted state by key.
      fn load_state(&self, key: &str) -> Result<Option<Self::Value>, Self::Error>;

      /// Save state by key.
      fn save_state(&self, key: &str, value: &Self::Value) -> Result<(), Self::Error>;
  }

TASK M8: OpenclawAdapter
FILE: src/adapters/openclaw.rs

DO:
  Implement MemoryRuntime for a struct that wraps 0-openclaw's GraphInterpreter.

  pub struct OpenclawAdapter {
      interpreter: GraphInterpreter,  // from 0-openclaw::runtime::interpreter
  }

  impl OpenclawAdapter {
      pub fn new() -> Self {
          Self {
              interpreter: GraphInterpreter::default(),
          }
      }
  }

  impl MemoryRuntime for OpenclawAdapter {
      type Value = Value;         // 0-openclaw's Value enum
      type Hash = [u8; 32];
      type Error = GatewayError;  // 0-openclaw's error type

      fn hash(&self, input: &[u8]) -> [u8; 32] {
          use sha2::{Sha256, Digest};
          let result = Sha256::digest(input);
          let mut hash = [0u8; 32];
          hash.copy_from_slice(&result);
          hash
      }

      fn execute_graph(...) {
          let graph = parse_graph_from_source(graph_source)?;
          // GraphInterpreter::execute is async — use block_on or make trait async
          // Decision: make this sync for now, use tokio::runtime::Runtime::block_on
          let rt = tokio::runtime::Runtime::new().unwrap();
          rt.block_on(self.interpreter.execute(&graph, inputs))
            .map(|result| result.outputs)
      }

      fn load_state(...) { ... }
      fn save_state(...) { ... }
  }

  NOTE: 0-openclaw must be added as a dependency:
    [dependencies]
    zero-openclaw = { path = "../0-openclaw" }

  If 0-openclaw's types are not publicly exported, create a feature flag
  `openclaw` and gate this adapter behind it:
    #[cfg(feature = "openclaw")]
    pub mod openclaw;

ALSO CREATE:
  - src/adapters/mod.rs
    #[cfg(feature = "openclaw")]
    pub mod openclaw;

DONE WHEN: `cargo check --features openclaw` passes.
           Or if 0-openclaw isn't importable, document the blocker and
           provide a mock implementation that passes cargo check.
```

---

## Phase: V1 — Ranking + Decay + Memory Hierarchy

**When**: After MVP (all M1–M10 done, `cargo test` green).
**Duration**: ~8 days wall-clock (4 agents in parallel).

### Reassignment

| Role | Owns | Tasks |
|---|---|---|
| **Agent Alpha** | `src/memory/working.rs` | V1.1 |
| **Agent Bravo** | `src/memory/episodic.rs`, `src/recall/decay.rs` | V1.2 → V1.7 |
| **Agent Charlie** | `src/memory/semantic.rs`, `src/memory/consolidation.rs` | V1.3 → V1.4 |
| **Agent Delta** | `src/recall/traversal.rs`, `src/recall/ranking.rs` | V1.5 → V1.6 |

### Timeline

```
DAY  1         2         3         4         5         6         7         8
     |---------|---------|---------|---------|---------|---------|---------|

Alpha: [V1.1: working.rs   ] done → free, can help with tests
            2d

Bravo: [V1.2: episodic.rs              ][V1.7: decay.rs]
              3d                             1d

Charlie: [V1.3: semantic.rs  ][  wait   ][V1.4: consolidation.rs       ]
              2d               for V1.2         3d

Delta: [V1.5: traversal.rs  ][V1.6: ranking.rs  ]
            2d                     2d

                                               ── converge ──

Alpha (or any free):                                      [V1.8: tests     ]
                                                               3d
```

### Agent Alpha — V1 Tasks

```
TASK V1.1: Working memory
FILE: src/memory/working.rs

DO:
  pub struct WorkingMemory {
      buffer: VecDeque<MemoryRecord>,
      max_capacity: usize,          // default: 20
      session_id: Option<String>,
  }

  impl WorkingMemory {
      pub fn new(max_capacity: usize) -> Self
      pub fn push(&mut self, record: MemoryRecord)
        - Push to back
        - If len > max_capacity, pop_front (LRU eviction)
      pub fn peek(&self, n: usize) -> &[MemoryRecord]
        - Return last n records (most recent)
      pub fn clear(&mut self)
      pub fn len(&self) -> usize
      pub fn set_session(&mut self, session_id: String)
      pub fn is_session_match(&self, session_id: &str) -> bool
  }

ALSO CREATE:
  - src/memory/mod.rs
    pub mod working;
    pub mod episodic;    // stubs for now
    pub mod semantic;
    pub mod consolidation;
```

### Agent Bravo — V1 Tasks

```
TASK V1.2: Episodic memory
FILE: src/memory/episodic.rs

DO:
  pub struct EpisodicMemory {
      episodes: HashMap<EpisodeHash, Episode>,
      fact_index: HashMap<FactHash, Vec<EpisodeHash>>,  // fact → its episodes
      config: EpisodicConfig,
  }

  pub struct Episode {
      pub relation: RelationNode,
      pub base_confidence: f64,
      pub created_at: chrono::DateTime<Utc>,   // add chrono dependency
      pub last_accessed: chrono::DateTime<Utc>,
      pub access_count: u64,
  }

  pub struct EpisodicConfig {
      pub decay_half_life_secs: f64,    // default: 7 days = 604800
      pub min_confidence: f64,           // below this → candidate for pruning (default: 0.01)
  }

  impl EpisodicMemory {
      pub fn new(config: EpisodicConfig) -> Self
      pub fn store(&mut self, relation: RelationNode)
        - Create Episode with base_confidence = relation.confidence
        - Index by EpisodeHash and FactHash

      pub fn recall(&self, fact_hash: &FactHash) -> Vec<&Episode>
        - Return all episodes for a fact, sorted by effective_confidence desc

      pub fn effective_confidence(&self, episode: &Episode, now: DateTime<Utc>) -> f64
        - base_confidence * e^(-lambda * elapsed_secs)
        - lambda = ln(2) / half_life_secs

      pub fn sweep(&mut self, now: DateTime<Utc>) -> Vec<Episode>
        - Remove all episodes with effective_confidence < min_confidence
        - Return removed episodes (for consolidation to inspect)

      pub fn touch(&mut self, hash: &EpisodeHash)
        - Update last_accessed and increment access_count
  }

TASK V1.7: Decay function
FILE: src/recall/decay.rs

DO:
  pub fn exponential_decay(base: f64, half_life_secs: f64, elapsed_secs: f64) -> f64 {
      base * (-elapsed_secs * (2.0_f64.ln()) / half_life_secs).exp()
  }

  pub fn importance_score(
      confidence: f64,
      recency_secs: f64,
      access_count: u64,
      max_access_count: u64,
      connection_degree: usize,
      max_degree: usize,
      weights: &ImportanceWeights,
  ) -> f64

  pub struct ImportanceWeights {
      pub confidence: f64,      // default: 0.35
      pub recency: f64,         // default: 0.25
      pub frequency: f64,       // default: 0.20
      pub connectivity: f64,    // default: 0.20
  }
```

### Agent Charlie — V1 Tasks

```
TASK V1.3: Semantic memory
FILE: src/memory/semantic.rs

DO:
  pub struct SemanticMemory {
      facts: HashMap<FactHash, SemanticFact>,
      concept_facts: HashMap<ConceptHash, Vec<FactHash>>,  // concept → related facts
  }

  pub struct SemanticFact {
      pub fact_hash: FactHash,
      pub subject_hash: ConceptHash,
      pub predicate: String,
      pub object_hash: ConceptHash,
      pub confidence: f64,          // aggregated across episodes
      pub observation_count: u64,   // how many episodes support this
      pub first_seen: DateTime<Utc>,
      pub last_reinforced: DateTime<Utc>,
  }

  impl SemanticMemory {
      pub fn new() -> Self
      pub fn store_or_merge(&mut self, fact_hash: FactHash, relation: &RelationNode)
        - If fact_hash exists: Bayesian confidence update, increment observation_count
        - If new: insert with observation_count = 1
        - Bayesian update: new_conf = 1 - (1 - old_conf) * (1 - incoming_conf)

      pub fn get_fact(&self, hash: &FactHash) -> Option<&SemanticFact>
      pub fn get_facts_for_concept(&self, concept: &ConceptHash) -> Vec<&SemanticFact>
      pub fn fact_count(&self) -> usize
  }

TASK V1.4: Consolidation pipeline
FILE: src/memory/consolidation.rs
PREREQUISITE: Wait for Agent Bravo's V1.2 (need EpisodicMemory API)

DO:
  pub struct ConsolidationConfig {
      pub min_episode_count: usize,     // fact must appear in ≥ N episodes (default: 3)
      pub min_surviving_confidence: f64, // after decay, still above this (default: 0.1)
  }

  pub fn consolidate(
      episodic: &mut EpisodicMemory,
      semantic: &mut SemanticMemory,
      store: &MemoryStore,          // to look up concept nodes
      config: &ConsolidationConfig,
      now: DateTime<Utc>,
  ) -> ConsolidationResult

  pub struct ConsolidationResult {
      pub promoted_facts: usize,
      pub pruned_episodes: usize,
  }

  Pipeline:
    1. Group episodes by FactHash
    2. For each FactHash group:
       a. Count episodes with effective_confidence > min_surviving_confidence
       b. If count >= min_episode_count → promote to semantic
       c. Call semantic.store_or_merge(fact_hash, best_episode.relation)
       d. Mark promoted episodes for pruning (or leave for natural decay)
    3. Return stats
```

### Agent Delta — V1 Tasks

```
TASK V1.5: Graph traversal
FILE: src/recall/traversal.rs

DO:
  pub struct TraversalConfig {
      pub max_depth: usize,          // default: 3
      pub max_results: usize,        // default: 50
      pub min_confidence: f64,       // skip edges below this (default: 0.1)
  }

  pub fn bfs_from_concept(
      store: &MemoryStore,
      seed: &ConceptHash,
      config: &TraversalConfig,
  ) -> Vec<TraversalHit>

  pub struct TraversalHit {
      pub concept: ConceptHash,
      pub path: Vec<FactHash>,       // path from seed to this concept
      pub depth: usize,
      pub path_confidence: f64,      // product of edge confidences along path
  }

  Algorithm:
    1. Start from seed concept
    2. BFS using store.get_relations(concept_hash) for neighbors
    3. For each edge, multiply path_confidence by edge confidence
    4. Skip edges with confidence < min_confidence
    5. Stop at max_depth
    6. Collect up to max_results hits
    7. Sort by path_confidence descending

TASK V1.6: Multi-factor ranking
FILE: src/recall/ranking.rs

DO:
  pub fn rank_results(
      hits: &[TraversalHit],
      store: &MemoryStore,
      episodic: &EpisodicMemory,
      weights: &ImportanceWeights,   // from decay.rs
      now: DateTime<Utc>,
  ) -> Vec<RankedResult>

  pub struct RankedResult {
      pub concept_hash: ConceptHash,
      pub score: f64,
      pub depth: usize,
      pub contributing_facts: Vec<FactHash>,
  }

  Scoring per hit:
    For each fact in hit.path:
      - confidence: from store relation
      - recency: decay::exponential_decay(1.0, 7d, elapsed) for newest episode
      - frequency: episode access_count / max across all
      - connectivity: store.get_relations(concept).len() / max across all
    Combine using ImportanceWeights

ALSO CREATE:
  - src/recall/mod.rs
    pub mod traversal;
    pub mod ranking;
    pub mod decay;
```

---

## Phase: V2 — Token-Efficient Formats

**When**: Can start once MVP store API is stable. Runs in parallel with V1.
**Duration**: ~7 days wall-clock (2 agents).

### Reassignment

| Role | Owns | Tasks |
|---|---|---|
| **Agent Alpha** | `src/format/compact.rs`, `src/format/levels.rs` | V2.1 → V2.2 → V2.3 |
| **Agent Bravo** | `src/format/delta.rs`, `benches/` | V2.4 → V2.5 |

Charlie and Delta continue on V1 during this time.

### Agent Alpha — V2 Tasks

```
TASK V2.1 + V2.2: Compact format (.0c)
FILES: src/format/compact.rs

Grammar:
  # Header
  0M:1.0                               # format version

  # Concepts  (C:short_hash:label)
  C:a1b2c3d4:agent
  C:e5f6a7b8:long-term-memory
  C:c9d0e1f2:0-memory

  # Relations  (R:subject_short>predicate>object_short:confidence)
  R:a1b2c3d4>needs>e5f6a7b8:0.98
  R:c9d0e1f2>solves>e5f6a7b8:0.97

  # Context  (X:hash:key=value,key=value)
  X:f3a4b5c6:t=2026-02-18T00:00:00Z,src=user_prompt,scope=design

  Rules:
  - short_hash = first 8 hex chars of full hash (enough to be unique within a record)
  - One line per entity
  - No quotes, no braces, no JSON overhead
  - Agent parses this faster and with fewer tokens than .0 or JSON

DO:
  pub fn serialize(record: &MemoryRecord, context: &ContextMeta) -> String
  pub fn deserialize(compact: &str) -> Result<MemoryRecord, ParseError>
  Round-trip: deserialize(serialize(r)) == r

TASK V2.3: Progressive detail levels
FILE: src/format/levels.rs

DO:
  pub enum DetailLevel { L0, L1, L2 }

  pub fn serialize_at_level(record: &MemoryRecord, level: DetailLevel) -> String

  L0 (skeleton):
    C:a1b2c3d4
    C:e5f6a7b8
    (~3 tokens per concept, no labels, no relations)

  L1 (structure):
    C:a1b2c3d4:agent
    R:a1b2c3d4>needs>e5f6a7b8
    (labels and predicates, no confidence, no context)

  L2 (full):
    Same as compact format — full detail.
```

### Agent Bravo — V2 Tasks

```
TASK V2.4: Delta encoding
FILE: src/format/delta.rs

DO:
  pub struct Checkpoint {
      pub hash: [u8; 32],           // sha256 of full store state
      pub concept_count: usize,
      pub relation_count: usize,
  }

  pub struct Delta {
      pub base_checkpoint: [u8; 32],
      pub new_concepts: Vec<ConceptNode>,
      pub new_relations: Vec<RelationNode>,
      pub updated_confidences: Vec<(FactHash, f64)>,
      pub removed: Vec<FactHash>,
  }

  pub fn checkpoint(store: &MemoryStore) -> Checkpoint
    - Hash all concept hashes + relation hashes sorted deterministically

  pub fn diff(old: &Checkpoint, old_store: &MemoryStore, new_store: &MemoryStore) -> Delta
    - new_concepts: in new but not in old (by ConceptHash)
    - new_relations: in new but not in old (by EpisodeHash)
    - updated_confidences: same FactHash, different confidence
    - removed: in old but not in new

  pub fn apply(store: &mut MemoryStore, delta: &Delta) -> Result<(), ApplyError>
    - Validate base_checkpoint matches store's current checkpoint
    - Insert new concepts, new relations
    - Update confidences
    - Remove marked facts

  pub fn serialize_delta(delta: &Delta) -> String   // compact text format
  pub fn deserialize_delta(text: &str) -> Result<Delta, ParseError>

TASK V2.5: Benchmark harness
FILES: benches/token_bench.rs, benches/README.md

DO:
  - Add to Cargo.toml:
    [dev-dependencies]
    criterion = "0.5"
    tiktoken-rs = "0.5"

    [[bench]]
    name = "token_bench"
    harness = false

  - Generate synthetic data:
    fn generate_test_store(concept_count: usize, relation_density: f64) -> MemoryStore

  - Measure for each format (verbose .0, compact .0c, L0, L1, L2, delta):
    fn count_tokens(text: &str) -> usize   // using tiktoken cl100k_base
    fn measure_format(store: &MemoryStore, format: &str) -> BenchResult

  - Run at 3 scales: 100, 1000, 10000 concepts
  - Output results as markdown table
  - benches/README.md: document hardware, rust version, tiktoken version

ALSO CREATE:
  - src/format/mod.rs
    pub mod compact;
    pub mod levels;
    pub mod delta;
```

---

## Phase: V3 — Ecosystem Integration

**When**: After V1 and V2.
**Duration**: ~12 days wall-clock (3 agents).

### Reassignment

| Role | Owns | Tasks |
|---|---|---|
| **Agent Alpha** | `src/recall/query.rs`, `src/recall/attention.rs` | V3.3 → V3.4 |
| **Agent Bravo** | `src/adapters/chain.rs`, `src/adapters/openclaw.rs` (extend) | V3.1 → V3.2 |
| **Agent Charlie** | `src/transfer/`, `src/memory/shared.rs` | V3.5 → V3.6 |
| **Agent Delta** | `tests/`, documentation | V3.7 |

Detailed task specs for V3 will be written when V1/V2 are complete, since
the APIs they depend on will be finalized by then.

---

## Quick Reference Card

Print this for each agent:

```
┌─────────────────────────────────────────────────────────┐
│  0-memory Agent Quick Reference                         │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  1. Read EXECUTION_PLAN.md first (design decisions)     │
│  2. Read this file for your specific tasks              │
│  3. Find your role in the Roles table                   │
│  4. Only edit YOUR files                                │
│  5. Import shared types from src/types.rs               │
│  6. Run `cargo check` before every commit               │
│  7. If blocked, say "BLOCKED: reason" and stop          │
│  8. Commit message format: "M3: implement normalizer"   │
│                                                         │
│  Shared contract: src/types.rs (read-only for you)      │
│  Your API surface: your_module/mod.rs                   │
│  Runtime ref: 0-openclaw/src/runtime/types.rs           │
│                                                         │
└─────────────────────────────────────────────────────────┘
```
