# Fero Pipeline

## Overview

Fero keeps one canonical compilation path and multiple execution or emission targets.

```text
source
  -> lexer
  -> parser
  -> AST
  -> HIR
  -> typeck
  -> MIR
  -> Fero IR
  -> interpreter
  -> wasm backend
  -> cranelift backend
```

## Phase Intent

- `lexer`: token boundaries, trivia handling, and lexical diagnostics
- `parser`: syntax tree construction and syntax recovery
- `AST`: user-shaped syntax representation
- `HIR`: normalized semantic program structure
- `typeck`: interface validation, inference boundaries, explicit errors
- `MIR`: control-flow and value lifetime planning
- `Fero IR`: backend-facing, target-neutral representation

## Execution Modes

### Script mode

- source file is loaded directly
- no mandatory `main`
- interpreter is the default target

### Package mode

- package metadata selects entrypoint and output configuration
- `main` is required for apps
- build target selects native or Wasm output

## Backend Split

### Native

- `fero-backend-cranelift` consumes Fero IR
- `fero-linker` plans system linker invocation
- `--mode fast` and `--mode light` influence codegen priorities

### Wasm

- `fero-backend-wasm` consumes Fero IR directly
- no Cranelift dependency in the Wasm emission path
- future WASI-focused packaging can be layered in `fero-package`

### Wasm Hosting

- `fero-wasm-host` will host generic Wasm modules
- `fero-component-host` will host Wasm components
- both are runtime integration layers, not compiler backends
