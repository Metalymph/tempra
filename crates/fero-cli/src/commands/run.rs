use std::path::PathBuf;

pub fn run_command(path: PathBuf) -> Result<(), String> {
    fero_driver::run_file(&path)
        .map_err(|e| format!("{:?}", e))
}
