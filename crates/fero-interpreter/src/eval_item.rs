use fero_hir::*;
use std::collections::{BTreeMap, HashMap};

pub fn register_item(
    functions: &mut HashMap<String, HirFunction>,
    structs: &mut HashMap<String, HirStruct>,
    enums: &mut BTreeMap<String, HirEnum>,
    item: &HirItem
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
        _ => {}
    }
}
