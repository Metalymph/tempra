use std::path::PathBuf;

pub fn run_command(path: PathBuf) -> Result<(), String> {
    match fero_driver::run_file(&path) {
        Ok(_) => Ok(()),
        Err(fero_driver::DriverError::Exit(code)) => std::process::exit(code),
        Err(e) => Err(format!("{:?}", e)),
    }
}
