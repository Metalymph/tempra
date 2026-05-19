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
) -> PathBuf {
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
            resolved
        }
        ImportPathKind::Package | ImportPathKind::StandardLibrary => {
            let mut resolved = root_dir.to_path_buf();
            for (i, seg) in segments.iter().enumerate() {
                if i == segments.len() - 1 {
                    resolved.push(format!("{}.tmpr", seg));
                } else {
                    resolved.push(seg);
                }
            }
            resolved
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
            diagnostics.push(Diagnostic::error(
                span,
                format!(
                    "could not read module file '{}' (path: {}): {}",
                    name,
                    path.display(),
                    e
                ),
            ));
            return Err(());
        }
    };

    let ast = match parse_source(&source) {
        Ok(a) => a,
        Err(e) => {
            diagnostics.push(Diagnostic::error(e.span, e.message));
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
            let local_alias = match imp.kind {
                tempra_ast::ImportPathKind::Relative => match imp.path.last() {
                    Some(alias) => alias.clone(),
                    None => {
                        diagnostics.push(Diagnostic::error(imp.span, "empty import path"));
                        load_errors = true;
                        continue;
                    }
                },
                _ => imp.path.join("."),
            };

            let resolved_path = resolve_import_path(root_dir, &path, &imp.path, imp.kind);
            let dep_name = get_module_name(root_dir, &resolved_path);

            imports.insert(local_alias, dep_name.clone());

            // Check if already visiting (cycle!)
            if visiting_set.contains(&dep_name) {
                let cycle_path: Vec<String> = visiting.iter().map(|(n, _)| n.clone()).collect();
                let cycle_str = format!("{} -> {}", cycle_path.join(" -> "), dep_name);
                diagnostics.push(Diagnostic::error(
                    imp.span,
                    format!("cyclic import detected: {}", cycle_str),
                ));
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
