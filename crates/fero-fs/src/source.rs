use anyhow::Result;
use fero_span::SourceId;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub id: SourceId,
    pub path: String,
    pub text: String,
}

impl SourceFile {
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)?;
        Ok(Self {
            id: SourceId(0),
            path: path.display().to_string(),
            text,
        })
    }
}
