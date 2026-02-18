# 0-memory MVP — Code Review & Finalisation Guide

> Reviewed against `EXECUTION_PLAN.md` and `AGENT_WORK_PLAN.md`.
> Build status: `cargo check` clean, `cargo test` 51/51 green, zero warnings.

---

## Overall Verdict

The four agents delivered a solid MVP. The code compiles, tests pass, the `.0`
graph files execute on the 0-openclaw runtime, and the core pipeline
(compile → store → recall-by-hash) works end to end. The architecture cleanly
separates concerns across modules with a well-defined shared type contract.

What follows is a per-agent review, then a cross-cutting issues section, and
finally a concrete task list for the **fifth agent** to stitch and finalise.

---

## Agent Alpha — Types, Scaffold, Schema Files

**Files owned**: `Cargo.toml`, `src/lib.rs`, `src/types.rs`, `schema/`, `examples/`, `compatibility.md`, `README.md`

### Strengths

1. **Clean type design.** Four distinct hash newtypes (`ConceptHash`, `FactHash`,
   `EpisodeHash`, `ContextHash`) prevent accidental mix-ups at compile time.
   The `impl_hash_type!` macro is DRY and correct.

2. **Serde hygiene.** `skip_serializing_if = "Option::is_none"` on optional
   fields in `ContextMeta` — good practice for wire-format cleanliness.

3. **Schema files execute.** Both `schema/schema.0` and `examples/example_memory.0`
   parse and execute through 0-openclaw's `GraphInterpreter` (verified by
   `compat_test.rs`). The `Aggregate` → `MergeMap` migration is done correctly.

4. **`compatibility.md` is thorough.** Documents Gotcha #5 (parser colons bug)
   which is critical knowledge for anyone touching the emitter.

### Issues

| # | Severity | File | Issue |
|---|----------|------|-------|
| A1 | **Medium** | `types.rs` | `ConceptNode.confidence` semantics are ambiguous. When the same concept appears in two tuples with different confidences, only the first-seen confidence is stored (see emitter line 37). Should this be `max`, `mean`, or Bayesian-updated? The V1 `SemanticMemory` does Bayesian updates on *facts*, but concept-level confidence is silently dropped-if-duplicate. |
| A2 | **Low** | `types.rs` | Hash newtypes derive `Serialize`/`Deserialize` but serialise as raw `[u8; 32]` arrays (32 integers in JSON). For human-readable JSON, consider a `#[serde(with = "hex_serde")]` wrapper or custom serde impl to emit hex strings. This will matter when records are persisted to disk or sent over APIs. |
| A3 | **Low** | `types.rs` | `CompilerOutput` does not derive `Serialize`/`Deserialize` (unlike every other public type). If downstream code needs to cache/persist compiler output, this will require manual work. |
| A4 | **Low** | `Cargo.toml` | No `rust-version` field. Since edition 2021 is used, pinning `rust-version = "1.56"` (or higher) documents the MSRV. |
| A5 | **Info** | `schema/` & root | `schema.0` and `schema/schema.0` are identical files. Same for `example_memory.0` and `examples/example_memory.0`. The execution plan specifies the `schema/` and `examples/` paths. The root-level duplicates should be removed (or symlinked) to avoid drift. |
| A6 | **Medium** | `README.md` | Line 45 says "proof placeholder + aggregate output" — should say "MergeMap output". Stale reference to the removed Aggregate concept. |

---

## Agent Bravo — Compiler (Normalizer, Hasher, Emitter)

**Files owned**: `src/compiler/*`

### Strengths

1. **Normalizer is idiomatic.** Handles all specified transformations (lowercase,
   trim, collapse spaces, separator replacement). `AliasTable::with_defaults()`
   pre-populates common domain aliases.

2. **Hasher is pure and deterministic.** Pipe-separator strategy for `fact_hash`
   is sound and prevents the ambiguity noted in the execution plan. The
   `short_hex` helper is a good forward investment for `.0c` format (V2).

3. **Emitter produces valid .0 output.** The `CreateMap` wrapping for concept
   nodes is correct — every MergeMap input is a `Value::Map`.

4. **Good test coverage.** Emitter has 6 unit tests covering dedup, relation count,
   MergeMap usage, concept presence, proof node, and context.

### Issues

| # | Severity | File | Issue |
|---|----------|------|-------|
| B1 | **Critical** | `emitter.rs:104` | **Emitted string values are not JSON-escaped.** `c.label` is interpolated directly into the graph text with `format!("value: \"{}\"")`. If a concept label contains a double-quote, backslash, or newline, the emitted `.0` file will be malformed JSON. Use `serde_json::to_string(&c.label)` which handles escaping. Same issue on lines 117, 127, 149-156 for context fields and relation fields. |
| B2 | **Critical** | `emitter.rs:127` | **Context fields containing colons will break the 0-openclaw parser.** `compatibility.md` Gotcha #5 documents this: `parse_graph_from_source()` corrupts `word:` patterns inside string values. The emitter uses `context.event_time` directly, which is typically ISO 8601 with colons (`T00:00:00Z`). The hand-authored `example_memory.0` works around this with `20260218T000000Z`, but the compiler emitter does NOT apply this workaround. Any compiled graph with a standard ISO timestamp will fail to parse through 0-openclaw. |
| B3 | **Medium** | `emitter.rs:117` | `CreateMap` with `params: { "keys": ["label", "hash"] }` — this assumes the 0-openclaw `CreateMap` builtin uses a `keys` param to zip inputs into a map. Need to verify this is the actual `CreateMap` API. Looking at the 0-openclaw builtins, `CreateMap` creates a map **from params directly** (not from inputs + keys). The emitter's concept wrapping may silently produce an unexpected result. The hand-authored `.0` files use `SetField` for wrapping, which is the verified pattern. |
| B4 | **Medium** | `emitter.rs` | **`AliasTable` is never used in the compile pipeline.** `compile()` calls `normalize_label()` but never creates or uses an `AliasTable`. The execution plan specifies that normalization should resolve aliases. This means "LTM" and "long_term_memory" will produce different concept hashes instead of being deduplicated. |
| B5 | **Medium** | `normalizer.rs` | `normalize_predicate` converts spaces to underscores, but does not handle existing underscores or dots. `normalize_label` converts `_` to `-`, but `normalize_predicate` does not convert `-` to `_`. This asymmetry could cause confusion: `"has_part"` as a label becomes `"has-part"`, but `"has-part"` as a predicate stays `"has-part"` (not `"has_part"`). |
| B6 | **Low** | `emitter.rs:191` | `entry_point` is hardcoded to `"concept_label_0"`. If the input has zero tuples (and thus zero concepts), this references a nonexistent node. Should either handle the empty case or use a guaranteed node like `"context"`. |
| B7 | **Low** | `emitter.rs:184-196` | The graph text format uses an ad-hoc unquoted `Graph { ... }` wrapper with mixed quoting (`name:` vs `"name":`). The compat tests pass because the parser handles both, but it's inconsistent with the hand-authored `.0` files which use fully-quoted JSON keys. |

---

## Agent Charlie — Store, Tests

**Files owned**: `src/store/*`, `tests/*`

### Strengths

1. **Store API is clean and complete.** All methods specified in the work plan
   are implemented. Dedup on `ConceptHash` and `EpisodeHash` works correctly.
   The `adjacency` index enables efficient neighbor lookups.

2. **Excellent test coverage.** 8 store tests, 8 hash tests, 8 compile tests,
   5 integration tests, 3 compat tests = 32 external tests. All test the
   specified scenarios from the work plan (dedup, two-episode-same-fact,
   label lookup, etc.).

3. **Integration test covers the full pipeline.** `full_pipeline_compile_store_retrieve`
   exercises compile → store → retrieve → assert round-trip semantics.

4. **Compat tests validate runtime execution.** Both `.0` files are parsed and
   executed through 0-openclaw with output assertions.

### Issues

| # | Severity | File | Issue |
|---|----------|------|-------|
| C1 | **Medium** | `graph.rs:41-48` | **Concept dedup discards updates.** When a concept with the same hash is inserted a second time, the entire new `ConceptNode` is dropped. This means updated `confidence`, `updated_at`, or new `aliases` are lost. The execution plan's V1 has Bayesian updates on facts but concept updates are silently ignored. At minimum, `updated_at` should be refreshed and confidence should be max'd or accumulated. |
| C2 | **Medium** | `graph.rs:62-69` | **Adjacency index grows without bounds.** Every new episode appends to the adjacency `Vec<FactHash>` even if the same `FactHash` is already present. After N episodes of the same fact, the adjacency list has N duplicate entries, inflating `get_relations()` results. The `seen` HashSet in `get_relations()` (line 107) masks this at query time but the index itself leaks memory. |
| C3 | **Low** | `graph.rs:98-113` | `get_relations()` deduplicates by `episode_hash` via HashSet, but the caller likely wants *all* episodes (not one per episode). The dedup is correct if the intent is "unique relations referencing this concept" but the doc comment says "all relations". This should be clarified. |
| C4 | **Low** | `store_test.rs` | Tests construct `MemoryRecord` manually (via `make_record`) rather than going through `compile()`. This is correct for unit-testing the store, but means the store tests don't verify that the compiler's output format is compatible with the store's expectations. The integration tests cover this gap, so it's acceptable. |
| C5 | **Low** | `index.rs` | `LabelIndex` does not normalise labels before lookup. If `MemoryStore::get_concept_by_label("Agent")` is called (capital A), it won't find the concept stored under `"agent"` (lowercase). The store relies on callers to pre-normalise. This should be documented or enforced. |
| C6 | **Info** | `compat_test.rs:70-85` | `root_level_files_are_aggregate_free` checks the root-level `.0` files — but these are duplicates of `schema/schema.0` and `examples/example_memory.0`. If the root files are removed (per A5), this test will break. |

---

## Agent Delta — Runtime Trait, OpenclawAdapter

**Files owned**: `src/runtime_trait.rs`, `src/adapters/*`

### Strengths

1. **Trait design matches the execution plan exactly.** Associated types for
   `Value`, `Hash`, `Error` with the correct trait bounds.

2. **Feature-gated adapter.** `#[cfg(feature = "openclaw")]` avoids hard-coupling
   the main crate to 0-openclaw. The default feature set is empty.

3. **Sync bridge is pragmatic.** `block_on()` with a per-call tokio runtime is
   the right choice for MVP. The doc comment correctly flags the async variant
   as future work.

4. **State round-trip test.** Tests `save_state` → `load_state` → verify, and
   the missing-key-returns-None case.

### Issues

| # | Severity | File | Issue |
|---|----------|------|-------|
| D1 | **High** | `openclaw.rs:37-40` | **Creating a new `tokio::runtime::Runtime` per call is expensive and will panic if called from within an existing tokio runtime** (e.g., from an async test or an async application). `tokio::runtime::Runtime::new()` inside an active runtime triggers "Cannot start a runtime from within a runtime". The adapter tests pass because they're `#[test]` not `#[tokio::test]`, but any async caller will hit this. Consider using `tokio::runtime::Handle::try_current()` to detect an existing runtime and use `block_in_place` + `handle.block_on` in that case, or `futures::executor::block_on` for a lighter sync bridge. |
| D2 | **Medium** | `openclaw.rs:66-69` | `load_state` calls `self.interpreter.load_state(key)` but the return type assumes the interpreter returns a raw `Value`. If the interpreter method itself returns `Result<Value, ...>`, this needs error propagation. Need to verify the actual `GraphInterpreter::load_state` signature. If it's infallible (returns `Value` directly), this is fine but unusual. |
| D3 | **Medium** | `openclaw.rs:73-76` | `save_state` calls `self.interpreter.save_state(key, value.clone())`. The `.clone()` is because the trait takes `&Self::Value` but the interpreter likely takes owned `Value`. This is correct but should be documented as a known cost. More importantly, `save_state` always returns `Ok(())` — if the interpreter's save is fallible, errors are silently swallowed. |
| D4 | **Medium** | `openclaw.rs:61` | `self.interpreter.execute(&graph, inputs)` passes the interpreter by `&self`, but `GraphInterpreter::execute` takes `&self` — meaning the interpreter's internal state (e.g., `state_store: Arc<RwLock<...>>`) is shared across calls. This is fine for single-threaded use but should be documented. If `MemoryRuntime` is used from multiple threads, the `OpenclawAdapter` needs `Send + Sync` bounds. Currently `GraphInterpreter` contains `Arc<RwLock<...>>` which is `Send + Sync`, so this should work, but it's untested. |
| D5 | **Low** | `runtime_trait.rs` | The `MemoryRuntime` trait is defined but **never used** by any other module in the MVP. The compiler doesn't call `execute_graph()`, the store doesn't call `load_state()`/`save_state()`. The trait exists as a contract for V1+ but is orphaned in the current codebase. This is acceptable for MVP scaffolding but should be noted. |
| D6 | **Low** | `adapters/mod.rs:1` | Comment says "Owned by Agent Delta" — agent ownership comments should be removed for the final codebase. |

---

## Cross-Cutting Issues

These span multiple agents' code and need coordinated fixes.

| # | Severity | Description | Agents Affected |
|---|----------|-------------|-----------------|
| X1 | **Critical** | **Emitter ↔ Parser colon bug.** The compiler emitter produces ISO 8601 timestamps with colons in the `.0` graph text. The 0-openclaw parser's regex corrupts `word:` patterns inside string values. The hand-authored `.0` files work around this, but compiler-emitted graphs will fail to parse. This means the full pipeline (compile → `.0` text → parse through 0-openclaw) is broken for any non-trivial context. The compat tests pass because they test the *hand-authored* files, not compiler-emitted output. | Bravo (emitter), Delta (adapter) |
| X2 | **High** | **`CreateMap` API mismatch.** The emitter uses `CreateMap` with `inputs` + `params.keys` to zip values into a map. The 0-openclaw `CreateMap` builtin creates a map directly from `params` (ignoring inputs). This means concept wrapping nodes in compiler-emitted graphs may produce empty maps or maps with unexpected structure. This is not caught by any test because no test parses compiler-emitted `.0` text through 0-openclaw — only the hand-authored files are tested. | Bravo (emitter), Charlie (compat_test gap) |
| X3 | **Medium** | **No test for compiler output → 0-openclaw execution.** The compat tests parse hand-authored `.0` files. The compile tests check text content with string assertions. No test compiles a `CompilerInput`, feeds the resulting `graph_text` to `parse_graph()`, and executes it. This is the most important round-trip and it's untested. | Charlie (tests) |
| X4 | **Medium** | **Duplicate `.0` files at root and subdirectories.** `schema.0` exists at both `/` and `/schema/`. `example_memory.0` exists at both `/` and `/examples/`. Divergence risk. | Alpha (schema files) |
| X5 | **Low** | **No `cargo clippy` or `cargo fmt` enforcement.** The code is well-formatted but there's no CI or pre-commit hook. Consider adding a `.cargo/config.toml` or a simple script. | All |

---

## Fifth Agent — Stitch & Finalisation Task List

The fifth agent should fix the critical and high issues above, close the
integration gaps, and prepare the codebase for V1 development.

### Priority 1: Critical Fixes

#### Task F1: Fix emitter string escaping (B1)

In `src/compiler/emitter.rs`, all string interpolations into the `.0` graph
text must be JSON-escaped. Replace raw `format!("\"{}\"", value)` patterns
with `serde_json::to_string(&value).unwrap()` (which adds quotes and escapes).

Affected lines: 104, 117, 127, 148-156.

#### Task F2: Fix emitter timestamp colon issue (B2 / X1)

The emitter must sanitise `ContextMeta.event_time` (and any other string
fields) to remove colons before embedding in `.0` graph text. Options:

- **Option A**: Strip colons from ISO timestamps: `T00:00:00Z` → `T000000Z`
- **Option B**: Use a different timestamp format (epoch millis)
- **Option C**: Base64-encode all string values in the graph text

Recommend Option A for consistency with the hand-authored examples.
Apply to `event_time`, `source`, and `scope` fields at minimum.

#### Task F3: Fix `CreateMap` usage in emitter (B3 / X2)

Replace the `CreateMap` with `inputs + params.keys` pattern with the verified
`SetField` pattern used in the hand-authored `.0` files:

```rust
// Instead of:
//   CreateMap with inputs: [label, hash], params: { keys: ["label", "hash"] }
// Use:
//   CreateMap with params: {} (empty map)
//   SetField inputs: [empty_map, label], params: { field: "label" }
//   SetField inputs: [map_with_label, hash], params: { field: "hash" }
```

Or emit a single `CreateMap` with both values in `params` if they're
string constants.

### Priority 2: High Fixes

#### Task F4: Fix `block_on` tokio nesting panic (D1)

In `src/adapters/openclaw.rs`, replace the naive `Runtime::new()` approach:

```rust
fn block_on<F: std::future::Future>(&self, future: F) -> F::Output {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
        Err(_) => {
            let rt = tokio::runtime::Runtime::new()
                .expect("failed to create tokio runtime");
            rt.block_on(future)
        }
    }
}
```

Update `Cargo.toml` to include `"rt-multi-thread"` in the tokio features
for the openclaw feature (required for `block_in_place`).

### Priority 3: Integration Gap

#### Task F5: Add compiler-output-through-runtime test (X3)

This is the most important missing test. Add to `tests/compat_test.rs`:

```rust
#[tokio::test]
async fn compiler_output_parses_and_executes() {
    let input = CompilerInput { /* standard test input */ };
    let output = compile(&input);
    let graph = parse_graph(&output.graph_text)
        .expect("Compiler-emitted .0 must parse");
    let interp = GraphInterpreter::default();
    let result = interp.execute(&graph, HashMap::new()).await
        .expect("Compiler-emitted graph must execute");
    assert!(result.outputs.contains_key("output"));
}
```

This test will likely fail until F1, F2, and F3 are fixed. Write it first
as a red test, then make it green.

### Priority 4: Medium Fixes

#### Task F6: Wire up `AliasTable` in compile pipeline (B4)

In `emitter.rs::compile()`, accept an optional `&AliasTable` parameter
(or add it to `CompilerInput`). Use `table.resolve(label)` instead of
bare `normalize_label()`. Default to `AliasTable::with_defaults()`.

#### Task F7: Update concept on re-insert (C1)

In `store/graph.rs::insert_record()`, when a concept hash already exists,
update `updated_at` to the new timestamp, take the max confidence, and
merge any new aliases. Change from:

```rust
if self.concepts.contains_key(&concept.hash) {
    result.dupes_skipped += 1;
} else { ... }
```

To:

```rust
if let Some(existing) = self.concepts.get_mut(&concept.hash) {
    existing.updated_at = concept.updated_at;
    if concept.confidence > existing.confidence {
        existing.confidence = concept.confidence;
    }
    for alias in concept.aliases {
        if !existing.aliases.contains(&alias) {
            existing.aliases.push(alias);
        }
    }
    result.dupes_skipped += 1;
} else { ... }
```

#### Task F8: Deduplicate adjacency index (C2)

In `store/graph.rs`, before pushing to the adjacency list, check if the
`FactHash` is already present:

```rust
let adj = self.adjacency.entry(relation.subject_hash.clone()).or_default();
if !adj.contains(&relation.fact_hash) {
    adj.push(relation.fact_hash.clone());
}
```

Or use a `HashSet<FactHash>` instead of `Vec<FactHash>` for the adjacency index.

### Priority 5: Cleanup

#### Task F9: Remove duplicate root `.0` files (A5 / X4)

Delete `schema.0` and `example_memory.0` at the project root. They are
copies of `schema/schema.0` and `examples/example_memory.0`. Update any
test that references root-level files (`compat_test.rs:70-85`) to point
at the canonical paths, or remove the root-file test entirely.

#### Task F10: Fix README stale reference (A6)

In `README.md` line 45, change "aggregate output" to "MergeMap output".

#### Task F11: Remove agent ownership comments (D6)

Remove the comment `// Owned by Agent Delta` from `src/adapters/mod.rs`
and any similar agent-reference comments elsewhere.

#### Task F12: Normalise `LabelIndex` lookups (C5)

Either: (a) have `LabelIndex::lookup()` normalise the input label before
lookup, or (b) document that callers must pre-normalise. Option (a) is
safer:

```rust
pub fn lookup(&self, label: &str) -> Option<&ConceptHash> {
    let normalised = crate::compiler::normalizer::normalize_label(label);
    self.label_to_hash.get(&normalised)
}
```

This introduces a dependency from `store` → `compiler::normalizer`. If
that coupling is undesirable, move `normalize_label` to `types.rs` or
a `utils` module.

---

## Execution Order for Fifth Agent

```
F5 (write red test)
  → F1 (fix escaping)
  → F2 (fix colons)
  → F3 (fix CreateMap)
  → F5 becomes green ✓
  → F4 (fix block_on)
  → F7 (concept update)
  → F8 (adjacency dedup)
  → F6 (alias table)
  → F12 (label index)
  → F9 (remove dupes)
  → F10 (readme fix)
  → F11 (comment cleanup)
  → cargo test --all-features
  → cargo clippy
  → cargo fmt --check
```

Estimated effort: **1–2 days** for a single agent.

---

## Summary Scorecard

| Agent | Scope Complete? | Code Quality | Test Coverage | Critical Issues |
|-------|-----------------|--------------|---------------|-----------------|
| **Alpha** | ✅ All tasks done | Good | N/A (types) | 0 |
| **Bravo** | ✅ All tasks done | Good, 3 bugs | Good (6 unit) | 2 (escaping, CreateMap) |
| **Charlie** | ✅ All tasks done | Good | Excellent (32 tests) | 0 (but missing round-trip test) |
| **Delta** | ✅ All tasks done | Good, 1 bug | Adequate (3 unit) | 0 (1 high: tokio panic) |

Overall: **solid MVP foundation** with 3 critical bugs in the emitter that
prevent compiler-emitted graphs from executing through 0-openclaw. The fifth
agent's primary job is fixing these 3 bugs and adding the round-trip test that
proves the full pipeline works end to end.
