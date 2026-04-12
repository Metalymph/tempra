use fero_ir::IrModule;
use fero_linker::LinkPlan;

#[derive(Debug, Clone)]
pub struct NativeArtifact {
    pub object_name: String,
    pub link: LinkPlan,
}

pub fn emit_native(_module: &IrModule, output: &str) -> NativeArtifact {
    NativeArtifact {
        object_name: format!("{output}.o"),
        link: LinkPlan {
            output: output.to_string(),
        },
    }
}
