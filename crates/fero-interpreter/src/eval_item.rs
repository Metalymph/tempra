use fero_hir::*;
use std::collections::HashMap;

pub fn register_item(functions: &mut HashMap<String, HirFunction>, item: &HirItem) {
    if let HirItem::Function(func) = item {
        functions.insert(func.name.clone(), func.clone());
    }
}
