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

/// Returns `Result<String, Error>` as `VmValue::Enum("Result", "Ok", [String])`
/// or `VmValue::Enum("Result", "Err", [String])` on failure.
pub fn read_text(args: &[VmValue]) -> Result<VmValue, String> {
    if args.len() != 1 {
        return Err("read_text expects 1 argument".to_string());
    }
    let VmValue::String(path_val) = &args[0] else {
        return Err("read_text expects a String path".to_string());
    };
    match std::fs::read_to_string(path_val.as_str()) {
        Ok(content) => Ok(VmValue::Enum(
            "Result".to_string(),
            "Ok".to_string(),
            vec![VmValue::String(content)],
        )),
        Err(e) => Ok(VmValue::Enum(
            "Result".to_string(),
            "Err".to_string(),
            vec![VmValue::String(e.to_string())],
        )),
    }
}

/// Returns `Result<Unit, Error>` as `VmValue::Enum("Result", "Ok", [Unit])`
/// or `VmValue::Enum("Result", "Err", [String])` on failure.
pub fn write_text(args: &[VmValue]) -> Result<VmValue, String> {
    if args.len() != 2 {
        return Err("write_text expects 2 arguments".to_string());
    }
    let VmValue::String(path_val) = &args[0] else {
        return Err("write_text expects a String path as first argument".to_string());
    };
    let VmValue::String(text_val) = &args[1] else {
        return Err("write_text expects a String text as second argument".to_string());
    };
    match std::fs::write(path_val.as_str(), text_val.as_str()) {
        Ok(()) => Ok(VmValue::Enum(
            "Result".to_string(),
            "Ok".to_string(),
            vec![VmValue::Unit],
        )),
        Err(e) => Ok(VmValue::Enum(
            "Result".to_string(),
            "Err".to_string(),
            vec![VmValue::String(e.to_string())],
        )),
    }
}
