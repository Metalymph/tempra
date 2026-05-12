use crate::ir::*;

pub fn format_tir_module(module: &TirModule) -> String {
    let mut out = String::new();
    for func in &module.functions {
        out.push_str(&format_tir_function(func));
        out.push('\n');
    }
    out
}

pub fn format_tir_function(func: &TirFunction) -> String {
    let mut out = String::new();
    let params: Vec<String> = func.params.iter()
        .map(|(id, ty)| format!("%{}: {}", id.0, format_type(ty)))
        .collect();
    
    out.push_str(&format!("fn {} ({}) -> {} {{\n", func.name, params.join(", "), format_type(&func.return_type)));
    
    let mut sorted_slots: Vec<_> = func.debug_slots.iter().collect();
    sorted_slots.sort_by_key(|(id, _)| id.0);
    
    for (slot_id, name) in sorted_slots {
        out.push_str(&format!("  // slot ${} = {}\n", slot_id.0, name));
    }

    for block in &func.blocks {
        out.push_str(&format!("^block{}:\n", block.id.0));
        for instr in &block.instrs {
            out.push_str(&format!("  {}\n", format_instr(instr)));
        }
        if let Some(term) = &block.terminator {
            out.push_str(&format!("  {}\n", format_terminator(term)));
        } else {
            out.push_str("  <missing terminator>\n");
        }
    }
    out.push_str("}\n");
    out
}

pub fn format_type(ty: &TirType) -> String {
    match ty {
        TirType::Unit => "()".to_string(),
        TirType::Bool => "bool".to_string(),
        TirType::Int64 => "i64".to_string(),
        TirType::UInt64 => "u64".to_string(),
        TirType::UInt8 => "u8".to_string(),
        TirType::Float64 => "f64".to_string(),
        TirType::String => "String".to_string(),
        TirType::Never => "!".to_string(),
        TirType::Tag => "Tag".to_string(),
        TirType::Array(t) => format!("[{}]", format_type(t)),
        TirType::Slice(t) => format!("&[{}]", format_type(t)),
        TirType::Tuple(ts) => format!("({})", ts.iter().map(format_type).collect::<Vec<_>>().join(", ")),
        TirType::Struct(name) => name.clone(),
        TirType::Enum(name) => name.clone(),
        TirType::Option(t) => format!("Option<{}>", format_type(t)),
        TirType::Result(t, e) => format!("Result<{}, {}>", format_type(t), format_type(e)),
        TirType::Function(args, ret) => {
            let arg_strs: Vec<String> = args.iter().map(format_type).collect();
            format!("fn({}) -> {}", arg_strs.join(", "), format_type(ret))
        }
    }
}

pub fn format_instr(instr: &TirInstr) -> String {
    match instr {
        TirInstr::Const(res, val) => format!("%{} = const {}", res.0, format_const(val)),
        TirInstr::Make(res, ty, variant, args) => format!("%{} = make {}::{:?} ({})", res.0, format_type(ty), variant, format_args(args)),
        TirInstr::MakeStruct(res, name, fields) => format!("%{} = make struct {} {{ {} }}", res.0, name, fields.iter().map(|(n, v)| format!("{}: %{}", n, v.0)).collect::<Vec<_>>().join(", ")),
        TirInstr::Get(res, target, key) => format!("%{} = get %{}, {}", res.0, target.0, format_key(key)),
        TirInstr::Set(res, target, key, val) => format!("%{} = set %{}, {}, %{}", res.0, target.0, format_key(key), val.0),
        TirInstr::Tag(res, target) => format!("%{} = tag %{}", res.0, target.0),
        TirInstr::Unwrap(res, target, variant, idx) => format!("%{} = unwrap %{} as {}.{}", res.0, target.0, variant, idx),
        TirInstr::Slot(slot, ty) => format!("slot ${}: {}", slot.0, format_type(ty)),
        TirInstr::Load(res, slot) => format!("%{} = load ${}", res.0, slot.0),
        TirInstr::Store(slot, val) => format!("store ${}, %{}", slot.0, val.0),
        TirInstr::Call(res, func, args) => format!("%{} = call @{} ({})", res.0, func.0, format_args(args)),
        TirInstr::Add(res, a, b) => format!("%{} = add %{}, %{}", res.0, a.0, b.0),
        TirInstr::Sub(res, a, b) => format!("%{} = sub %{}, %{}", res.0, a.0, b.0),
        TirInstr::Mul(res, a, b) => format!("%{} = mul %{}, %{}", res.0, a.0, b.0),
        TirInstr::Div(res, a, b) => format!("%{} = div %{}, %{}", res.0, a.0, b.0),
        TirInstr::Eq(res, a, b) => format!("%{} = eq %{}, %{}", res.0, a.0, b.0),
        TirInstr::Lt(res, a, b) => format!("%{} = lt %{}, %{}", res.0, a.0, b.0),
        TirInstr::Gt(res, a, b) => format!("%{} = gt %{}, %{}", res.0, a.0, b.0),
        TirInstr::And(res, a, b) => format!("%{} = and %{}, %{}", res.0, a.0, b.0),
        TirInstr::Or(res, a, b) => format!("%{} = or %{}, %{}", res.0, a.0, b.0),
        TirInstr::Not(res, a) => format!("%{} = not %{}", res.0, a.0),
        TirInstr::GetIndex(res, obj, idx) => format!("%{} = get_index %{}, %{}", res.0, obj.0, idx.0),
        TirInstr::Slice(res, obj, s, e) => format!("%{} = slice %{}, %{}..%{}", res.0, obj.0, s.0, e.0),
        TirInstr::Len(res, obj) => format!("%{} = len %{}", res.0, obj.0),
        TirInstr::Size(res, obj) => format!("%{} = size %{}", res.0, obj.0),
        TirInstr::Bytes(res, obj) => format!("%{} = bytes %{}", res.0, obj.0),
    }
}

pub fn format_args(args: &[TirValueId]) -> String {
    args.iter().map(|a| format!("%{}", a.0)).collect::<Vec<_>>().join(", ")
}

pub fn format_key(key: &TirKey) -> String {
    match key {
        TirKey::Field(f) => format!(".{}", f),
        TirKey::Index(i) => format!("{}", i),
    }
}

pub fn format_const(val: &TirConst) -> String {
    match val {
        TirConst::Unit => "()".to_string(),
        TirConst::Bool(b) => b.to_string(),
        TirConst::Int64(i) => format!("{}i64", i),
        TirConst::UInt64(u) => format!("{}u64", u),
        TirConst::Float64(f) => format!("{}f64", f),
        TirConst::String(s) => format!("{:?}", s),
    }
}

pub fn format_terminator(term: &TirTerminator) -> String {
    match term {
        TirTerminator::Br(target) => format!("br ^block{}", target.0),
        TirTerminator::CondBr(cond, t, f) => format!("condbr %{}, ^block{}, ^block{}", cond.0, t.0, f.0),
        TirTerminator::Ret(Some(val)) => format!("ret %{}", val.0),
        TirTerminator::Ret(None) => "ret".to_string(),
        TirTerminator::Unreachable => "unreachable".to_string(),
    }
}
