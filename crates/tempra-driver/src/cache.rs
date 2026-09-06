use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use tempra_bytecode::serialize::{decode_and_verify_artifact, encode_artifact};
use tempra_bytecode::BytecodeCacheArtifact;

// Versions guarantees required for artifact reuse, independently of bytecode format.
// Increment when previously compiled artifacts no longer certify current semantics.
const SEMANTIC_REVISION: u32 = 3;

pub(crate) fn compiler_fingerprint() -> String {
    format!(
        "tempra-{}-semantics-{SEMANTIC_REVISION}",
        env!("CARGO_PKG_VERSION")
    )
}

// A cache hit is bytecode-only, not an analysis-producing compilation.
pub(crate) fn read_compatible_module(
    path: &Path,
    source_hash: &[u8; 32],
) -> Option<tempra_bytecode::BytecodeModule> {
    let artifact = read_cache_artifact(path).ok()?;
    (artifact.header.compiler_fingerprint == compiler_fingerprint()
        && artifact.header.source_hash == *source_hash)
        .then_some(artifact.module)
}

/// Computes a v0 non-cryptographic source hash.
/// This is not stable across compiler/runtime versions.
/// It is only a cache key hint, not an integrity/security primitive.
pub fn compute_source_hash(source: &str) -> [u8; 32] {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    let hash = hasher.finish();

    let mut key = [0u8; 32];
    let bytes = hash.to_le_bytes();
    for i in 0..4 {
        key[i * 8..(i + 1) * 8].copy_from_slice(&bytes);
    }
    key
}

/// Converts a 32-byte hash to a simple hex string for filenames.
pub fn hash_to_hex(hash: &[u8; 32]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(64);
    for b in hash {
        write!(&mut s, "{:02x}", b).unwrap();
    }
    s
}

pub fn get_cache_path(hash: &[u8; 32]) -> PathBuf {
    let hex_str = hash_to_hex(hash);
    Path::new(".tempra")
        .join("cache")
        .join(format!("{}.tbc", hex_str))
}

pub fn read_cache_artifact(path: &Path) -> Result<BytecodeCacheArtifact, String> {
    let bytes = fs::read(path).map_err(|e| format!("IO error: {}", e))?;
    decode_and_verify_artifact(&bytes).map_err(|e| format!("Decode/verify error: {}", e))
}

pub fn write_cache_artifact(path: &Path, artifact: &BytecodeCacheArtifact) -> Result<(), String> {
    let bytes = encode_artifact(artifact).map_err(|e| format!("Encode error: {}", e))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create cache dir: {}", e))?;
    }
    fs::write(path, bytes).map_err(|e| format!("Failed to write cache: {}", e))
}
