# 0-memory ↔ 0-openclaw Compatibility

> Documents how 0-memory concepts map to 0-openclaw's `NodeType` variants
> and runtime builtins. All `.0` graph files in this repository use only
> types and operations that exist in the 0-openclaw runtime today.

---

## Node Mapping Table

| 0-memory concept | 0-openclaw `NodeType` | Notes |
|---|---|---|
| Concept label | `Constant { value: "label" }` | Produces `Value::String`. Must be wrapped in `SetField` before feeding to `MergeMap`. |
| Concept hash | `Operation { op: "Hash" }` | SHA-256 of the label string bytes. Produces `Value::Hash([u8; 32])`. |
| Relation record | `Constant { value: { subject_hash_ref, predicate, object_hash_ref, confidence } }` | Produces `Value::Map`. Compatible with `MergeMap` directly. |
| Context block | `Constant { value: { event_time, source, scope, ... } }` | Produces `Value::Map`. |
| Context hash | `Operation { op: "Hash" }` | SHA-256 of the serialized context map. Produces `Value::Hash`. |
| Proof placeholder | `Constant { value: { trace_hash, signer, signature } }` | Produces `Value::Map`. |
| Memory record aggregate | `Operation { op: "MergeMap" }` | **Replaces the removed `Aggregate` node type.** |
| Field extraction | `Operation { op: "GetField" }` | Extract a named field from a `Value::Map`. |
| Field assignment | `Operation { op: "SetField" }` | Wrap a value into a `Value::Map` under a named key. |
| Map construction | `Operation { op: "CreateMap" }` | Create a `Value::Map` from params (no inputs needed). |
| Timestamp | `Operation { op: "Timestamp" }` | Current time as `Value::Int` (millis since epoch). |
| State persistence | `Operation { op: "SaveState" }` / `Operation { op: "LoadState" }` | Session-scoped key-value store. |
| String concatenation | `Operation { op: "Concat" }` | Used to build composite hash inputs. |

---

## Breaking Change: `Aggregate` → `MergeMap`

### What changed

The original `.0` files used an `Aggregate` node type that does not exist in
any 0-lang runtime. It has been replaced with `Operation { op: "MergeMap" }`,
which is a registered builtin in 0-openclaw's `BuiltinRegistry`.

### Semantic difference

`MergeMap` merges multiple `Value::Map` inputs into a single `Value::Map`.
**It silently ignores non-Map inputs** (String, Hash, Int, etc.). This means
every input to MergeMap must be a `Value::Map` or its data will be dropped.

### Migration pattern

For values that are not Maps (concept labels, hash outputs, raw strings):

```
# Before (broken — MergeMap drops the String)
{ "id": "label", "type": "Constant", "value": "Agent" }
{ "id": "output", "type": "Operation", "op": "MergeMap", "inputs": ["label"] }

# After (works — SetField wraps String into a Map)
{ "id": "label", "type": "Constant", "value": "Agent" }
{ "id": "empty", "type": "Operation", "op": "CreateMap", "inputs": [], "params": {} }
{ "id": "wrap_label", "type": "Operation", "op": "SetField",
  "inputs": ["empty", "label"], "params": { "field": "concept_label" } }
{ "id": "output", "type": "Operation", "op": "MergeMap", "inputs": ["wrap_label"] }
```

Alternatively, use `CreateMap` for static values:

```
{ "id": "label_map", "type": "Operation", "op": "CreateMap",
  "inputs": [], "params": { "concept_label": "Agent" } }
{ "id": "output", "type": "Operation", "op": "MergeMap", "inputs": ["label_map"] }
```

---

## Parser Requirements

`parse_graph_from_source()` in 0-openclaw performs a simple regex conversion:

1. Quotes unquoted keys: `word:` → `"word":`
2. Removes trailing commas before `}` or `]`

It does **not** quote unquoted values. Therefore:

- **All string values must be quoted** in `.0` files: `"type": "Constant"`, not `type: Constant`
- Numeric values (`0.98`, `1`) and boolean values (`true`, `false`) work unquoted
- Object/array values (`{ ... }`, `[ ... ]`) work as-is
- Comment lines starting with `#` are stripped before parsing

---

## Two-Layer Hash Design

0-memory uses two distinct hash layers, both computed via the `Hash` builtin:

| Layer | Identity | Formula | Purpose |
|---|---|---|---|
| **FactHash** | Semantic (context-free) | `sha256(subject_label \| predicate \| object_label)` | Dedup facts across episodes |
| **EpisodeHash** | Event (context-bound) | `sha256(FactHash bytes + ContextHash bytes)` | Record each observation event |

`ConceptHash = sha256(normalized_label)` remains unchanged.

The `Hash` builtin in 0-openclaw hashes `Value::String` inputs as raw bytes
and other types via JSON serialization. This matches 0-memory's hashing
strategy when concept labels are passed as `Value::String` constants.

---

## Gotchas Discovered During Rewrite

1. **MergeMap key collisions**: When multiple relation constants share the same
   field names (`subject_hash_ref`, `predicate`, etc.), `MergeMap` uses
   `HashMap::extend` which overwrites earlier values. Each relation must be
   wrapped in a `SetField` under a unique key before merging.

2. **Entry point semantics**: The `entry_point` field is informational for
   topo-sort seeding. Nodes with no inputs are processed first regardless of
   `entry_point` value.

3. **Output references**: The interpreter's output collector looks up node IDs
   directly in the computed values map. Dot notation (`output.field`) in the
   `outputs` array does **not** work — use the node ID directly.

4. **Hash determinism**: `Hash` on `Value::Map` serializes via `serde_json`,
   and `HashMap` iteration order is non-deterministic. For reproducible hashes,
   always hash `Value::String` or `Value::Bytes` inputs, not Map values directly.

5. **No colons in string values**: The `parse_graph_from_source()` regex
   `(\w+)(\s*):` matches **inside** quoted string values — it cannot
   distinguish keys from value content. Any `word:` pattern inside a string
   (e.g., ISO 8601 timestamps `T00:00:00Z`, URIs `agent://...`) will be
   corrupted by the regex inserting extra quotes. **Workaround**: use compact
   ISO format without colons (`20260218T000000Z`) and avoid `://` in values.
   This also affects the compiler emitter: if `ContextMeta.event_time` contains
   colons, the emitted `.0` graph text will not parse through 0-openclaw.
