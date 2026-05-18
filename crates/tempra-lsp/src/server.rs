//! LSP Backend: thin adapter over `tempra-analysis`.
//!
//! This module contains zero compiler or typechecker logic.
//! All semantic intelligence is delegated to `AnalysisSession`.

use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use tempra_analysis::{AnalysisSession, DocumentUri, LspPosition};

/// Converts a Tempra `Severity` to an LSP `DiagnosticSeverity`.
fn severity_to_lsp(sev: tempra_diagnostics::Severity) -> Option<DiagnosticSeverity> {
    match sev {
        tempra_diagnostics::Severity::Error => Some(DiagnosticSeverity::ERROR),
        tempra_diagnostics::Severity::Warning => Some(DiagnosticSeverity::WARNING),
        tempra_diagnostics::Severity::Info => Some(DiagnosticSeverity::INFORMATION),
        tempra_diagnostics::Severity::Hint => Some(DiagnosticSeverity::HINT),
    }
}

/// Converts a Tempra `SymbolKind` to an LSP `SymbolKind`.
pub fn symbol_kind_to_lsp(kind: tempra_diagnostics::SymbolKind) -> SymbolKind {
    match kind {
        tempra_diagnostics::SymbolKind::Function => SymbolKind::FUNCTION,
        tempra_diagnostics::SymbolKind::Struct => SymbolKind::STRUCT,
        tempra_diagnostics::SymbolKind::Enum => SymbolKind::ENUM,
        tempra_diagnostics::SymbolKind::Method => SymbolKind::METHOD,
        tempra_diagnostics::SymbolKind::Extension => SymbolKind::MODULE,
        tempra_diagnostics::SymbolKind::Variable => SymbolKind::VARIABLE,
    }
}

/// Convert an `LspRange` from `tempra-analysis` to a tower-lsp `Range`.
fn analysis_range_to_lsp(r: tempra_analysis::LspRange) -> Range {
    Range {
        start: Position {
            line: r.start.line,
            character: r.start.character,
        },
        end: Position {
            line: r.end.line,
            character: r.end.character,
        },
    }
}

/// Convert an `LspPosition` (tower-lsp) to a tempra-analysis `LspPosition`.
fn to_analysis_pos(pos: Position) -> LspPosition {
    LspPosition {
        line: pos.line,
        character: pos.character,
    }
}

/// Build a zero-length LSP range at position (0,0) — used as fallback when
/// we have a diagnostic but the span cannot be mapped (e.g. empty source).
fn fallback_range() -> Range {
    Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: 0,
            character: 0,
        },
    }
}

/// Convert a list of Tempra diagnostics into LSP diagnostics for a given URI.
///
/// Requires the session to be already locked (read-only) so that span-to-range
/// conversion can access the document's line index.
fn convert_diagnostics(
    session: &AnalysisSession,
    uri: &DocumentUri,
    tempra_diags: Vec<tempra_diagnostics::Diagnostic>,
) -> Vec<Diagnostic> {
    tempra_diags
        .into_iter()
        .map(|d| {
            // Prefer primary label span; fall back to the main diagnostic span.
            let span = d.labels.first().map(|l| l.span).unwrap_or(d.span);

            let range = session
                .span_to_lsp_range(uri, span)
                .map(analysis_range_to_lsp)
                .unwrap_or_else(fallback_range);

            Diagnostic {
                range,
                severity: severity_to_lsp(d.severity),
                code: d.code.map(|c| NumberOrString::String(c)),
                source: Some("tempra".to_string()),
                message: d.message,
                related_information: None,
                tags: None,
                code_description: None,
                data: None,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Backend
// ---------------------------------------------------------------------------

/// The LSP backend. Holds an `AnalysisSession` behind an async read-write lock
/// and a `Client` handle for sending notifications back to the editor.
pub struct Backend {
    client: Client,
    session: Arc<RwLock<AnalysisSession>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            session: Arc::new(RwLock::new(AnalysisSession::new())),
        }
    }

    /// Run analysis on an open/changed document and push fresh diagnostics.
    async fn refresh_diagnostics(&self, uri: Url, content: String, version: i32) {
        let doc_uri = DocumentUri::new(uri.as_str());

        // Write: update the in-memory session.
        {
            let mut session = self.session.write().await;
            session.open_document(doc_uri.clone(), content, version as u64);
        }

        // Read: collect diagnostics.
        let lsp_diags = {
            let session = self.session.read().await;
            let tempra_diags = session.diagnostics(&doc_uri).unwrap_or_default();
            convert_diagnostics(&session, &doc_uri, tempra_diags)
        };

        self.client
            .publish_diagnostics(uri, lsp_diags, Some(version))
            .await;
    }
}

// ---------------------------------------------------------------------------
// LanguageServer impl
// ---------------------------------------------------------------------------

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _params: InitializeParams) -> LspResult<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    // Full sync only for now (Phase 18.1E scope).
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "tempra-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "tempra-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Text document sync
    // -----------------------------------------------------------------------

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let td = params.text_document;
        self.refresh_diagnostics(td.uri, td.text, td.version).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // Full sync: we always get exactly one content item containing the
        // whole document.
        if let Some(change) = params.content_changes.into_iter().next() {
            self.refresh_diagnostics(
                params.text_document.uri,
                change.text,
                params.text_document.version,
            )
            .await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let doc_uri = DocumentUri::new(uri.as_str());

        {
            let mut session = self.session.write().await;
            session.close_document(&doc_uri);
        }

        // Clear diagnostics for the closed document.
        self.client.publish_diagnostics(uri, vec![], None).await;
    }

    // -----------------------------------------------------------------------
    // Hover
    // -----------------------------------------------------------------------

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        let td = params.text_document_position_params;
        let doc_uri = DocumentUri::new(td.text_document.uri.as_str());
        let pos = to_analysis_pos(td.position);

        let session = self.session.read().await;
        let text = session
            .hover_at_lsp_position(&doc_uri, pos)
            .map(|contents| Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("```\n{}\n```", contents),
                }),
                range: None,
            });

        Ok(text)
    }

    // -----------------------------------------------------------------------
    // Go To Definition
    // -----------------------------------------------------------------------

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let td = params.text_document_position_params;
        let url = td.text_document.uri.clone();
        let doc_uri = DocumentUri::new(url.as_str());
        let pos = to_analysis_pos(td.position);

        let session = self.session.read().await;

        let span = match session.definition_at_lsp_position(&doc_uri, pos) {
            Some(s) => s,
            None => return Ok(None),
        };

        let range = match session.span_to_lsp_range(&doc_uri, span) {
            Some(r) => analysis_range_to_lsp(r),
            None => fallback_range(),
        };

        Ok(Some(GotoDefinitionResponse::Scalar(Location {
            uri: url,
            range,
        })))
    }

    // -----------------------------------------------------------------------
    // Document Symbols
    // -----------------------------------------------------------------------

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> LspResult<Option<DocumentSymbolResponse>> {
        let url = params.text_document.uri.clone();
        let doc_uri = DocumentUri::new(url.as_str());

        let session = self.session.read().await;

        let symbols = match session.document_symbols(&doc_uri) {
            Some(syms) => syms,
            None => return Ok(None),
        };

        let lsp_symbols: Vec<SymbolInformation> = symbols
            .into_iter()
            .map(|sym| {
                let range = session
                    .span_to_lsp_range(&doc_uri, sym.span)
                    .map(analysis_range_to_lsp)
                    .unwrap_or_else(fallback_range);

                #[allow(deprecated)]
                SymbolInformation {
                    name: sym.name,
                    kind: symbol_kind_to_lsp(sym.kind),
                    tags: None,
                    deprecated: None,
                    location: Location {
                        uri: url.clone(),
                        range,
                    },
                    container_name: None,
                }
            })
            .collect();

        Ok(Some(DocumentSymbolResponse::Flat(lsp_symbols)))
    }
}
