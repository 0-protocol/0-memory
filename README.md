# 0-memory

0-memory is a native memory format for agents built on top of `0-lang`.
Instead of storing memory only as text chunks or embeddings, 0-memory stores
memory as executable, content-addressed graph records.

## Goals

- Keep long-term memory lightweight via deduplication and hashing.
- Preserve semantics with explicit concept-relation-context structure.
- Support efficient recall using graph traversal, not full-context replay.
- Keep memory verifiable with proof-carrying execution traces.

## Directory Contents

- `schema.0`: canonical schema graph for 0-memory records.
- `example_memory.0`: compiled memory example from the 0-memory user request.

## Core Data Model

0-memory uses four logical units:

1. `Concept`
   - Entity/idea atom (for example: `0-memory`, `0-lang`, `LongTermMemory`).
   - ID strategy: `sha256(normalized_label)`.
2. `Relation`
   - Typed edge between concepts (`subject -> predicate -> object`).
   - Carries a confidence score.
3. `Context`
   - Event and source metadata (time, scope, channel, language).
4. `MemoryRecord`
   - Aggregates concepts, relations, context, and proof metadata.

## Compilation Pipeline (Natural Language -> 0-memory Graph)

1. Parse utterance into semantic tuples:
   - `(subject, predicate, object, confidence)`
2. Normalize extracted concepts (case, aliases, canonical terms).
3. Hash each concept and context block for content-addressed identity.
4. Emit `0-lang` nodes:
   - concept constants
   - hash operations
   - relation constants referencing concept hashes
   - context constant + context hash
   - proof placeholder + aggregate output
5. Store resulting graph record in persistent chain/state.

## Storage and Recall Strategy

### Write Path

- Compile new utterance into graph form.
- Reuse existing concept hashes when available.
- Append only net-new relations/context links.

### Read Path (Recall)

- Input: query concept hash or canonical label.
- Resolve hash -> concept node.
- Traverse connected relations ranked by:
  - edge confidence
  - temporal recency
  - relation type priority
- Reconstruct concise response memory from highest-scoring paths.

## Why This Is Agent-Native

- Memory is executable graph state, not passive text blobs.
- Hash-addressed structure enables deterministic dedup and replay.
- Confidence-aware relations support probabilistic reasoning.
- Graph form maps directly to `0-lang` runtime primitives.

## Runtime Notes

- `example_memory.0` uses placeholder proof fields:
  - `trace_hash: "pending_runtime_trace_hash"`
  - `signature: "pending_runtime_signature"`
- At runtime, these fields should be filled from actual execution trace and
  signer identity.

## Next Implementation Steps

- Add a 0-memory compiler module to transform parsed tuples into `.0` graphs.
- Add chain/state adapters for persistent memory writes.
- Add a recall executor that performs ranked graph traversal and emits concise
  memory summaries for downstream agents.
