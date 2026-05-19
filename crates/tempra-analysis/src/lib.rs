//! Editor-facing analysis layer, Coordinate Adapters, and LSP Core Foundation for Tempra.
//!
//! Separates Tempra internal UTF-8 character coordinates (`TextPosition`) from
//! external UTF-16 code-unit coordinates (`LspPosition`) at the boundary to prevent
//! coordinate drift while managing document caches and lightweight symbol indices.

use std::collections::HashMap;
use tempra_driver::pipeline::{diagnose_source_with_facts, extract_symbols_from_source};

/// A normalized, unique Document URI representation.
#[derive(
    Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct DocumentUri(String);

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CompletionKind {
    Keyword,
    Function,
    Struct,
    Enum,
    Variable,
    Method,
    Type,
    Module,
    Field,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: Option<String>,
    pub insert_text: Option<String>,
    pub sort_text: Option<String>,
    pub documentation: Option<String>,
}

/// Extract base struct/enum/type name for completion lookups
fn type_to_base_name(ty: &tempra_typeck::Type) -> Option<String> {
    match ty {
        tempra_typeck::Type::Struct(name, _) => Some(name.clone()),
        tempra_typeck::Type::Enum(name, _) => Some(name.clone()),
        tempra_typeck::Type::Interface(name) => Some(name.clone()),
        tempra_typeck::Type::String => Some("String".to_string()),
        tempra_typeck::Type::Bool => Some("Bool".to_string()),
        tempra_typeck::Type::Int => Some("Int".to_string()),
        tempra_typeck::Type::Int8 => Some("Int8".to_string()),
        tempra_typeck::Type::Int16 => Some("Int16".to_string()),
        tempra_typeck::Type::Int32 => Some("Int32".to_string()),
        tempra_typeck::Type::Int64 => Some("Int64".to_string()),
        tempra_typeck::Type::UInt8 => Some("UInt8".to_string()),
        tempra_typeck::Type::UInt16 => Some("UInt16".to_string()),
        tempra_typeck::Type::UInt32 => Some("UInt32".to_string()),
        tempra_typeck::Type::UInt64 => Some("UInt64".to_string()),
        tempra_typeck::Type::Float32 => Some("Float32".to_string()),
        tempra_typeck::Type::Float64 => Some("Float64".to_string()),
        tempra_typeck::Type::Array(_) => Some("Array".to_string()),
        tempra_typeck::Type::Slice(_) => Some("Slice".to_string()),
        tempra_typeck::Type::Result(_, _) => Some("Result".to_string()),
        tempra_typeck::Type::Option(_) => Some("Option".to_string()),
        tempra_typeck::Type::Unknown | tempra_typeck::Type::Error => None,
        // Fallback to string representation for other complex types
        other => {
            let s = other.to_string();
            // If it looks like a simple name, keep it, otherwise drop
            if !s.contains('(') && !s.contains('<') {
                Some(s)
            } else {
                None
            }
        }
    }
}

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

    /// Convert a `tempra_span::Span` (byte offsets) to an external LSP `LspRange`.
    ///
    /// Returns `None` if either offset falls outside the document.
    pub fn span_to_lsp_range(&self, text: &str, span: tempra_span::Span) -> Option<LspRange> {
        let start = self.byte_offset_to_lsp_position(text, span.start as usize)?;
        let end = self.byte_offset_to_lsp_position(text, span.end as usize)?;
        Some(LspRange { start, end })
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
    pub semantic_index: SemanticIndex,
    pub facts: Option<tempra_typeck::TypecheckFacts>,
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

        let pos = TextPosition {
            line,
            byte_character,
        };
        if let Some(offset) = snapshot
            .line_index
            .text_position_to_byte_offset(&snapshot.content, pos)
        {
            // Prefer SemanticIndex first
            if let Some(sem_sym) = result.semantic_index.find_by_offset(offset) {
                return Some(tempra_diagnostics::Symbol {
                    name: sem_sym.name.clone(),
                    kind: sem_sym.kind,
                    span: sem_sym.definition_span,
                    signature: sem_sym.signature.clone(),
                });
            }
        }

        // Fallback to lexical outline lookups
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
        let normalized = DocumentUri::new(uri.as_str());
        let snapshot = self.documents.get(&normalized)?;
        let result = self.analysis_cache.get(&normalized)?;

        let pos = TextPosition {
            line,
            byte_character,
        };
        let offset = snapshot
            .line_index
            .text_position_to_byte_offset(&snapshot.content, pos)?;

        // Query Order:
        // 1. TypecheckFacts first
        if let Some(facts) = &result.facts {
            let offset_u32 = offset as u32;

            // Check member resolutions (fields)
            for (&span, &def_span) in &facts.member_resolutions {
                if span.start <= offset_u32 && offset_u32 <= span.end {
                    return Some(def_span);
                }
            }

            // Check variant resolutions
            for (&span, &def_span) in &facts.variant_resolutions {
                if span.start <= offset_u32 && offset_u32 <= span.end {
                    return Some(def_span);
                }
            }

            // Check method resolutions
            for (&span, &def_span) in &facts.method_resolutions {
                if span.start <= offset_u32 && offset_u32 <= span.end {
                    return Some(def_span);
                }
            }

            // Check resolved symbols
            for (&span, &def_span) in &facts.resolved_symbols {
                if span.start <= offset_u32 && offset_u32 <= span.end {
                    return Some(def_span);
                }
            }
        }

        // 2. Fallback to SemanticIndex next
        if let Some(sem_sym) = result.semantic_index.find_by_offset(offset) {
            return Some(sem_sym.definition_span);
        }

        // 3. Fallback to SymbolIndex (lexical outline lookups)
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
        let normalized = DocumentUri::new(uri.as_str());
        let snapshot = self.documents.get(&normalized)?;
        let result = self.analysis_cache.get(&normalized)?;

        let pos = TextPosition {
            line,
            byte_character,
        };
        let offset = snapshot
            .line_index
            .text_position_to_byte_offset(&snapshot.content, pos)?;

        // 1. Find the lexical SemanticSymbol first to understand the context/prefix/rich format.
        let sem_sym = result.semantic_index.find_by_offset(offset);

        // 2. Query TypecheckFacts to get the authoritative type.
        let mut authoritative_ty = None;
        if let Some(facts) = &result.facts {
            let offset_u32 = offset as u32;

            // Check binding/parameter spans
            for (&span, ty) in &facts.binding_types {
                if span.start <= offset_u32 && offset_u32 <= span.end {
                    authoritative_ty = Some(ty.clone());
                    break;
                }
            }

            if authoritative_ty.is_none() {
                // Check expression spans (innermost/smallest first)
                let mut matched_exprs: Vec<(&tempra_span::Span, &tempra_typeck::Type)> = facts
                    .expr_types
                    .iter()
                    .filter(|(&span, _)| span.start <= offset_u32 && offset_u32 <= span.end)
                    .collect();
                matched_exprs.sort_by_key(|&(span, _)| span.end - span.start);
                if let Some((_, ty)) = matched_exprs.first() {
                    authoritative_ty = Some((*ty).clone());
                }
            }
        }

        // 3. Combine them beautifully!
        if let Some(sym) = sem_sym {
            if let Some(ref ty) = authoritative_ty {
                if sym.kind == tempra_diagnostics::SymbolKind::Variable {
                    if let Some(sig) = &sym.signature {
                        if sig.contains(": ") {
                            let parts: Vec<&str> = sig.splitn(2, ": ").collect();
                            if parts.len() == 2 {
                                return Some(format!("{}: {}", parts[0], ty));
                            }
                        }
                    }
                }
            }
            // Return lexical signature as fallback
            if let Some(sig) = &sym.signature {
                return Some(sig.clone());
            }
        }

        // 4. Fallback to just the authoritative type if no lexical symbol was found
        if let Some(ty) = authoritative_ty {
            return Some(format!("{}", ty));
        }

        // 5. Fallback to SymbolIndex (lexical outline lookups)
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

    /// Convert a `tempra_span::Span` to an `LspRange` using the document's line index.
    ///
    /// Returns `None` if the document is not open or if offsets are out of range.
    pub fn span_to_lsp_range(
        &self,
        uri: &DocumentUri,
        span: tempra_span::Span,
    ) -> Option<LspRange> {
        let normalized = DocumentUri::new(uri.as_str());
        let snapshot = self.documents.get(&normalized)?;
        snapshot
            .line_index
            .span_to_lsp_range(&snapshot.content, span)
    }

    /// Retrieve completions at the given internal coordinate.
    pub fn completions_at_position(
        &self,
        uri: &DocumentUri,
        line: u32,
        byte_character: u32,
    ) -> Vec<CompletionItem> {
        let normalized = DocumentUri::new(uri.as_str());
        let snapshot = match self.documents.get(&normalized) {
            Some(s) => s,
            None => return vec![],
        };
        let result = match self.analysis_cache.get(&normalized) {
            Some(r) => r,
            None => return vec![],
        };

        let pos = TextPosition {
            line,
            byte_character,
        };
        let offset = match snapshot
            .line_index
            .text_position_to_byte_offset(&snapshot.content, pos)
        {
            Some(o) => o,
            None => return vec![],
        };

        let mut dot_idx = 0;
        let is_member_access = {
            let mut ptr = offset;
            let bytes = snapshot.content.as_bytes();
            // Skip the current word we might be typing
            while ptr > 0 {
                let ch = bytes[ptr - 1] as char;
                if ch.is_alphanumeric() || ch == '_' {
                    ptr -= 1;
                } else {
                    break;
                }
            }
            // Skip spaces
            while ptr > 0 {
                let ch = bytes[ptr - 1] as char;
                if ch.is_whitespace() {
                    ptr -= 1;
                } else {
                    break;
                }
            }
            if ptr > 0 && bytes[ptr - 1] == b'.' {
                dot_idx = ptr - 1;
                true
            } else {
                false
            }
        };

        let mut items = Vec::new();
        let mut seen = std::collections::HashSet::new();

        let mut add_item = |label: &str, kind: CompletionKind, detail: Option<String>| {
            if seen.insert(label.to_string()) {
                items.push(CompletionItem {
                    label: label.to_string(),
                    kind,
                    detail,
                    insert_text: None,
                    sort_text: None,
                    documentation: None,
                });
            }
        };

        if is_member_access {
            let mut receiver_type = None;
            if let Some(facts) = &result.facts {
                // TODO: future TypecheckFacts should expose receiver_span/member_completion facts directly.
                // Using expr_types span ending before '.' is an MVP strategy.
                let mut matched_exprs: Vec<(&tempra_span::Span, &tempra_typeck::Type)> = facts
                    .expr_types
                    .iter()
                    .filter(|(&span, _)| span.end <= dot_idx as u32)
                    .collect();
                matched_exprs.sort_by_key(|&(span, _)| dot_idx as u32 - span.end);

                if let Some((_, ty)) = matched_exprs.first() {
                    receiver_type = Some((*ty).clone());
                }
            }

            if let Some(ty) = receiver_type {
                if let Some(base_name) = type_to_base_name(&ty) {
                    let prefix = format!("{}.", base_name);
                    for sym in &result.symbols {
                        if sym.name.starts_with(&prefix) {
                            // Point.x -> label 'x'
                            let label = &sym.name[prefix.len()..];
                            let kind = match sym.kind {
                                tempra_diagnostics::SymbolKind::Field => CompletionKind::Field,
                                tempra_diagnostics::SymbolKind::Method => CompletionKind::Method,
                                tempra_diagnostics::SymbolKind::Function => {
                                    CompletionKind::Function
                                }
                                _ => CompletionKind::Method,
                            };
                            add_item(label, kind, sym.signature.clone());
                        }
                    }
                }
            }

            // Empty Vec fallback
            return items;
        }

        // 1. Keywords
        let keywords = [
            "fn", "let", "var", "struct", "enum", "extend", "if", "else", "match", "for", "in",
            "return", "break", "continue", "true", "false", "comptime",
        ];
        for kw in keywords {
            add_item(kw, CompletionKind::Keyword, None);
        }

        // 2. Builtin Types
        let types = [
            "Int8", "Int16", "Int32", "Int64", "UInt8", "UInt16", "UInt32", "UInt64", "Float32",
            "Float64", "Bool", "String", "Never", "Option", "Result",
        ];
        for t in types {
            add_item(t, CompletionKind::Type, None);
        }

        // 3. Root namespaces
        let namespaces = ["fs", "os", "env", "cmd", "io"];
        for ns in namespaces {
            add_item(ns, CompletionKind::Module, None);
        }

        // 4. Local bindings from SemanticIndex
        // Only include symbols defined before the current offset
        for sym in &result.semantic_index.symbols {
            if sym.definition_span.start <= offset as u32 {
                let kind = match sym.kind {
                    tempra_diagnostics::SymbolKind::Function => CompletionKind::Function,
                    tempra_diagnostics::SymbolKind::Struct => CompletionKind::Struct,
                    tempra_diagnostics::SymbolKind::Enum => CompletionKind::Enum,
                    tempra_diagnostics::SymbolKind::Method => CompletionKind::Method,
                    tempra_diagnostics::SymbolKind::Extension => CompletionKind::Module,
                    tempra_diagnostics::SymbolKind::Variable => CompletionKind::Variable,
                    tempra_diagnostics::SymbolKind::Field => CompletionKind::Field,
                };
                add_item(&sym.name, kind, sym.signature.clone());
            }
        }

        // 5. Top-level symbols from SymbolIndex
        for sym in &result.symbols {
            let kind = match sym.kind {
                tempra_diagnostics::SymbolKind::Function => CompletionKind::Function,
                tempra_diagnostics::SymbolKind::Struct => CompletionKind::Struct,
                tempra_diagnostics::SymbolKind::Enum => CompletionKind::Enum,
                tempra_diagnostics::SymbolKind::Method => CompletionKind::Method,
                tempra_diagnostics::SymbolKind::Extension => CompletionKind::Module,
                tempra_diagnostics::SymbolKind::Variable => CompletionKind::Variable,
                tempra_diagnostics::SymbolKind::Field => CompletionKind::Field,
            };
            add_item(&sym.name, kind, sym.signature.clone());
        }

        items
    }

    /// Retrieve completions at the given external LSP coordinate.
    pub fn completions_at_lsp_position(
        &self,
        uri: &DocumentUri,
        pos: LspPosition,
    ) -> Vec<CompletionItem> {
        let normalized = DocumentUri::new(uri.as_str());
        let snapshot = match self.documents.get(&normalized) {
            Some(s) => s,
            None => return vec![],
        };
        let text_pos = match snapshot
            .line_index
            .lsp_to_text_position(&snapshot.content, pos)
        {
            Some(p) => p,
            None => return vec![],
        };
        self.completions_at_position(&normalized, text_pos.line, text_pos.byte_character)
    }

    fn run_and_cache_analysis(&mut self, uri: &DocumentUri, snapshot: DocumentSnapshot) {
        let facts_res = diagnose_source_with_facts(&snapshot.content);
        let diags = facts_res.diagnostics;
        let facts = facts_res.facts;
        let symbols = extract_symbols_from_source(&snapshot.content).unwrap_or_default();
        let symbol_index = SymbolIndex::new(&symbols);

        let semantic_index = if let Ok(file) = tempra_parser::parse_source(&snapshot.content) {
            SemanticResolver::new().resolve(&file, &snapshot.content)
        } else {
            SemanticIndex::default()
        };

        let result = AnalysisResult {
            version: snapshot.version,
            diagnostics: diags,
            symbols,
            symbol_index,
            semantic_index,
            facts,
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

// ============================================================================
// Phase 18.1C — Real Semantic Resolution Foundation Structures and Walkers
// ============================================================================

#[derive(Debug, Clone)]
pub struct SemanticSymbol {
    pub name: String,
    pub kind: tempra_diagnostics::SymbolKind,
    pub definition_span: tempra_span::Span,
    pub reference_spans: Vec<tempra_span::Span>,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SemanticIndex {
    pub symbols: Vec<SemanticSymbol>,
}

impl SemanticIndex {
    pub fn find_by_offset(&self, offset: usize) -> Option<&SemanticSymbol> {
        let offset_u32 = offset as u32;
        let mut best_match: Option<(&SemanticSymbol, u32)> = None;
        for symbol in &self.symbols {
            if symbol.definition_span.start <= offset_u32
                && offset_u32 <= symbol.definition_span.end
            {
                let len = symbol.definition_span.end - symbol.definition_span.start;
                if let Some((_, best_len)) = best_match {
                    if len < best_len {
                        best_match = Some((symbol, len));
                    }
                } else {
                    best_match = Some((symbol, len));
                }
            }
            for ref_span in &symbol.reference_spans {
                if ref_span.start <= offset_u32 && offset_u32 <= ref_span.end {
                    let len = ref_span.end - ref_span.start;
                    if let Some((_, best_len)) = best_match {
                        if len < best_len {
                            best_match = Some((symbol, len));
                        }
                    } else {
                        best_match = Some((symbol, len));
                    }
                }
            }
        }
        best_match.map(|(sym, _)| sym)
    }
}

fn get_token_text<'a>(token: tempra_lexer::Token, source: &'a str) -> &'a str {
    let start = token.span.start as usize;
    let end = token.span.end as usize;
    if start <= end && end <= source.len() {
        &source[start..end]
    } else {
        ""
    }
}

fn get_type_expr_string(ty: &tempra_ast::TypeExpr, source: &str) -> String {
    let span = ty.span();
    let start = span.start as usize;
    let end = span.end as usize;
    if start <= end && end <= source.len() {
        source[start..end].to_string()
    } else {
        "".to_string()
    }
}

pub struct SemanticResolver {
    globals: HashMap<String, usize>,
    scopes: Vec<HashMap<String, usize>>,
    symbols: Vec<SemanticSymbol>,
}

impl SemanticResolver {
    pub fn new() -> Self {
        Self {
            globals: HashMap::new(),
            scopes: vec![HashMap::new()],
            symbols: vec![],
        }
    }

    pub fn resolve(mut self, file: &tempra_ast::File, source: &str) -> SemanticIndex {
        // Pre-pass: Populate globals (functions, structs, enums, interfaces, extends)
        for item in &file.items {
            match item {
                tempra_ast::Item::Function(func) => {
                    let name = get_token_text(func.name, source).to_string();
                    let start = func.span.start as usize;
                    let end = func.body.span.start as usize;
                    let sig = if start <= end && end <= source.len() {
                        source[start..end].trim().to_string()
                    } else {
                        format!("fn {}(...) -> ...", name)
                    };
                    let sym = SemanticSymbol {
                        name: name.clone(),
                        kind: tempra_diagnostics::SymbolKind::Function,
                        definition_span: func.name.span,
                        reference_spans: vec![],
                        signature: Some(sig),
                    };
                    self.symbols.push(sym);
                    let idx = self.symbols.len() - 1;
                    self.globals.insert(name, idx);
                }
                tempra_ast::Item::Struct(s) => {
                    let name = get_token_text(s.name, source).to_string();
                    let sig = format!("struct {}", name);
                    let sym = SemanticSymbol {
                        name: name.clone(),
                        kind: tempra_diagnostics::SymbolKind::Struct,
                        definition_span: s.name.span,
                        reference_spans: vec![],
                        signature: Some(sig),
                    };
                    self.symbols.push(sym);
                    let idx = self.symbols.len() - 1;
                    self.globals.insert(name.clone(), idx);

                    // Pre-pass fields for best-effort lookups
                    for field in &s.fields {
                        let field_name = get_token_text(field.name, source).to_string();
                        let field_sig = format!(
                            "{}.{}: {}",
                            name,
                            field_name,
                            get_type_expr_string(&field.ty, source)
                        );
                        let f_sym = SemanticSymbol {
                            name: field_name.clone(),
                            kind: tempra_diagnostics::SymbolKind::Variable,
                            definition_span: field.name.span,
                            reference_spans: vec![],
                            signature: Some(field_sig),
                        };
                        self.symbols.push(f_sym);
                        let f_idx = self.symbols.len() - 1;
                        self.globals.insert(field_name, f_idx);
                    }
                }
                tempra_ast::Item::Enum(e) => {
                    let name = get_token_text(e.name, source).to_string();
                    let sig = format!("enum {}", name);
                    let sym = SemanticSymbol {
                        name: name.clone(),
                        kind: tempra_diagnostics::SymbolKind::Enum,
                        definition_span: e.name.span,
                        reference_spans: vec![],
                        signature: Some(sig),
                    };
                    self.symbols.push(sym);
                    let idx = self.symbols.len() - 1;
                    self.globals.insert(name.clone(), idx);

                    // Pre-pass variants for best-effort lookups
                    for variant in &e.variants {
                        let var_name = get_token_text(variant.name, source).to_string();
                        let var_sig = format!("{}.{}", name, var_name);
                        let v_sym = SemanticSymbol {
                            name: var_name.clone(),
                            kind: tempra_diagnostics::SymbolKind::Variable,
                            definition_span: variant.name.span,
                            reference_spans: vec![],
                            signature: Some(var_sig),
                        };
                        self.symbols.push(v_sym);
                        let v_idx = self.symbols.len() - 1;
                        self.globals.insert(var_name, v_idx);
                    }
                }
                tempra_ast::Item::Interface(itf) => {
                    let name = get_token_text(itf.name, source).to_string();
                    let sig = format!("interface {}", name);
                    let sym = SemanticSymbol {
                        name: name.clone(),
                        kind: tempra_diagnostics::SymbolKind::Struct,
                        definition_span: itf.name.span,
                        reference_spans: vec![],
                        signature: Some(sig),
                    };
                    self.symbols.push(sym);
                    let idx = self.symbols.len() - 1;
                    self.globals.insert(name, idx);
                }
                tempra_ast::Item::Extend(ext) => {
                    let target_name = match &ext.target {
                        tempra_ast::TypeExpr::Named(n) => {
                            get_token_text(n.name, source).to_string()
                        }
                        _ => "ExtendedType".to_string(),
                    };
                    let sig = format!("extend {}", target_name);
                    let sym = SemanticSymbol {
                        name: target_name.clone(),
                        kind: tempra_diagnostics::SymbolKind::Extension,
                        definition_span: ext.span,
                        reference_spans: vec![],
                        signature: Some(sig),
                    };
                    self.symbols.push(sym);
                    let idx = self.symbols.len() - 1;
                    self.globals.insert(target_name.clone(), idx);

                    // Pre-pass methods
                    for method in &ext.methods {
                        let method_name = get_token_text(method.name, source).to_string();
                        let start = method.span.start as usize;
                        let end = method.body.span.start as usize;
                        let method_sig = if start <= end && end <= source.len() {
                            source[start..end].trim().to_string()
                        } else {
                            format!("fn {}.{}(...) -> ...", target_name, method_name)
                        };
                        let m_sym = SemanticSymbol {
                            name: method_name.clone(),
                            kind: tempra_diagnostics::SymbolKind::Method,
                            definition_span: method.name.span,
                            reference_spans: vec![],
                            signature: Some(method_sig),
                        };
                        self.symbols.push(m_sym);
                        let m_idx = self.symbols.len() - 1;
                        self.globals.insert(method_name, m_idx);
                    }
                }
                _ => {}
            }
        }

        // Scan top-level let/var statements
        for stmt in &file.top_level_statements {
            match stmt {
                tempra_ast::Statement::Let(let_stmt) => {
                    let name = get_token_text(let_stmt.name, source).to_string();
                    let sig = format!("let {}", name);
                    let sym = SemanticSymbol {
                        name: name.clone(),
                        kind: tempra_diagnostics::SymbolKind::Variable,
                        definition_span: let_stmt.name.span,
                        reference_spans: vec![],
                        signature: Some(sig),
                    };
                    self.symbols.push(sym);
                    let idx = self.symbols.len() - 1;
                    self.globals.insert(name, idx);
                }
                tempra_ast::Statement::Var(var_stmt) => {
                    let name = get_token_text(var_stmt.name, source).to_string();
                    let sig = format!("var {}", name);
                    let sym = SemanticSymbol {
                        name: name.clone(),
                        kind: tempra_diagnostics::SymbolKind::Variable,
                        definition_span: var_stmt.name.span,
                        reference_spans: vec![],
                        signature: Some(sig),
                    };
                    self.symbols.push(sym);
                    let idx = self.symbols.len() - 1;
                    self.globals.insert(name, idx);
                }
                _ => {}
            }
        }

        // Initialize scopes stack with the pre-pass globals
        self.scopes = vec![self.globals.clone()];

        // Main pass: Resolve references and walk definitions
        for item in &file.items {
            match item {
                tempra_ast::Item::Function(func) => {
                    self.scopes.push(HashMap::new());
                    for param in &func.parameters {
                        let name = get_token_text(param.name, source).to_string();
                        let sig =
                            format!("let {}: {}", name, get_type_expr_string(&param.ty, source));
                        let sym = SemanticSymbol {
                            name: name.clone(),
                            kind: tempra_diagnostics::SymbolKind::Variable,
                            definition_span: param.name.span,
                            reference_spans: vec![],
                            signature: Some(sig),
                        };
                        self.symbols.push(sym);
                        let idx = self.symbols.len() - 1;
                        self.scopes.last_mut().unwrap().insert(name, idx);
                    }
                    if let Some(ret) = &func.return_type {
                        self.walk_type_expr(ret, source);
                    }
                    for stmt in &func.body.statements {
                        self.walk_statement(stmt, source);
                    }
                    self.scopes.pop();
                }
                tempra_ast::Item::Struct(s) => {
                    for field in &s.fields {
                        self.walk_type_expr(&field.ty, source);
                        if let Some(def) = &field.default_value {
                            self.walk_expression(def, source);
                        }
                    }
                }
                tempra_ast::Item::Enum(e) => {
                    for variant in &e.variants {
                        for payload in &variant.payload_types {
                            self.walk_type_expr(payload, source);
                        }
                    }
                }
                tempra_ast::Item::Interface(itf) => {
                    for method in &itf.methods {
                        self.scopes.push(HashMap::new());
                        for param in &method.parameters {
                            let name = get_token_text(param.name, source).to_string();
                            let sig = format!(
                                "let {}: {}",
                                name,
                                get_type_expr_string(&param.ty, source)
                            );
                            let sym = SemanticSymbol {
                                name: name.clone(),
                                kind: tempra_diagnostics::SymbolKind::Variable,
                                definition_span: param.name.span,
                                reference_spans: vec![],
                                signature: Some(sig),
                            };
                            self.symbols.push(sym);
                            let idx = self.symbols.len() - 1;
                            self.scopes.last_mut().unwrap().insert(name, idx);
                        }
                        if let Some(ret) = &method.return_type {
                            self.walk_type_expr(ret, source);
                        }
                        self.scopes.pop();
                    }
                }
                tempra_ast::Item::Extend(ext) => {
                    self.walk_type_expr(&ext.target, source);
                    for method in &ext.methods {
                        self.scopes.push(HashMap::new());
                        for param in &method.parameters {
                            let name = get_token_text(param.name, source).to_string();
                            let sig = format!(
                                "let {}: {}",
                                name,
                                get_type_expr_string(&param.ty, source)
                            );
                            let sym = SemanticSymbol {
                                name: name.clone(),
                                kind: tempra_diagnostics::SymbolKind::Variable,
                                definition_span: param.name.span,
                                reference_spans: vec![],
                                signature: Some(sig),
                            };
                            self.symbols.push(sym);
                            let idx = self.symbols.len() - 1;
                            self.scopes.last_mut().unwrap().insert(name, idx);
                        }
                        if let Some(ret) = &method.return_type {
                            self.walk_type_expr(ret, source);
                        }
                        for stmt in &method.body.statements {
                            self.walk_statement(stmt, source);
                        }
                        self.scopes.pop();
                    }
                }
                _ => {}
            }
        }

        // Walk top-level statements
        for stmt in &file.top_level_statements {
            self.walk_statement(stmt, source);
        }

        SemanticIndex {
            symbols: self.symbols,
        }
    }

    fn walk_statement(&mut self, stmt: &tempra_ast::Statement, source: &str) {
        match stmt {
            tempra_ast::Statement::Let(let_stmt) => {
                self.walk_expression(&let_stmt.initializer, source);
                if let Some(ty) = &let_stmt.ty {
                    self.walk_type_expr(ty, source);
                }
                let name = get_token_text(let_stmt.name, source).to_string();
                let inferred_ty = if let Some(ty) = &let_stmt.ty {
                    format!(": {}", get_type_expr_string(ty, source))
                } else {
                    match &let_stmt.initializer {
                        tempra_ast::Expression::Integer(_) => ": Int64".to_string(),
                        tempra_ast::Expression::Float(_) => ": Float64".to_string(),
                        tempra_ast::Expression::String(_) => ": String".to_string(),
                        tempra_ast::Expression::Bool(_) => ": Bool".to_string(),
                        _ => "".to_string(),
                    }
                };
                let sig = format!("let {}{}", name, inferred_ty);
                let sym = SemanticSymbol {
                    name: name.clone(),
                    kind: tempra_diagnostics::SymbolKind::Variable,
                    definition_span: let_stmt.name.span,
                    reference_spans: vec![],
                    signature: Some(sig),
                };
                self.symbols.push(sym);
                let idx = self.symbols.len() - 1;
                self.scopes.last_mut().unwrap().insert(name, idx);
            }
            tempra_ast::Statement::Var(var_stmt) => {
                self.walk_expression(&var_stmt.initializer, source);
                if let Some(ty) = &var_stmt.ty {
                    self.walk_type_expr(ty, source);
                }
                let name = get_token_text(var_stmt.name, source).to_string();
                let inferred_ty = if let Some(ty) = &var_stmt.ty {
                    format!(": {}", get_type_expr_string(ty, source))
                } else {
                    match &var_stmt.initializer {
                        tempra_ast::Expression::Integer(_) => ": Int64".to_string(),
                        tempra_ast::Expression::Float(_) => ": Float64".to_string(),
                        tempra_ast::Expression::String(_) => ": String".to_string(),
                        tempra_ast::Expression::Bool(_) => ": Bool".to_string(),
                        _ => "".to_string(),
                    }
                };
                let sig = format!("var {}{}", name, inferred_ty);
                let sym = SemanticSymbol {
                    name: name.clone(),
                    kind: tempra_diagnostics::SymbolKind::Variable,
                    definition_span: var_stmt.name.span,
                    reference_spans: vec![],
                    signature: Some(sig),
                };
                self.symbols.push(sym);
                let idx = self.symbols.len() - 1;
                self.scopes.last_mut().unwrap().insert(name, idx);
            }
            tempra_ast::Statement::Return(ret) => {
                if let Some(val) = &ret.value {
                    self.walk_expression(val, source);
                }
            }
            tempra_ast::Statement::For(for_stmt) => {
                self.walk_expression(&for_stmt.iterable, source);
                if let Some(filter) = &for_stmt.filter {
                    self.walk_expression(filter, source);
                }
                self.scopes.push(HashMap::new());
                let name = get_token_text(for_stmt.binding, source).to_string();
                let sym = SemanticSymbol {
                    name: name.clone(),
                    kind: tempra_diagnostics::SymbolKind::Variable,
                    definition_span: for_stmt.binding.span,
                    reference_spans: vec![],
                    signature: Some(format!("let {}", name)),
                };
                self.symbols.push(sym);
                let idx = self.symbols.len() - 1;
                self.scopes.last_mut().unwrap().insert(name, idx);

                for sub_stmt in &for_stmt.body.statements {
                    self.walk_statement(sub_stmt, source);
                }
                self.scopes.pop();
            }
            tempra_ast::Statement::Expression(expr_stmt) => {
                self.walk_expression(&expr_stmt.expression, source);
            }
            _ => {}
        }
    }

    fn walk_expression(&mut self, expr: &tempra_ast::Expression, source: &str) {
        match expr {
            tempra_ast::Expression::Identifier(id_expr) => {
                let name = get_token_text(id_expr.name, source);
                let mut resolved = false;
                for scope in self.scopes.iter().rev() {
                    if let Some(&idx) = scope.get(name) {
                        self.symbols[idx].reference_spans.push(id_expr.name.span);
                        resolved = true;
                        break;
                    }
                }
                if !resolved {
                    if let Some(&idx) = self.globals.get(name) {
                        self.symbols[idx].reference_spans.push(id_expr.name.span);
                    }
                }
            }
            tempra_ast::Expression::Block(block_expr) => {
                self.scopes.push(HashMap::new());
                for stmt in &block_expr.block.statements {
                    self.walk_statement(stmt, source);
                }
                self.scopes.pop();
            }
            tempra_ast::Expression::If(if_expr) => {
                self.walk_expression(&if_expr.condition, source);
                self.scopes.push(HashMap::new());
                for stmt in &if_expr.then_block.statements {
                    self.walk_statement(stmt, source);
                }
                self.scopes.pop();
                if let Some(else_branch) = &if_expr.else_branch {
                    self.walk_expression(else_branch, source);
                }
            }
            tempra_ast::Expression::Match(match_expr) => {
                self.walk_expression(&match_expr.value, source);
                for arm in &match_expr.arms {
                    self.scopes.push(HashMap::new());
                    self.walk_pattern(&arm.pattern, source);
                    if let Some(guard) = &arm.guard {
                        self.walk_expression(guard, source);
                    }
                    self.walk_expression(&arm.value, source);
                    self.scopes.pop();
                }
            }
            tempra_ast::Expression::For(for_expr) => {
                self.walk_expression(&for_expr.iterable, source);
                self.scopes.push(HashMap::new());
                let name = get_token_text(for_expr.binding, source).to_string();
                let sym = SemanticSymbol {
                    name: name.clone(),
                    kind: tempra_diagnostics::SymbolKind::Variable,
                    definition_span: for_expr.binding.span,
                    reference_spans: vec![],
                    signature: Some(format!("let {}", name)),
                };
                self.symbols.push(sym);
                let idx = self.symbols.len() - 1;
                self.scopes.last_mut().unwrap().insert(name, idx);

                for stmt in &for_expr.body.statements {
                    self.walk_statement(stmt, source);
                }
                self.scopes.pop();
            }
            tempra_ast::Expression::Unary(unary) => {
                self.walk_expression(&unary.operand, source);
            }
            tempra_ast::Expression::Binary(binary) => {
                self.walk_expression(&binary.left, source);
                self.walk_expression(&binary.right, source);
            }
            tempra_ast::Expression::Is(is_expr) => {
                self.walk_expression(&is_expr.value, source);
                self.walk_type_expr(&is_expr.ty, source);
            }
            tempra_ast::Expression::IsPattern(is_pattern) => {
                self.walk_expression(&is_pattern.value, source);
                self.walk_pattern(&is_pattern.pattern, source);
            }
            tempra_ast::Expression::AsOptional(as_opt) => {
                self.walk_expression(&as_opt.expression, source);
                self.walk_type_expr(&as_opt.ty, source);
            }
            tempra_ast::Expression::AsForced(as_force) => {
                self.walk_expression(&as_force.expression, source);
                self.walk_type_expr(&as_force.ty, source);
            }
            tempra_ast::Expression::Try(try_expr) => {
                self.walk_expression(&try_expr.expression, source);
            }
            tempra_ast::Expression::Unwrap(unwrap_expr) => {
                self.walk_expression(&unwrap_expr.expression, source);
            }
            tempra_ast::Expression::Call(call) => {
                self.walk_expression(&call.callee, source);
                if let Some(type_args) = &call.type_args {
                    for ty in type_args {
                        self.walk_type_expr(ty, source);
                    }
                }
                for arg in &call.arguments {
                    match arg {
                        tempra_ast::CallArgument::Positional(pos) => {
                            self.walk_expression(&pos.value, source);
                        }
                        tempra_ast::CallArgument::Named(named) => {
                            let arg_name = get_token_text(named.name, source);
                            if let Some(idx) = self.globals.get(arg_name).copied() {
                                self.symbols[idx].reference_spans.push(named.name.span);
                            }
                            self.walk_expression(&named.value, source);
                        }
                    }
                }
            }
            tempra_ast::Expression::Member(mem) => {
                self.walk_expression(&mem.object, source);
                let member_name = get_token_text(mem.member, source);
                if let Some(idx) = self.globals.get(member_name).copied() {
                    self.symbols[idx].reference_spans.push(mem.member.span);
                }
            }
            tempra_ast::Expression::Index(index_expr) => {
                self.walk_expression(&index_expr.object, source);
                self.walk_expression(&index_expr.index, source);
            }
            tempra_ast::Expression::Slice(slice_expr) => {
                self.walk_expression(&slice_expr.object, source);
                self.walk_expression(&slice_expr.start, source);
                self.walk_expression(&slice_expr.end, source);
            }
            tempra_ast::Expression::Tuple(tuple) => {
                for elem in &tuple.elements {
                    self.walk_expression(elem, source);
                }
            }
            tempra_ast::Expression::Array(arr) => {
                for elem in &arr.elements {
                    self.walk_expression(elem, source);
                }
            }
            tempra_ast::Expression::StructLiteral(lit) => {
                self.walk_type_expr(&lit.ty, source);
                for field in &lit.fields {
                    let field_name = get_token_text(field.name, source);
                    if let Some(idx) = self.globals.get(field_name).copied() {
                        self.symbols[idx].reference_spans.push(field.name.span);
                    }
                    self.walk_expression(&field.value, source);
                }
                if let Some(upd) = &lit.update_from {
                    self.walk_expression(upd, source);
                }
            }
            tempra_ast::Expression::Closure(closure) => {
                self.scopes.push(HashMap::new());
                for param in &closure.parameters {
                    let name = get_token_text(param.name, source).to_string();
                    let sym = SemanticSymbol {
                        name: name.clone(),
                        kind: tempra_diagnostics::SymbolKind::Variable,
                        definition_span: param.name.span,
                        reference_spans: vec![],
                        signature: Some(format!("let {}: ...", name)),
                    };
                    self.symbols.push(sym);
                    let idx = self.symbols.len() - 1;
                    self.scopes.last_mut().unwrap().insert(name, idx);
                }
                if let Some(ret) = &closure.return_type {
                    self.walk_type_expr(ret, source);
                }
                for stmt in &closure.body.statements {
                    self.walk_statement(stmt, source);
                }
                self.scopes.pop();
            }
            tempra_ast::Expression::Command(cmd) => {
                for arg in &cmd.arguments {
                    self.walk_expression(arg, source);
                }
            }
            tempra_ast::Expression::Comptime(comp) => {
                self.walk_expression(&comp.expression, source);
            }
            _ => {}
        }
    }

    fn walk_type_expr(&mut self, ty: &tempra_ast::TypeExpr, source: &str) {
        match ty {
            tempra_ast::TypeExpr::Named(named) => {
                let name = get_token_text(named.name, source);
                if let Some(idx) = self.globals.get(name).copied() {
                    self.symbols[idx].reference_spans.push(named.name.span);
                }
            }
            tempra_ast::TypeExpr::Array(arr) => {
                self.walk_type_expr(&arr.element_type, source);
            }
            tempra_ast::TypeExpr::Tuple(tuple) => {
                for elem in &tuple.elements {
                    self.walk_type_expr(elem, source);
                }
            }
            tempra_ast::TypeExpr::Apply(apply) => {
                self.walk_type_expr(&apply.callee, source);
                for arg in &apply.arguments {
                    self.walk_type_expr(arg, source);
                }
            }
            tempra_ast::TypeExpr::Optional(opt) => {
                self.walk_type_expr(&opt.inner, source);
            }
            tempra_ast::TypeExpr::Result(res) => {
                self.walk_type_expr(&res.ok, source);
                if let Some(err) = &res.err {
                    self.walk_type_expr(err, source);
                }
            }
            tempra_ast::TypeExpr::Function(func) => {
                for param in &func.parameters {
                    self.walk_type_expr(param, source);
                }
                self.walk_type_expr(&func.return_type, source);
            }
            tempra_ast::TypeExpr::Projection(proj) => {
                self.walk_type_expr(&proj.base, source);
                let name = get_token_text(proj.member, source);
                if let Some(idx) = self.globals.get(name).copied() {
                    self.symbols[idx].reference_spans.push(proj.member.span);
                }
            }
            _ => {}
        }
    }

    fn walk_pattern(&mut self, pattern: &tempra_ast::Pattern, source: &str) {
        match pattern {
            tempra_ast::Pattern::Identifier(id_pat) => {
                let name = get_token_text(id_pat.name, source).to_string();
                let sym = SemanticSymbol {
                    name: name.clone(),
                    kind: tempra_diagnostics::SymbolKind::Variable,
                    definition_span: id_pat.name.span,
                    reference_spans: vec![],
                    signature: Some(format!("let {}", name)),
                };
                self.symbols.push(sym);
                let idx = self.symbols.len() - 1;
                self.scopes.last_mut().unwrap().insert(name, idx);
            }
            tempra_ast::Pattern::EnumCase(enum_pat) => {
                let var_name = get_token_text(enum_pat.name, source);
                if let Some(idx) = self.globals.get(var_name).copied() {
                    self.symbols[idx].reference_spans.push(enum_pat.name.span);
                }
                for sub_pat in &enum_pat.arguments {
                    self.walk_pattern(sub_pat, source);
                }
            }
            tempra_ast::Pattern::Struct(struct_pat) => {
                let struct_name = get_token_text(struct_pat.name, source);
                if let Some(idx) = self.globals.get(struct_name).copied() {
                    self.symbols[idx].reference_spans.push(struct_pat.name.span);
                }
                for (_, sub_pat) in &struct_pat.fields {
                    if let Some(sub) = sub_pat {
                        self.walk_pattern(sub, source);
                    }
                }
            }
            tempra_ast::Pattern::Tuple(tuple_pat) => {
                for sub in &tuple_pat.items {
                    self.walk_pattern(sub, source);
                }
            }
            tempra_ast::Pattern::Or(or_pat) => {
                for alt in &or_pat.alternatives {
                    self.walk_pattern(alt, source);
                }
            }
            _ => {}
        }
    }
}
