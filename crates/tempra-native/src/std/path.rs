use std::path::Path;
use tempra_vm::value::VmValue;

pub fn join(args: &[VmValue]) -> Result<VmValue, String> {
    if args.len() != 2 {
        return Err("join expects 2 arguments".to_string());
    }
    let VmValue::String(a) = &args[0] else { return Err("join expects String".to_string()); };
    let VmValue::String(b) = &args[1] else { return Err("join expects String".to_string()); };
    let path = Path::new(a.as_str()).join(b.as_str());
    Ok(VmValue::String(path.to_string_lossy().into_owned().into()))
}

pub fn basename(args: &[VmValue]) -> Result<VmValue, String> {
    if args.len() != 1 {
        return Err("basename expects 1 argument".to_string());
    }
    let VmValue::String(a) = &args[0] else { return Err("basename expects String".to_string()); };
    let base = Path::new(a.as_str()).file_name().unwrap_or_default();
    Ok(VmValue::String(base.to_string_lossy().into_owned().into()))
}
