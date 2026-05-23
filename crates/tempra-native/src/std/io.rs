use tempra_vm::value::VmValue;

pub fn print(args: &[VmValue]) -> Result<VmValue, String> {
    if args.len() != 1 {
        return Err("print expects 1 argument".to_string());
    }
    let VmValue::String(text) = &args[0] else { return Err("expected String".into()); };
    print!("{}", text.as_str());
    Ok(VmValue::Unit)
}

pub fn println(args: &[VmValue]) -> Result<VmValue, String> {
    if args.len() != 1 {
        return Err("println expects 1 argument".to_string());
    }
    let VmValue::String(text) = &args[0] else { return Err("expected String".into()); };
    println!("{}", text.as_str());
    Ok(VmValue::Unit)
}
