# justfile
# Tempra workspace automation

set shell := ["bash", "-cu"]

default:
    @just --list

# --------------------------------------------------
# HELP
# --------------------------------------------------

help:
    @just --list

# --------------------------------------------------
# CHECK
# --------------------------------------------------

# Check entire workspace
check:
    cargo check --workspace

# Check single crate
# usage:
# just check-crate tempra-parser
check-crate crate:
    cargo check -p {{crate}}

# --------------------------------------------------
# BUILD
# --------------------------------------------------

# Dev build workspace
build:
    cargo build --workspace

# Release build workspace
build-release:
    cargo build --workspace --release

# Build single crate
# usage:
# just build-crate tempra-parser
build-crate crate:
    cargo build -p {{crate}}

# Build single crate release
# usage:
# just build-crate-release tempra-parser
build-crate-release crate:
    cargo build -p {{crate}} --release

# --------------------------------------------------
# TEST
# --------------------------------------------------

# Test all workspace
test:
    cargo test --workspace

# Test single crate
# usage:
# just test-crate tempra-parser
test-crate crate:
    cargo test -p {{crate}}

# Test filtered name in workspace
# usage:
# just test-filter parser
test-filter filter:
    cargo test {{filter}}

# Test filtered name in crate
# usage:
# just test-crate-filter tempra-parser lexer
test-crate-filter crate filter:
    cargo test -p {{crate}} {{filter}}

# --------------------------------------------------
# CLEANUP
# --------------------------------------------------

# Hard clean
clean:
    cargo clean

# Sweep old build artifacts
sweep:
    cargo sweep --time 30

# Cleanup cargo cache
cache-clean:
    cargo cache --autoclean

# Full cleanup
cleanup:
    cargo sweep --time 30
    cargo cache --autoclean

# --------------------------------------------------
# SIZE / BLOAT ANALYSIS
# --------------------------------------------------

# Build optimized release
size-build:
    cargo build --release

# Binary bloat analysis
# usage:
# just bloat tempra-cli
bloat crate="tempra-cli":
    cargo bloat --release -p {{crate}}

# Target directory usage
target-size:
    du -h -d 1 target | sort -h

# Full size analysis
analyze-size crate="tempra-cli":
    cargo build --release
    cargo bloat --release -p {{crate}}
    du -h -d 1 target | sort -h

# --------------------------------------------------
# FORMAT / LINT
# --------------------------------------------------

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# --------------------------------------------------
# CI STYLE
# --------------------------------------------------

ci:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    cargo check --workspace

# --------------------------------------------------
# QUICK DEV
# --------------------------------------------------

dev:
    cargo check --workspace
    cargo test --workspace

# --------------------------------------------------
# TOOL INSTALL
# --------------------------------------------------

install-tools:
    cargo install cargo-sweep
    cargo install cargo-cache
    cargo install cargo-bloat
# --------------------------------------------------
# PERFORMANCE
# --------------------------------------------------

# Run VM performance baseline
perf-vm:
    cargo test -p tempra-driver --test perf_baseline -- --ignored --nocapture
