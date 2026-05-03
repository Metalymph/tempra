use std::collections::HashMap;
use crate::types::Type;

#[derive(Debug, Clone)]
pub struct VariableInfo {
    pub ty: Type,
    pub is_mutable: bool,
    pub is_capture: bool,
    pub is_consumed: bool,
    pub is_comptime: bool,
}

/// A stack of lexical scopes mapping variable names to their types and metadata.
pub struct ScopeStack {
    scopes: Vec<HashMap<String, VariableInfo>>,
    boundaries: Vec<usize>,
}

impl ScopeStack {
    pub fn mark_consumed(&mut self, name: &str) {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(info) = scope.get_mut(name) {
                info.is_consumed = true;
                return;
            }
        }
    }

    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            boundaries: vec![0],
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn push_isolated_scope(&mut self) {
        self.boundaries.push(self.scopes.len());
        self.push_scope();
    }

    pub fn pop_scope(&mut self) {
        if let Some(&boundary) = self.boundaries.last() {
            if self.scopes.len() > boundary {
                self.scopes.pop();
                if self.scopes.len() == boundary && self.boundaries.len() > 1 {
                    self.boundaries.pop();
                }
            }
        } else {
            self.scopes.pop();
        }
    }

    pub fn pop_isolated_scope(&mut self) {
        let boundary = self.boundaries.pop().unwrap_or(0);
        while self.scopes.len() > boundary {
            self.scopes.pop();
        }
    }

    pub fn insert(&mut self, name: String, ty: Type) {
        self.insert_info(name, ty, false, false, false);
    }

    pub fn insert_info(&mut self, name: String, ty: Type, is_mutable: bool, is_capture: bool, is_comptime: bool) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, VariableInfo { ty, is_mutable, is_capture, is_consumed: false, is_comptime });
        }
    }

    pub fn lookup(&self, name: &str) -> Option<&VariableInfo> {
        let boundary = self.boundaries.last().copied().unwrap_or(0);
        for scope in self.scopes[boundary..].iter().rev() {
            if let Some(info) = scope.get(name) {
                return Some(info);
            }
        }
        None
    }

    pub fn lookup_type(&self, name: &str) -> Option<&Type> {
        self.lookup(name).map(|info| &info.ty)
    }

    /// Check if a name exists in ANY scope, even outside current isolation boundary.
    pub fn lookup_anywhere(&self, name: &str) -> Option<&VariableInfo> {
        for scope in self.scopes.iter().rev() {
            if let Some(info) = scope.get(name) {
                return Some(info);
            }
        }
        None
    }

    pub fn lookup_type_anywhere(&self, name: &str) -> Option<&Type> {
        self.lookup_anywhere(name).map(|info| &info.ty)
    }
}
