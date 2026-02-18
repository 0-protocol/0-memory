use std::collections::HashMap;

use sha2::{Digest, Sha256};
use zero_openclaw::error::GatewayError;
use zero_openclaw::runtime::{parse_graph, GraphInterpreter};
use zero_openclaw::Value;

use crate::runtime_trait::MemoryRuntime;

/// Adapter that delegates 0-memory runtime operations to 0-openclaw's
/// `GraphInterpreter`.
///
/// # Sync bridge
///
/// The interpreter's methods are `async` (it uses `tokio::sync::RwLock`
/// internally), but [`MemoryRuntime`] is a sync trait.  The adapter bridges
/// the gap via [`block_on`](Self::block_on):
///
/// * If a tokio runtime is already active (e.g. the caller is an async
///   test or an async application), it uses `block_in_place` + the
///   existing runtime handle — avoiding the "cannot start a runtime
///   from within a runtime" panic.  **The runtime must be multi-threaded**
///   (`tokio::runtime::Builder::new_multi_thread`); calling from a
///   current-thread runtime will panic.
/// * Otherwise it spins up a lightweight current-thread runtime per call.
///
/// For fully-async callers, consider writing an `AsyncMemoryRuntime` trait
/// or calling the interpreter directly.
///
/// # Thread safety
///
/// `GraphInterpreter` holds its state store behind
/// `Arc<tokio::sync::RwLock<…>>`, so `OpenclawAdapter` is `Send + Sync`
/// and safe to share across threads.  The internal state store is
/// session-scoped (keyed by a string ID), so concurrent calls with
/// different keys are non-contending.
pub struct OpenclawAdapter {
    interpreter: GraphInterpreter,
}

impl OpenclawAdapter {
    pub fn new() -> Self {
        Self {
            interpreter: GraphInterpreter::default(),
        }
    }
}

impl Default for OpenclawAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenclawAdapter {
    /// Drive a future to completion, handling both sync and async contexts.
    ///
    /// If called from within an active tokio runtime, uses `block_in_place`
    /// (requires the `rt-multi-thread` tokio feature).  Otherwise creates a
    /// lightweight current-thread runtime for the call.
    fn block_on<F: std::future::Future>(&self, future: F) -> F::Output {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
            Err(_) => {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("failed to create tokio runtime for OpenclawAdapter");
                rt.block_on(future)
            }
        }
    }
}

impl MemoryRuntime for OpenclawAdapter {
    type Value = Value;
    type Hash = [u8; 32];
    type Error = GatewayError;

    fn hash(&self, input: &[u8]) -> [u8; 32] {
        let result = Sha256::digest(input);
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    fn execute_graph(
        &self,
        graph_source: &str,
        inputs: HashMap<String, Self::Value>,
    ) -> Result<HashMap<String, Self::Value>, Self::Error> {
        let graph = parse_graph(graph_source)?;
        let result = self.block_on(self.interpreter.execute(&graph, inputs))?;
        Ok(result.outputs)
    }

    /// Load persisted state by key.
    ///
    /// The underlying `GraphInterpreter::load_state` is infallible and
    /// returns `Value::Null` for missing keys, which we map to `None`.
    fn load_state(&self, key: &str) -> Result<Option<Self::Value>, Self::Error> {
        let value = self.block_on(self.interpreter.load_state(key));
        match value {
            Value::Null => Ok(None),
            other => Ok(Some(other)),
        }
    }

    /// Persist state under the given key.
    ///
    /// The underlying `GraphInterpreter::save_state` takes ownership of the
    /// value and is infallible, so we clone from the `&Value` reference.
    fn save_state(&self, key: &str, value: &Self::Value) -> Result<(), Self::Error> {
        self.block_on(self.interpreter.save_state(key, value.clone()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_determinism() {
        let adapter = OpenclawAdapter::new();
        let h1 = adapter.hash(b"hello");
        let h2 = adapter.hash(b"hello");
        assert_eq!(h1, h2, "same input must produce same hash");
    }

    #[test]
    fn hash_differs_for_different_input() {
        let adapter = OpenclawAdapter::new();
        let h1 = adapter.hash(b"hello");
        let h2 = adapter.hash(b"world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn state_round_trip() {
        let adapter = OpenclawAdapter::new();
        let loaded = adapter.load_state("nonexistent").unwrap();
        assert_eq!(loaded, None, "missing key should return None");

        adapter
            .save_state("test_key", &Value::String("stored".into()))
            .unwrap();
        let loaded = adapter.load_state("test_key").unwrap();
        assert_eq!(loaded, Some(Value::String("stored".into())));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn adapter_works_inside_tokio_runtime() {
        let adapter = OpenclawAdapter::new();
        adapter
            .save_state("async_key", &Value::String("from_async".into()))
            .unwrap();
        let loaded = adapter.load_state("async_key").unwrap();
        assert_eq!(loaded, Some(Value::String("from_async".into())));
    }
}
