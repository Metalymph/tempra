use std::path::PathBuf;

pub fn check_command(path: PathBuf) -> Result<(), String> {
    fero_driver::check_file(&path)
        .map_err(|e| format!("{}", e))
}
