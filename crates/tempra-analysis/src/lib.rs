//! Editor-facing analysis layer and LSP Core Foundation for Tempra.
//!
//! Provides document snapshot caching, query-driven compilations,
//! and best-effort top-level hover/goto-definition resolvers.

use std::collections::HashMap;
use tempra_driver::pipeline::{diagnose_source, extract_symbols_from_source};

/// A normalized, unique Document URI representation.
#[derive(
    Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct DocumentUri(String);

impl DocumentUri {
    /// Create a new normalized URI.
    pub fn new(uri: &str) -> Self {
        let mut normalized = uri.trim().to_string();
        while normalized.ends_with('/') && normalized.len() > 1 {
            normalized.pop();
        }
        if normalized.starts_with("file://localhost/") {
            normalized = normalized.replace("file://localhost/", "file:///");
        }
        DocumentUri(normalized)
    }

    /// Access the URI string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DocumentUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for DocumentUri {
    fn from(s: String) -> Self {
        DocumentUri::new(&s)
    }
}

impl From<&str> for DocumentUri {
    fn from(s: &str) -> Self {
        DocumentUri::new(s)
    }
}

/// Monotonically increasing version counter for documents.
pub type SourceVersion = u64;

/// A read-only snapshot of a document's content at a specific version.
#[derive(Debug, Clone)]
pub struct DocumentSnapshot {
    pub uri: DocumentUri,
    pub content: String,
    pub version: SourceVersion,
}

/// Cached static analysis result for a document.
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub version: SourceVersion,
    pub diagnostics: Vec<tempra_diagnostics::Diagnostic>,
    pub symbols: Vec<tempra_diagnostics::Symbol>,
}

/// AnalysisSession manages in-memory documents and keeps static analysis cached.
#[derive(Debug, Default)]
pub struct AnalysisSession {
    documents: HashMap<DocumentUri, DocumentSnapshot>,
    analysis_cache: HashMap<DocumentUri, AnalysisResult>,
}

impl AnalysisSession {
    /// Create a new, empty AnalysisSession.
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
            analysis_cache: HashMap::new(),
        }
    }

    /// Open a new document inside the active session.
    pub fn open_document(&mut self, uri: DocumentUri, content: String, version: SourceVersion) {
        let snapshot = DocumentSnapshot {
            uri: uri.clone(),
            content,
            version,
        };
        self.run_and_cache_analysis(&uri, snapshot);
    }

    /// Update an existing document snapshot and refresh diagnostics/symbols.
    pub fn update_document(&mut self, uri: &DocumentUri, content: String, version: SourceVersion) {
        let normalized = DocumentUri::new(uri.as_str());
        let snapshot = DocumentSnapshot {
            uri: normalized.clone(),
            content,
            version,
        };
        self.run_and_cache_analysis(&normalized, snapshot);
    }

    /// Evict document snapshot and analysis results from cache.
    pub fn close_document(&mut self, uri: &DocumentUri) {
        let normalized = DocumentUri::new(uri.as_str());
        self.documents.remove(&normalized);
        self.analysis_cache.remove(&normalized);
    }

    /// Returns true if the session contains the document.
    pub fn contains_document(&self, uri: &DocumentUri) -> bool {
        let normalized = DocumentUri::new(uri.as_str());
        self.documents.contains_key(&normalized)
    }

    /// Retrieve a snapshot of the requested document.
    pub fn snapshot(&self, uri: &DocumentUri) -> Option<DocumentSnapshot> {
        let normalized = DocumentUri::new(uri.as_str());
        self.documents.get(&normalized).cloned()
    }

    /// Retrieve the cached diagnostics for a document.
    pub fn diagnostics(&self, uri: &DocumentUri) -> Option<Vec<tempra_diagnostics::Diagnostic>> {
        let normalized = DocumentUri::new(uri.as_str());
        self.analysis_cache
            .get(&normalized)
            .map(|res| res.diagnostics.clone())
    }

    /// Retrieve the cached symbols for a document.
    pub fn document_symbols(&self, uri: &DocumentUri) -> Option<Vec<tempra_diagnostics::Symbol>> {
        let normalized = DocumentUri::new(uri.as_str());
        self.analysis_cache
            .get(&normalized)
            .map(|res| res.symbols.clone())
    }

    /// Best-effort top-level hover provider.
    pub fn hover(&self, uri: &DocumentUri, line: u32, character: u32) -> Option<String> {
        let normalized = DocumentUri::new(uri.as_str());
        let snapshot = self.documents.get(&normalized)?;
        let word = word_at_position(&snapshot.content, line, character)?;
        let symbols = self.document_symbols(&normalized)?;

        // Search for symbol exact match, or extension method ending match
        let matching_symbol = symbols
            .iter()
            .find(|s| s.name == word || s.name.ends_with(&format!(".{}", word)))?;

        matching_symbol.signature.clone()
    }

    /// Best-effort top-level goto definition resolver.
    pub fn goto_definition(
        &self,
        uri: &DocumentUri,
        line: u32,
        character: u32,
    ) -> Option<tempra_span::Span> {
        let normalized = DocumentUri::new(uri.as_str());
        let snapshot = self.documents.get(&normalized)?;
        let word = word_at_position(&snapshot.content, line, character)?;
        let symbols = self.document_symbols(&normalized)?;

        // Search for symbol exact match, or extension method ending match
        let matching_symbol = symbols
            .iter()
            .find(|s| s.name == word || s.name.ends_with(&format!(".{}", word)))?;

        Some(matching_symbol.span)
    }

    fn run_and_cache_analysis(&mut self, uri: &DocumentUri, snapshot: DocumentSnapshot) {
        let diags = diagnose_source(&snapshot.content);
        let symbols = extract_symbols_from_source(&snapshot.content).unwrap_or_default();

        let result = AnalysisResult {
            version: snapshot.version,
            diagnostics: diags,
            symbols,
        };

        self.documents.insert(uri.clone(), snapshot);
        self.analysis_cache.insert(uri.clone(), result);
    }
}

/// Find word/identifier at position.
pub fn word_at_position(source: &str, line: u32, character: u32) -> Option<String> {
    let byte_offset = position_to_offset(source, line, character)?;
    word_at_offset(source, byte_offset)
}

fn position_to_offset(source: &str, line: u32, character: u32) -> Option<usize> {
    let mut current_line = 0;
    let mut current_char = 0;

    let mut chars = source.char_indices().peekable();
    while let Some(&(idx, ch)) = chars.peek() {
        if current_line == line && current_char == character {
            return Some(idx);
        }

        let _ = chars.next();
        if ch == '\n' {
            current_line += 1;
            current_char = 0;
        } else if ch == '\r' {
            if let Some(&(_, '\n')) = chars.peek() {
                let _ = chars.next();
            }
            current_line += 1;
            current_char = 0;
        } else {
            current_char += 1;
        }
    }

    None
}

fn word_at_offset(source: &str, byte_offset: usize) -> Option<String> {
    if byte_offset >= source.len() {
        return None;
    }

    let chars: Vec<(usize, char)> = source.char_indices().collect();
    let char_pos = chars.iter().position(|&(idx, _)| idx == byte_offset)?;
    let (_, ch) = chars[char_pos];

    if !is_word_char(ch) {
        if char_pos > 0 {
            let (_, prev_ch) = chars[char_pos - 1];
            if is_word_char(prev_ch) {
                return extract_word_at_index(&chars, char_pos - 1);
            }
        }
        return None;
    }

    extract_word_at_index(&chars, char_pos)
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn extract_word_at_index(chars: &[(usize, char)], char_pos: usize) -> Option<String> {
    let mut start = char_pos;
    while start > 0 && is_word_char(chars[start - 1].1) {
        start -= 1;
    }

    let mut end = char_pos;
    while end < chars.len() && is_word_char(chars[end].1) {
        end += 1;
    }

    let word: String = chars[start..end].iter().map(|&(_, c)| c).collect();
    if word.is_empty() {
        None
    } else {
        Some(word)
    }
}
