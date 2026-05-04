use std::collections::{BTreeMap, HashMap};
use tempra_hir::*;

pub fn register_item(
    functions: &mut HashMap<String, HirFunction>,
    structs: &mut HashMap<String, HirStruct>,
    enums: &mut BTreeMap<String, HirEnum>,
    interfaces: &mut HashMap<String, HirInterface>,
    item: &HirItem,
) {
    match item {
        HirItem::Function(func) => {
            functions.insert(func.name.clone(), func.clone());
        }
        HirItem::Struct(s) => {
            structs.insert(s.name.clone(), s.clone());
        }
        HirItem::Enum(e) => {
            enums.insert(e.name.clone(), e.clone());
        }
        HirItem::Interface(i) => {
            interfaces.insert(i.name.clone(), i.clone());
        }
        HirItem::Extend(e) => {
            let target_ty_name = match &e.target {
                HirType::Named(_, name) => name.clone(),
                _ => return, // Unsupported for now or Error
            };
            for m in &e.methods {
                let full_name = format!("{}.{}", target_ty_name, m.name);
                functions.insert(full_name, m.clone());
            }
        }
        _ => {}
    }
}
