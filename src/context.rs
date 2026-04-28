use std::collections::HashMap;

/// A value that can be stored in a template context
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    List(Vec<Value>),
    Null,
}

impl Value {
    /// Render this value as a Scout literal string.
    pub fn to_scout_literal(&self) -> String {
        match self {
            Value::Str(s) => s.clone(),
            Value::Int(n) => n.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".to_string(),
            Value::List(items) => {
                let inner: Vec<String> = items.iter().map(|v| v.to_scout_literal()).collect();
                format!("[{}]", inner.join(", "))
            }
        }
    }

    /// Return truthy evaluation used in {% if %} blocks
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Null => false,
            Value::Int(n) => *n != 0,
            Value::Float(f) => *f != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::List(l) => !l.is_empty(),
        }
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Str(s.to_string())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Str(s)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Value::Int(n)
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Value::Float(f)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<Vec<Value>> for Value {
    fn from(l: Vec<Value>) -> Self {
        Value::List(l)
    }
}

/// Template rendering context
#[derive(Debug, Clone, Default)]
pub struct Context {
    scopes: Vec<HashMap<String, Value>>,
}

impl Context {
    pub fn new() -> Self {
        Context {
            scopes: vec![HashMap::new()],
        }
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<Value>) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(key.into(), value.into());
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(key) {
                return Some(v);
            }
        }
        None
    }

    pub(crate) fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub(crate) fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }
}
