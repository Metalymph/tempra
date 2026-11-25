use std::path::PathBuf;

pub fn run_command(path: PathBuf, args: Vec<String>, experimental_vm: bool) -> Result<(), String> {
    let result = if experimental_vm {
        tempra_driver::run_file_with_experimental_vm(&path, &args)
    } else {
        tempra_driver::run_file_with_args(&path, &args)
    };

    match result {
        Ok(_) => Ok(()),
        Err(tempra_driver::DriverError::Exit(code)) => std::process::exit(code),
        Err(e) => Err(format!("{}", e)),
    }
}
