use std::fmt;

pub type TemplateResult<T> = Result<T, TemplateError>;

#[derive(Debug, Clone, PartialEq)]
pub enum TemplateError {
    /// A template tag was opened but never closed.
    UnclosedTag {
        tag: String,
        line: usize,
        col: usize,
    },
    /// An expression inside `{{ }}` is syntactically invalid.
    InvalidExpression { expr: String, reason: String },
    /// A variable was referenced that is not in the context.
    UndefinedVariable { name: String },
    ///A filter function was called that doesn't exist.
    UnknownFilter { name: String },
    ///An `{% for %}` block was not terminated with `{% endfor %}`.
    UnclosedBlock { kind: String },
    ///A `{% for %}` block was not terminated with `{% endfor %}`
    MalformedForLoop { reason: String },
    ///General render error.
    RenderError(String),
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TemplateError::UnclosedTag { tag, line, col } => {
                write!(f, "Unclosed tag `{tag} at line {line}, col {col}")
            }
            TemplateError::InvalidExpression { expr, reason } => {
                write!(f, "Invalid express `{expr}`: {reason}")
            }
            TemplateError::UndefinedVariable { name } => {
                write!(f, "Undefined variable `{name}`")
            }
            TemplateError::UnknownFilter { name } => {
                write!(f, "Unknown filter `{name}`")
            }
            TemplateError::UnclosedBlock { kind } => {
                write!(f, "Unclosed block `{kind}` -- missing end tag")
            }
            TemplateError::MalformedForLoop { reason } => {
                write!(f, "Malformed for loop: {reason}")
            }
            TemplateError::RenderError(msg) => {
                write!(f, "Render error: {msg}")
            }
        }
    }
}

impl std::error::Error for TemplateError {}
