use crate::value::Value;
use std::collections::HashMap;

pub struct RuntimeEnv {
    scopes: Vec<HashMap<String, Value>>,
}

impl RuntimeEnv {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn insert(&mut self, name: String, value: Value) {
        self.scopes.last_mut().unwrap().insert(name, value);
    }

    pub fn insert_var(&mut self, name: String, value: Value) {
        let cell = Value::Cell(std::rc::Rc::new(std::cell::RefCell::new(value)));
        self.scopes.last_mut().unwrap().insert(name, cell);
    }

    pub fn update(&mut self, name: &str, value: Value) -> bool {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(v) = scope.get_mut(name) {
                if let Value::Cell(c) = v {
                    *c.borrow_mut() = value;
                } else {
                    *v = value;
                }
                return true;
            }
        }
        false
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                if let Value::Cell(c) = v {
                    return Some(c.borrow().clone());
                }
                return Some(v.clone());
            }
        }
        None
    }

    pub fn get_raw(&self, name: &str) -> Option<Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v.clone());
            }
        }
        None
    }
}
