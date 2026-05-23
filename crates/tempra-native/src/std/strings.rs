use tempra_vm::value::VmValue;

pub fn contains(args: &[VmValue]) -> Result<VmValue, String> {
    if args.len() != 2 {
        return Err("contains expects 2 arguments".to_string());
    }
    let VmValue::String(text) = &args[0] else { return Err("expected String".into()); };
    let VmValue::String(needle) = &args[1] else { return Err("expected String".into()); };
    Ok(VmValue::Bool(text.as_str().contains(needle.as_str())))
}

pub fn starts_with(args: &[VmValue]) -> Result<VmValue, String> {
    if args.len() != 2 {
        return Err("starts_with expects 2 arguments".to_string());
    }
    let VmValue::String(text) = &args[0] else { return Err("expected String".into()); };
    let VmValue::String(prefix) = &args[1] else { return Err("expected String".into()); };
    Ok(VmValue::Bool(text.as_str().starts_with(prefix.as_str())))
}

pub fn ends_with(args: &[VmValue]) -> Result<VmValue, String> {
    if args.len() != 2 {
        return Err("ends_with expects 2 arguments".to_string());
    }
    let VmValue::String(text) = &args[0] else { return Err("expected String".into()); };
    let VmValue::String(suffix) = &args[1] else { return Err("expected String".into()); };
    Ok(VmValue::Bool(text.as_str().ends_with(suffix.as_str())))
}
