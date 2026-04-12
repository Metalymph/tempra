use fero_ir::IrModule;

#[derive(Debug, Clone)]
pub struct WasmArtifact {
    pub bytes: Vec<u8>,
}

pub fn emit_wasm(_module: &IrModule) -> WasmArtifact {
    WasmArtifact {
        bytes: b"\0asm".to_vec(),
    }
}
