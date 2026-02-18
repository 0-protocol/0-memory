use std::collections::HashMap;

/// Trait that any 0-lang runtime must implement for 0-memory to use it.
///
/// For MVP, implemented by `OpenclawAdapter` (behind the `openclaw` feature).
/// When 0-chain's executor matures, a `ChainAdapter` will be added.
pub trait MemoryRuntime {
    type Value: Clone + std::fmt::Debug;
    type Hash: AsRef<[u8]> + Clone;
    type Error: std::fmt::Display;

    /// Compute SHA-256 hash of arbitrary bytes.
    fn hash(&self, input: &[u8]) -> Self::Hash;

    /// Execute a 0-lang graph given as source text, with named inputs.
    fn execute_graph(
        &self,
        graph_source: &str,
        inputs: HashMap<String, Self::Value>,
    ) -> Result<HashMap<String, Self::Value>, Self::Error>;

    /// Load persisted state by key. Returns `None` if the key does not exist.
    fn load_state(&self, key: &str) -> Result<Option<Self::Value>, Self::Error>;

    /// Save state under a key.
    fn save_state(&self, key: &str, value: &Self::Value) -> Result<(), Self::Error>;
}
