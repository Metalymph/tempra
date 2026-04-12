use fero_package::{BuildMode, BuildTarget};

#[derive(Debug, Clone)]
pub enum DriverCommand {
    Run,
    Check,
    Build(BuildRequest),
    Install(BuildRequest),
    Test,
    Fmt,
}

#[derive(Debug, Clone, Copy)]
pub struct BuildRequest {
    pub target: BuildTarget,
    pub mode: BuildMode,
}

#[derive(Debug, Clone)]
pub struct DriverOutcome {
    pub summary: String,
}
