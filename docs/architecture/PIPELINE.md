# Tempra Pipeline

## Overview

Tempra keeps one canonical compilation path and multiple execution or emission targets.

```text
source
  -> lexer
  -> parser
  -> AST
  -> HIR
  -> typeck
  -> MIR
  -> Tempra IR
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
- `Tempra IR`: backend-facing, target-neutral representation

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

- `tempra-backend-cranelift` consumes Tempra IR
- `tempra-linker` plans system linker invocation
- `--mode fast` and `--mode light` influence codegen priorities

### Wasm

- `tempra-backend-wasm` consumes Tempra IR directly
- no Cranelift dependency in the Wasm emission path
- future WASI-focused packaging can be layered in `tempra-package`

### Wasm Hosting

- `tempra-wasm-host` will host generic Wasm modules
- `tempra-component-host` will host Wasm components
- both are runtime integration layers, not compiler backends
