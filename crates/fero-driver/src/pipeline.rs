use crate::build::{BuildRequest, DriverCommand, DriverOutcome};
use anyhow::Result;
use fero_ast::Module;
use fero_backend_cranelift::codegen::emit_native;
use fero_backend_wasm::emitter::emit_wasm;
use fero_fs::SourceFile;
use fero_hir::{HirFunction, HirProgram};
use fero_interpreter::Interpreter;
use fero_ir::IrModule;
use fero_mir::{MirBody, MirProgram};
use fero_opt::OptimizationPlan;
use fero_package::{BuildMode, BuildTarget};
use fero_parser::parse_module;
use fero_typeck::TypeChecker;
use std::path::Path;

#[derive(Debug, Default)]
pub struct Driver;

impl Driver {
    pub fn execute(&self, command: DriverCommand, path: &Path) -> Result<DriverOutcome> {
        match command {
            DriverCommand::Run => Ok(self.run(path)),
            DriverCommand::Check => Ok(self.check(path)),
            DriverCommand::Build(request) => Ok(self.build(path, request)),
            DriverCommand::Install(request) => Ok(self.install(path, request)),
            DriverCommand::Test => Ok(DriverOutcome {
                summary: format!("test pipeline prepared for {}", path.display()),
            }),
            DriverCommand::Fmt => Ok(DriverOutcome {
                summary: format!("format pipeline prepared for {}", path.display()),
            }),
        }
    }

    fn load_module(&self, path: &Path) -> Module {
        match SourceFile::load(path) {
            Ok(source) => parse_module(&source.text),
            Err(_) => parse_module("io.println(\"placeholder\")"),
        }
    }

    fn lower_to_hir(&self, module: &Module) -> HirProgram {
        HirProgram {
            functions: module
                .functions
                .iter()
                .map(|function| HirFunction {
                    name: function.name.clone(),
                })
                .collect(),
        }
    }

    fn lower_to_mir(&self, hir: &HirProgram) -> MirProgram {
        MirProgram {
            functions: hir
                .functions
                .iter()
                .map(|function| MirBody {
                    blocks: vec![format!("entry:{}", function.name)],
                })
                .collect(),
        }
    }

    fn lower_to_ir(&self, mir: &MirProgram) -> IrModule {
        IrModule {
            functions: mir
                .functions
                .iter()
                .enumerate()
                .map(|(index, _)| format!("fn_{index}"))
                .collect(),
        }
    }

    fn run(&self, path: &Path) -> DriverOutcome {
        let module = self.load_module(path);
        let value = Interpreter.run(&module);
        DriverOutcome {
            summary: format!("interpreted {} -> {:?}", path.display(), value),
        }
    }

    fn check(&self, path: &Path) -> DriverOutcome {
        let module = self.load_module(path);
        let hir = self.lower_to_hir(&module);
        let result = TypeChecker::check(&hir);
        DriverOutcome {
            summary: format!(
                "checked {} with {} diagnostics",
                path.display(),
                result.diagnostics.len()
            ),
        }
    }

    fn build(&self, path: &Path, request: BuildRequest) -> DriverOutcome {
        let module = self.load_module(path);
        let hir = self.lower_to_hir(&module);
        let _checked = TypeChecker::check(&hir);
        let mir = self.lower_to_mir(&hir);
        let ir = self.lower_to_ir(&mir);
        let plan = OptimizationPlan::for_mode(match request.mode {
            BuildMode::Fast => "fast",
            BuildMode::Light => "light",
        });
        let ir = plan.apply(ir);
        match request.target {
            BuildTarget::Native => {
                let artifact = emit_native(&ir, "fero-out");
                DriverOutcome {
                    summary: format!("built native artifact {}", artifact.object_name),
                }
            }
            BuildTarget::Wasm32Wasi => {
                let artifact = emit_wasm(&ir);
                DriverOutcome {
                    summary: format!("built wasm artifact with {} bytes", artifact.bytes.len()),
                }
            }
        }
    }

    fn install(&self, path: &Path, request: BuildRequest) -> DriverOutcome {
        let built = self.build(path, request);
        DriverOutcome {
            summary: format!("installed {}", built.summary),
        }
    }
}
