use std::path::Path;
use tempra_vm::value::VmValue;

pub fn exists(args: &[VmValue]) -> Result<VmValue, String> {
    if args.len() != 1 {
        return Err("exists expects 1 argument".to_string());
    }
    let VmValue::String(path_val) = &args[0] else {
        return Err("exists expects a String".to_string());
    };
    let path = Path::new(path_val.as_str());
    Ok(VmValue::Bool(path.exists()))
}
