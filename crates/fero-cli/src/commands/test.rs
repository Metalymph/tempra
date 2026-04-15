use std::path::PathBuf;

pub fn test_command(path: PathBuf) -> Result<(), String> {
    fero_driver::pipeline::test_file(&path).map_err(|e| format!("{:?}", e))
}
