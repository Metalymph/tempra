use crate::HirFunction;

#[derive(Debug, Clone, Default)]
pub struct HirProgram {
    pub functions: Vec<HirFunction>,
}
