use std::path::PathBuf;

pub fn run_command(path: PathBuf, args: Vec<String>, experimental_vm: bool, experimental_jit: bool) -> Result<(), String> {
    // Validate: --experimental-jit requires --experimental-vm
    if experimental_jit && !experimental_vm {
        return Err("--experimental-jit requires --experimental-vm".into());
    }

    // Validate: JIT requires the jit-cranelift feature to be compiled in
    if experimental_jit && !cfg!(feature = "jit-cranelift") {
        return Err("JIT support not compiled into this build. Rebuild with --features jit-cranelift".into());
    }

    let result = if experimental_vm {
        tempra_driver::run_file_with_experimental_vm_jit(&path, &args, experimental_jit)
    } else {
        tempra_driver::run_file_with_args(&path, &args)
    };

    match result {
        Ok(_) => Ok(()),
        Err(tempra_driver::DriverError::Exit(code)) => std::process::exit(code),
        Err(e) => Err(format!("{}", e)),
    }
}
