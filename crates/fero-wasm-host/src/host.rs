#[derive(Debug, Clone)]
pub struct WasmHostPlan {
    pub engine: &'static str,
}

impl Default for WasmHostPlan {
    fn default() -> Self {
        Self { engine: "wasmtime" }
    }
}
