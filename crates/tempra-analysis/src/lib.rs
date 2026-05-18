//! Editor-facing analysis layer, Coordinate Adapters, and LSP Core Foundation for Tempra.
//!
//! Separates Tempra internal UTF-8 character coordinates (`TextPosition`) from
//! external UTF-16 code-unit coordinates (`LspPosition`) at the boundary to prevent
//! coordinate drift while managing document caches and lightweight symbol indices.

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

/// Internal 0-indexed position using UTF-8 byte offsets within a line.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct TextPosition {
    pub line: u32,
    pub byte_character: u32,
}

/// External 0-indexed position using UTF-16 code-unit offsets within a line (LSP standard).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct LspPosition {
    pub line: u32,
    pub character: u32,
}

/// Range defined by internal UTF-8 coordinates.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct TextRange {
    pub start: TextPosition,
    pub end: TextPosition,
}

/// Range defined by external UTF-16 coordinates.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct LspRange {
    pub start: LspPosition,
    pub end: LspPosition,
}

/// Bidirectional index mapping coordinates to byte offsets.
#[derive(Debug, Clone)]
pub struct LineIndex {
    line_starts: Vec<usize>,
    len: usize,
}

impl LineIndex {
    /// Compute the LineIndex for a source text.
    pub fn new(text: &str) -> Self {
        let mut line_starts = vec![0];
        for (offset, c) in text.char_indices() {
            if c == '\n' {
                line_starts.push(offset + 1);
            }
        }
        Self {
            line_starts,
            len: text.len(),
        }
    }

    /// Access the raw text of a specific 0-indexed line, stripping line endings.
    pub fn line_text<'a>(&self, text: &'a str, line: usize) -> Option<&'a str> {
        if line >= self.line_starts.len() {
            return None;
        }
        let start = self.line_starts[line];
        let end = if line + 1 < self.line_starts.len() {
            self.line_starts[line + 1]
        } else {
            self.len
        };

        let mut slice = &text[start..end];
        if slice.ends_with('\n') {
            slice = &slice[..slice.len() - 1];
        }
        if slice.ends_with('\r') {
            slice = &slice[..slice.len() - 1];
        }
        Some(slice)
    }

    /// Map external LSP UTF-16 coordinates to internal UTF-8 byte character offsets.
    pub fn lsp_to_text_position(&self, text: &str, pos: LspPosition) -> Option<TextPosition> {
        let line = pos.line as usize;
        let line_text = self.line_text(text, line)?;

        let mut utf16_offset = 0;
        let mut byte_char = 0;

        for c in line_text.chars() {
            if utf16_offset == pos.character as usize {
                return Some(TextPosition {
                    line: pos.line,
                    byte_character: byte_char as u32,
                });
            }

            utf16_offset += c.len_utf16();
            byte_char += c.len_utf8();

            if utf16_offset > pos.character as usize {
                // Requested index falls mid-codepoint / mid-surrogate
                return None;
            }
        }

        if utf16_offset == pos.character as usize {
            Some(TextPosition {
                line: pos.line,
                byte_character: byte_char as u32,
            })
        } else {
            None
        }
    }

    /// Map internal UTF-8 byte character offsets to external LSP UTF-16 coordinates.
    pub fn text_to_lsp_position(&self, text: &str, pos: TextPosition) -> Option<LspPosition> {
        let line = pos.line as usize;
        let line_text = self.line_text(text, line)?;

        let mut utf16_offset = 0;
        let mut byte_offset = 0;

        for c in line_text.chars() {
            if byte_offset == pos.byte_character as usize {
                return Some(LspPosition {
                    line: pos.line,
                    character: utf16_offset as u32,
                });
            }

            utf16_offset += c.len_utf16();
            byte_offset += c.len_utf8();
        }

        if byte_offset == pos.byte_character as usize {
            Some(LspPosition {
                line: pos.line,
                character: utf16_offset as u32,
            })
        } else {
            None
        }
    }

    /// Translate LSP UTF-16 position directly to an absolute byte offset.
    pub fn lsp_position_to_byte_offset(&self, text: &str, pos: LspPosition) -> Option<usize> {
        let text_pos = self.lsp_to_text_position(text, pos)?;
        let line_start = self.line_starts.get(text_pos.line as usize)?;
        Some(*line_start + text_pos.byte_character as usize)
    }

    /// Translate an absolute byte offset directly to an external LSP UTF-16 position.
    pub fn byte_offset_to_lsp_position(&self, text: &str, offset: usize) -> Option<LspPosition> {
        let text_pos = self.byte_offset_to_text_position(text, offset)?;
        self.text_to_lsp_position(text, text_pos)
    }

    /// Translate an absolute byte offset to an internal UTF-8 position.
    pub fn byte_offset_to_text_position(&self, _text: &str, offset: usize) -> Option<TextPosition> {
        if offset > self.len {
            return None;
        }
        let line = match self.line_starts.binary_search(&offset) {
            Ok(idx) => idx,
            Err(idx) => idx - 1,
        };
        let line_start = self.line_starts[line];
        let byte_char = offset - line_start;
        Some(TextPosition {
            line: line as u32,
            byte_character: byte_char as u32,
        })
    }

    /// Translate an internal UTF-8 position to an absolute byte offset.
    pub fn text_position_to_byte_offset(&self, _text: &str, pos: TextPosition) -> Option<usize> {
        let line = pos.line as usize;
        let line_start = *self.line_starts.get(line)?;
        let line_end = if line + 1 < self.line_starts.len() {
            self.line_starts[line + 1]
        } else {
            self.len
        };
        let byte_offset = line_start + pos.byte_character as usize;
        if byte_offset <= line_end {
            Some(byte_offset)
        } else {
            None
        }
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
    pub line_index: LineIndex,
}

impl DocumentSnapshot {
    pub fn text(&self) -> &str {
        &self.content
    }

    pub fn version(&self) -> SourceVersion {
        self.version
    }

    pub fn line_index(&self) -> &LineIndex {
        &self.line_index
    }

    pub fn identifier_at(&self, line: u32, byte_character: u32) -> Option<String> {
        let pos = TextPosition {
            line,
            byte_character,
        };
        let offset = self
            .line_index
            .text_position_to_byte_offset(&self.content, pos)?;
        word_at_offset(&self.content, offset)
    }
}

/// Spatial and exact/suffix lookup indices for Document symbols.
#[derive(Debug, Clone)]
pub struct SymbolIndex {
    by_name: HashMap<String, usize>,
    by_span: Vec<(tempra_span::Span, usize)>,
}

impl SymbolIndex {
    pub fn new(symbols: &[tempra_diagnostics::Symbol]) -> Self {
        let mut by_name = HashMap::new();
        let mut by_span = Vec::new();

        for (idx, sym) in symbols.iter().enumerate() {
            by_name.insert(sym.name.clone(), idx);
            if let Some(dot_idx) = sym.name.rfind('.') {
                let suffix = &sym.name[dot_idx + 1..];
                by_name.entry(suffix.to_string()).or_insert(idx);
            }
            by_span.push((sym.span, idx));
        }

        by_span.sort_by_key(|&(span, _)| span.start);

        Self { by_name, by_span }
    }

    pub fn find_by_name(&self, name: &str) -> Option<usize> {
        self.by_name.get(name).copied()
    }

    pub fn find_by_offset(&self, offset: usize) -> Option<usize> {
        let offset_u32 = offset as u32;
        let mut best_match: Option<(usize, u32)> = None;
        for &(span, idx) in &self.by_span {
            if span.start <= offset_u32 && offset_u32 <= span.end {
                let len = span.end - span.start;
                if let Some((_, best_len)) = best_match {
                    if len < best_len {
                        best_match = Some((idx, len));
                    }
                } else {
                    best_match = Some((idx, len));
                }
            }
        }
        best_match.map(|(idx, _)| idx)
    }
}

/// Cached static analysis result for a document.
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub version: SourceVersion,
    pub diagnostics: Vec<tempra_diagnostics::Diagnostic>,
    pub symbols: Vec<tempra_diagnostics::Symbol>,
    pub symbol_index: SymbolIndex,
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
        let line_index = LineIndex::new(&content);
        let snapshot = DocumentSnapshot {
            uri: uri.clone(),
            content,
            version,
            line_index,
        };
        self.run_and_cache_analysis(&uri, snapshot);
    }

    /// Update an existing document snapshot and refresh diagnostics/symbols.
    pub fn update_document(&mut self, uri: &DocumentUri, content: String, version: SourceVersion) {
        let normalized = DocumentUri::new(uri.as_str());
        let line_index = LineIndex::new(&content);
        let snapshot = DocumentSnapshot {
            uri: normalized.clone(),
            content,
            version,
            line_index,
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

    // --- Internal-first UTF-8 Position Queries ---

    /// Retrieve symbol at the given internal coordinate.
    pub fn symbol_at_position(
        &self,
        uri: &DocumentUri,
        line: u32,
        byte_character: u32,
    ) -> Option<tempra_diagnostics::Symbol> {
        let normalized = DocumentUri::new(uri.as_str());
        let snapshot = self.documents.get(&normalized)?;
        let result = self.analysis_cache.get(&normalized)?;

        let word = snapshot.identifier_at(line, byte_character)?;
        let sym_idx = result.symbol_index.find_by_name(&word)?;

        Some(result.symbols[sym_idx].clone())
    }

    /// Retrieve definition location at the given internal coordinate.
    pub fn definition_at_position(
        &self,
        uri: &DocumentUri,
        line: u32,
        byte_character: u32,
    ) -> Option<tempra_span::Span> {
        self.symbol_at_position(uri, line, byte_character)
            .map(|sym| sym.span)
    }

    /// Retrieve hover telemetry message at the given internal coordinate.
    pub fn hover_at_position(
        &self,
        uri: &DocumentUri,
        line: u32,
        byte_character: u32,
    ) -> Option<String> {
        self.symbol_at_position(uri, line, byte_character)
            .and_then(|sym| sym.signature.clone())
    }

    // --- Outer LSP UTF-16 Position Adapters ---

    /// Retrieve symbol at the given external LSP coordinate.
    pub fn symbol_at_lsp_position(
        &self,
        uri: &DocumentUri,
        pos: LspPosition,
    ) -> Option<tempra_diagnostics::Symbol> {
        let normalized = DocumentUri::new(uri.as_str());
        let snapshot = self.documents.get(&normalized)?;
        let text_pos = snapshot
            .line_index
            .lsp_to_text_position(&snapshot.content, pos)?;
        self.symbol_at_position(&normalized, text_pos.line, text_pos.byte_character)
    }

    /// Retrieve definition location at the given external LSP coordinate.
    pub fn definition_at_lsp_position(
        &self,
        uri: &DocumentUri,
        pos: LspPosition,
    ) -> Option<tempra_span::Span> {
        let normalized = DocumentUri::new(uri.as_str());
        let snapshot = self.documents.get(&normalized)?;
        let text_pos = snapshot
            .line_index
            .lsp_to_text_position(&snapshot.content, pos)?;
        self.definition_at_position(&normalized, text_pos.line, text_pos.byte_character)
    }

    /// Retrieve hover telemetry message at the given external LSP coordinate.
    pub fn hover_at_lsp_position(&self, uri: &DocumentUri, pos: LspPosition) -> Option<String> {
        let normalized = DocumentUri::new(uri.as_str());
        let snapshot = self.documents.get(&normalized)?;
        let text_pos = snapshot
            .line_index
            .lsp_to_text_position(&snapshot.content, pos)?;
        self.hover_at_position(&normalized, text_pos.line, text_pos.byte_character)
    }

    /// Best-effort top-level hover provider.
    pub fn hover(&self, uri: &DocumentUri, line: u32, character: u32) -> Option<String> {
        self.hover_at_position(uri, line, character)
    }

    /// Best-effort top-level goto definition resolver.
    pub fn goto_definition(
        &self,
        uri: &DocumentUri,
        line: u32,
        character: u32,
    ) -> Option<tempra_span::Span> {
        self.definition_at_position(uri, line, character)
    }

    fn run_and_cache_analysis(&mut self, uri: &DocumentUri, snapshot: DocumentSnapshot) {
        let diags = diagnose_source(&snapshot.content);
        let symbols = extract_symbols_from_source(&snapshot.content).unwrap_or_default();
        let symbol_index = SymbolIndex::new(&symbols);

        let result = AnalysisResult {
            version: snapshot.version,
            diagnostics: diags,
            symbols,
            symbol_index,
        };

        self.documents.insert(uri.clone(), snapshot);
        self.analysis_cache.insert(uri.clone(), result);
    }
}

/// Find word/identifier at absolute byte offset.
pub fn word_at_offset(source: &str, byte_offset: usize) -> Option<String> {
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
