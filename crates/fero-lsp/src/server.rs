#[derive(Debug, Default)]
pub struct LanguageServer;

impl LanguageServer {
    pub fn name(&self) -> &'static str {
        "fero-lsp"
    }
}
