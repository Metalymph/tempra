use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tempra_ast::ImportPathKind;
use tempra_diagnostics::Diagnostic;
use tempra_hir::{lower_file, HirFile, HirItem};
use tempra_parser::parse_source;
use tempra_span::Span;

pub struct ModuleNode {
    pub name: String,
    pub path: PathBuf,
    pub source: String,
    pub hir: Option<HirFile>,
    pub imports: HashMap<String, String>, // local_alias -> fully_qualified_module_name
}

pub struct ModuleGraph {
    pub root_dir: PathBuf,
    pub modules: HashMap<String, ModuleNode>,
    pub order: Vec<String>, // Topological order (dependencies first)
}

/// A module import could not be mapped to a source path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportResolutionError {
    /// A standard-library import was requested without a resolved stdlib root.
    StandardLibraryUnavailable { import: String },
}

impl std::fmt::Display for ImportResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StandardLibraryUnavailable { import } => {
                write!(
                    f,
                    "standard library could not be located for import '{import}'"
                )
            }
        }
    }
}

impl std::error::Error for ImportResolutionError {}

impl Default for ModuleGraph {
    fn default() -> Self {
        Self {
            root_dir: PathBuf::new(),
            modules: HashMap::new(),
            order: Vec::new(),
        }
    }
}

pub fn resolve_import_path(
    root_dir: &Path,
    importer_path: &Path,
    segments: &[String],
    kind: ImportPathKind,
    stdlib_root: Option<&Path>,
) -> Result<PathBuf, ImportResolutionError> {
    match kind {
        ImportPathKind::Relative => {
            let parent = importer_path.parent().unwrap_or(root_dir);
            let mut resolved = parent.to_path_buf();
            for (i, seg) in segments.iter().enumerate() {
                if i == segments.len() - 1 {
                    resolved.push(format!("{}.tmpr", seg));
                } else {
                    resolved.push(seg);
                }
            }
            Ok(resolved)
        }
        ImportPathKind::Package => {
            let mut resolved = root_dir.to_path_buf();
            for (i, seg) in segments.iter().enumerate() {
                if i == segments.len() - 1 {
                    resolved.push(format!("{}.tmpr", seg));
                } else {
                    resolved.push(seg);
                }
            }
            Ok(resolved)
        }
        ImportPathKind::StandardLibrary => {
            let mut resolved = stdlib_root
                .ok_or_else(|| ImportResolutionError::StandardLibraryUnavailable {
                    import: segments.join("."),
                })?
                .to_path_buf();
            for (i, seg) in segments.iter().enumerate().skip(1) {
                if i == segments.len() - 1 {
                    resolved.push(format!("{}.tmpr", seg));
                } else {
                    resolved.push(seg);
                }
            }
            Ok(resolved)
        }
    }
}

pub fn get_module_name(root_dir: &Path, file_path: &Path) -> String {
    if let Ok(rel) = file_path.strip_prefix(root_dir) {
        let mut components = Vec::new();
        for component in rel.components() {
            let s = component.as_os_str().to_string_lossy().into_owned();
            components.push(s);
        }
        if let Some(last) = components.last_mut() {
            if last.ends_with(".tmpr") {
                *last = last[..last.len() - 5].to_string();
            }
        }
        components.join(".")
    } else {
        file_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }
}

pub trait DocumentLoader {
    fn read_to_string(&self, path: &Path) -> std::io::Result<String>;
}

pub struct DiskDocumentLoader;
impl DocumentLoader for DiskDocumentLoader {
    fn read_to_string(&self, path: &Path) -> std::io::Result<String> {
        std::fs::read_to_string(path)
    }
}

pub fn load_module_graph(
    entry_path: &Path,
    loader: &dyn DocumentLoader,
    stdlib_root: Option<&Path>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<ModuleGraph, ()> {
    let canonical_entry = entry_path
        .canonicalize()
        .unwrap_or_else(|_| entry_path.to_path_buf());
    let root_dir = canonical_entry
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    let mut graph = ModuleGraph {
        root_dir: root_dir.clone(),
        modules: HashMap::new(),
        order: Vec::new(),
    };

    let mut visiting = Vec::new();
    let mut visiting_set = HashSet::new();

    let entry_name = get_module_name(&root_dir, &canonical_entry);

    load_module_recursive(
        entry_name,
        canonical_entry,
        &root_dir,
        stdlib_root,
        loader,
        &mut visiting,
        &mut visiting_set,
        &mut graph,
        diagnostics,
    )?;

    Ok(graph)
}

fn load_module_recursive(
    name: String,
    path: PathBuf,
    root_dir: &Path,
    stdlib_root: Option<&Path>,
    loader: &dyn DocumentLoader,
    visiting: &mut Vec<(String, Span)>,
    visiting_set: &mut HashSet<String>,
    graph: &mut ModuleGraph,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), ()> {
    if graph.modules.contains_key(&name) {
        return Ok(());
    }

    // Read and parse
    let source = match loader.read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            let span = visiting.last().map(|(_, s)| *s).unwrap_or(Span::new(0, 0));
            diagnostics.push(
                Diagnostic::error(
                    span,
                    format!(
                        "could not read module file '{}' (path: {}): {}",
                        name,
                        path.display(),
                        e
                    ),
                )
                .with_file_path(path.to_string_lossy().to_string()),
            );
            return Err(());
        }
    };

    let ast = match parse_source(&source) {
        Ok(a) => a,
        Err(e) => {
            diagnostics.push(
                Diagnostic::error(e.span, e.message)
                    .with_file_path(path.to_string_lossy().to_string()),
            );
            return Err(());
        }
    };

    let hir = lower_file(&source, &ast);

    let mut imports = HashMap::new();

    // Push self onto visiting stack
    let dummy_span = Span::new(0, 0);
    visiting.push((name.clone(), dummy_span));
    visiting_set.insert(name.clone());

    let mut load_errors = false;

    // First, resolve imports and detect cycles
    for item in &hir.items {
        if let HirItem::Import(imp) = item {
            let local_alias = if imp.path.is_empty() {
                diagnostics.push(
                    Diagnostic::error(imp.span, "empty import path")
                        .with_file_path(path.to_string_lossy().to_string()),
                );
                load_errors = true;
                continue;
            } else {
                imp.path.join(".")
            };

            let resolved_path =
                match resolve_import_path(root_dir, &path, &imp.path, imp.kind, stdlib_root) {
                    Ok(path) => path,
                    Err(error) => {
                        diagnostics.push(
                            Diagnostic::error(imp.span, error.to_string())
                                .with_file_path(path.to_string_lossy().to_string()),
                        );
                        load_errors = true;
                        continue;
                    }
                };
            let dep_name = match imp.kind {
                ImportPathKind::Relative => get_module_name(root_dir, &resolved_path),
                _ => imp.path.join("."),
            };

            imports.insert(local_alias, dep_name.clone());

            // Check if already visiting (cycle!)
            if visiting_set.contains(&dep_name) {
                let cycle_path: Vec<String> = visiting.iter().map(|(n, _)| n.clone()).collect();
                let cycle_str = format!("{} -> {}", cycle_path.join(" -> "), dep_name);
                diagnostics.push(
                    Diagnostic::error(imp.span, format!("cyclic import detected: {}", cycle_str))
                        .with_file_path(path.to_string_lossy().to_string()),
                );
                load_errors = true;
                continue;
            }

            // Update the span in the visiting stack for the dependency
            if let Some(last) = visiting.last_mut() {
                last.1 = imp.span;
            }

            // Recursively load the dependency
            if load_module_recursive(
                dep_name.clone(),
                resolved_path,
                root_dir,
                stdlib_root,
                loader,
                visiting,
                visiting_set,
                graph,
                diagnostics,
            )
            .is_err()
            {
                load_errors = true;
            }
        }
    }

    visiting.pop();
    visiting_set.remove(&name);

    if load_errors {
        return Err(());
    }

    graph.modules.insert(
        name.clone(),
        ModuleNode {
            name: name.clone(),
            path,
            source,
            hir: Some(hir),
            imports,
        },
    );
    graph.order.push(name);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segments(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_string()).collect()
    }

    #[test]
    fn standard_library_import_uses_only_the_supplied_root() {
        let root = Path::new("/toolchain/std");
        let resolved = resolve_import_path(
            Path::new("/project"),
            Path::new("/project/main.tmpr"),
            &segments(&["std", "path"]),
            ImportPathKind::StandardLibrary,
            Some(root),
        )
        .unwrap();

        assert_eq!(resolved, Path::new("/toolchain/std/path.tmpr"));
    }

    #[test]
    fn missing_standard_library_root_is_a_typed_error() {
        let error = resolve_import_path(
            Path::new("/project"),
            Path::new("/project/main.tmpr"),
            &segments(&["std", "path"]),
            ImportPathKind::StandardLibrary,
            None,
        )
        .unwrap_err();

        assert_eq!(
            error,
            ImportResolutionError::StandardLibraryUnavailable {
                import: "std.path".to_string(),
            }
        );
        assert_eq!(
            error.to_string(),
            "standard library could not be located for import 'std.path'"
        );
    }

    #[test]
    fn local_import_resolution_is_unchanged() {
        let resolved = resolve_import_path(
            Path::new("/project"),
            Path::new("/project/app/main.tmpr"),
            &segments(&["helper"]),
            ImportPathKind::Relative,
            None,
        )
        .unwrap();

        assert_eq!(resolved, Path::new("/project/app/helper.tmpr"));
    }

    #[test]
    fn module_graph_order_preserves_source_import_order_before_the_entry() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("main.tmpr"),
            "import alpha\nimport beta\nfn main() {}\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("alpha.tmpr"), "interface Alpha {}\n").unwrap();
        std::fs::write(dir.path().join("beta.tmpr"), "interface Beta {}\n").unwrap();

        let mut diagnostics = Vec::new();
        let graph = load_module_graph(
            &dir.path().join("main.tmpr"),
            &DiskDocumentLoader,
            None,
            &mut diagnostics,
        )
        .unwrap();

        assert!(diagnostics.is_empty());
        assert_eq!(graph.order, vec!["alpha", "beta", "main"]);
    }
}
