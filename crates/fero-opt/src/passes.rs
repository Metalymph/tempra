use fero_ir::IrModule;

#[derive(Debug, Clone)]
pub struct OptimizationPlan {
    pub mode: &'static str,
}

impl OptimizationPlan {
    pub fn for_mode(mode: &'static str) -> Self {
        Self { mode }
    }

    pub fn apply(&self, module: IrModule) -> IrModule {
        let _ = self.mode;
        module
    }
}
