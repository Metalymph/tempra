#[derive(Debug, Clone, Default)]
pub struct MirBody {
    pub blocks: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MirProgram {
    pub functions: Vec<MirBody>,
}
