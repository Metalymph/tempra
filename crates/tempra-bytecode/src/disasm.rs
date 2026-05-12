use crate::bytecode::BytecodeModule;
use crate::function::FunctionId;
use crate::error::BytecodeError;
use crate::opcode::BytecodeInstr;

pub fn disassemble_module(module: &BytecodeModule) -> String {
    let mut out = String::new();
    out.push_str(&format!("Module v{}\n", module.version));
    
    out.push_str("\nConstants:\n");
    for (i, c) in module.constants.iter().enumerate() {
        out.push_str(&format!("  #{}: {}\n", i, c));
    }

    out.push_str("\nFunctions:\n");
    for (i, _) in module.functions.iter().enumerate() {
        if let Ok(s) = disassemble_function(module, FunctionId(i as u32)) {
            out.push_str(&s);
            out.push_str("\n");
        }
    }

    out
}

pub fn disassemble_function(module: &BytecodeModule, func_id: FunctionId) -> Result<String, BytecodeError> {
    let func = module.functions.get(func_id.0 as usize)
        .ok_or(BytecodeError::InvalidFunctionId(func_id))?;

    let mut out = String::new();
    out.push_str(&format!("fn {} (", func.name));
    for (i, p) in func.params.iter().enumerate() {
        if i > 0 { out.push_str(", "); }
        out.push_str(&format!("{}", p));
    }
    out.push_str(&format!(") -> {}\n", func.ret));
    out.push_str(&format!("  max_stack: {}\n", func.max_stack));
    out.push_str("  locals:\n");
    for (i, l) in func.locals.iter().enumerate() {
        out.push_str(&format!("    ${}: {}\n", i, l));
    }
    out.push_str("  code:\n");
    for (i, instr) in func.code.iter().enumerate() {
        out.push_str(&format!("    {:4}: ", i));
        match instr {
            BytecodeInstr::ConstUnit => out.push_str("ConstUnit\n"),
            BytecodeInstr::ConstBool(b) => out.push_str(&format!("ConstBool {}\n", b)),
            BytecodeInstr::ConstI64(id) => out.push_str(&format!("ConstI64 #{}\n", id.0)),
            BytecodeInstr::ConstU64(id) => out.push_str(&format!("ConstU64 #{}\n", id.0)),
            BytecodeInstr::ConstF64(id) => out.push_str(&format!("ConstF64 #{}\n", id.0)),
            BytecodeInstr::ConstString(id) => out.push_str(&format!("ConstString #{}\n", id.0)),
            BytecodeInstr::LoadLocal(id) => out.push_str(&format!("LoadLocal ${}\n", id.0)),
            BytecodeInstr::StoreLocal(id) => out.push_str(&format!("StoreLocal ${}\n", id.0)),
            BytecodeInstr::Pop => out.push_str("Pop\n"),
            BytecodeInstr::Dup => out.push_str("Dup\n"),
            BytecodeInstr::AddI64 => out.push_str("AddI64\n"),
            BytecodeInstr::SubI64 => out.push_str("SubI64\n"),
            BytecodeInstr::MulI64 => out.push_str("MulI64\n"),
            BytecodeInstr::DivI64 => out.push_str("DivI64\n"),
            BytecodeInstr::AddU64 => out.push_str("AddU64\n"),
            BytecodeInstr::SubU64 => out.push_str("SubU64\n"),
            BytecodeInstr::MulU64 => out.push_str("MulU64\n"),
            BytecodeInstr::DivU64 => out.push_str("DivU64\n"),
            BytecodeInstr::AddF64 => out.push_str("AddF64\n"),
            BytecodeInstr::SubF64 => out.push_str("SubF64\n"),
            BytecodeInstr::MulF64 => out.push_str("MulF64\n"),
            BytecodeInstr::DivF64 => out.push_str("DivF64\n"),
            BytecodeInstr::Eq => out.push_str("Eq\n"),
            BytecodeInstr::LtI64 => out.push_str("LtI64\n"),
            BytecodeInstr::GtI64 => out.push_str("GtI64\n"),
            BytecodeInstr::LtU64 => out.push_str("LtU64\n"),
            BytecodeInstr::GtU64 => out.push_str("GtU64\n"),
            BytecodeInstr::LtF64 => out.push_str("LtF64\n"),
            BytecodeInstr::GtF64 => out.push_str("GtF64\n"),
            BytecodeInstr::Not => out.push_str("Not\n"),
            BytecodeInstr::And => out.push_str("And\n"),
            BytecodeInstr::Or => out.push_str("Or\n"),
            BytecodeInstr::Jump(off) => out.push_str(&format!("Jump {}\n", off.0)),
            BytecodeInstr::JumpIfFalse(off) => out.push_str(&format!("JumpIfFalse {}\n", off.0)),
            BytecodeInstr::Return => out.push_str("Return\n"),
            BytecodeInstr::Unreachable => out.push_str("Unreachable\n"),
            BytecodeInstr::Call(id, argc) => {
                let target = module.functions.get(id.0 as usize);
                let name = target.map(|f| f.name.as_str()).unwrap_or("unknown");
                out.push_str(&format!("Call @{} ({}) // {}\n", id.0, argc, name));
            }
            BytecodeInstr::MakeEnum(enum_id, variant_id, count) => {
                out.push_str(&format!("MakeEnum #{} #{} count={}\n", enum_id.0, variant_id.0, count));
            }
            BytecodeInstr::Tag => out.push_str("Tag\n"),
            BytecodeInstr::Unwrap(variant_id, idx) => {
                out.push_str(&format!("Unwrap #{} idx={}\n", variant_id.0, idx));
            }
            BytecodeInstr::MakeArray(count) => out.push_str(&format!("MakeArray {}\n", count)),
            BytecodeInstr::GetIndex => out.push_str("GetIndex\n"),
            BytecodeInstr::Slice => out.push_str("Slice\n"),
            BytecodeInstr::Len => out.push_str("Len\n"),
            BytecodeInstr::Size => out.push_str("Size\n"),
            BytecodeInstr::Bytes => out.push_str("Bytes\n"),
            BytecodeInstr::ArrayPush => out.push_str("ArrayPush\n"),
            BytecodeInstr::MakeStruct(name, fields, count) => {
                out.push_str(&format!("MakeStruct #{} #{} count={}\n", name.0, fields.0, count));
            }
            BytecodeInstr::GetField(name) => {
                out.push_str(&format!("GetField #{}\n", name.0));
            }
            BytecodeInstr::SetField(name) => {
                out.push_str(&format!("SetField #{}\n", name.0));
            }
            BytecodeInstr::AddI64LL(a, b) => {
                out.push_str(&format!("AddI64LL ${} ${}\n", a.0, b.0));
            }
            BytecodeInstr::AddI64LC(a, id) => {
                out.push_str(&format!("AddI64LC ${} #{}\n", a.0, id.0));
            }
        }
    }
    Ok(out)
}
