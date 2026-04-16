use fero_hir::*;
use std::collections::HashMap;

pub fn register_item(
    functions: &mut HashMap<String, HirFunction>,
    structs: &mut HashMap<String, HirStruct>,
    item: &HirItem
) {
    match item {
        HirItem::Function(func) => {
            functions.insert(func.name.clone(), func.clone());
        }
        HirItem::Struct(s) => {
            structs.insert(s.name.clone(), s.clone());
        }
        _ => {}
    }
}
