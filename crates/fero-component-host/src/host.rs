#[derive(Debug, Clone)]
pub struct ComponentHostPlan {
    pub engine: &'static str,
}

impl Default for ComponentHostPlan {
    fn default() -> Self {
        Self {
            engine: "wasmtime-component",
        }
    }
}
