use std::path::PathBuf;

pub fn emit_tir_command(path: PathBuf) -> Result<(), String> {
    let output = tempra_driver::emit_tir_file(&path).map_err(|e| format!("{}", e))?;
    print!("{}", output);
    Ok(())
}
