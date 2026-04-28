//! Walks the parsed AST and renders Scout (.sct) source output

use crate::context::{Context, Value};
use crate::error::{TemplateError, TemplateResult};
use crate::parser::{CompareOp, Condition, Expr, ExprNode, Filter, ForNode, IfNode, Node, SetNode};

/// Render a parsed template AST into a Scout source string
pub fn render(nodes: &[Node], ctx: &Context) -> TemplateResult<String> {
    let mut out = String::new();
    let mut ctx = ctx.clone();
    render_nodes(nodes, &mut ctx, &mut out)?;
    // Collapse 3+ consecutive newlines (from stripped block tags) down to 2,
    // preserving intentional blank lines while removing spurious empties.
    let out = collapse_blank_lines(out);
    Ok(out)
}

fn collapse_blank_lines(s: String) -> String {
    let mut result = String::with_capacity(s.len());
    let mut newline_run = 0usize;
    for ch in s.chars() {
        if ch == '\n' {
            if newline_run <= 2 {
                result.push(ch);
            }
        } else {
            newline_run = 0;
            result.push(ch);
        }
    }
    result
}

fn render_nodes(nodes: &[Node], ctx: &mut Context, out: &mut String) -> TemplateResult<()> {
    for node in nodes {
        render_node(node, ctx, out)?;
    }
    Ok(())
}

fn render_node(node: &Node, ctx: &mut Context, out: &mut String) -> TemplateResult<()> {
    match node {
        Node::Raw(s) => out.push_str(s),
        Node::Comment(_) => { /* strip template comments */ }
        Node::RawBlock(s) => out.push_str(s),
        Node::Expr(expr_node) => {
            let val = eval_expr_node(expr_node, ctx)?;
            out.push_str(&val.to_scout_literal());
        }
        Node::If(if_node) => render_if(if_node, ctx, out)?,
        Node::For(for_node) => render_for(for_node, ctx, out)?,
        Node::Set(set_node) => render_set(set_node, ctx)?,

        Node::Include(path) => {
            let mod_path = path.trim_end_matches(".sct").replace('/', "::");
            out.push_str(&format!("use {mod_path}\n"));
        }
    }
    Ok(())
}

fn render_if(node: &IfNode, ctx: &mut Context, out: &mut String) -> TemplateResult<()> {
    for (cond, body) in &node.branches {
        let val = eval_condition(cond, ctx)?;
        if val.is_truthy() {
            ctx.push_scope();
            render_nodes(body, ctx, out)?;
            ctx.pop_scope();
            return Ok(());
        }
    }
    if let Some(else_body) = &node.else_body {
        ctx.push_scope();
        render_nodes(else_body, ctx, out)?;
        ctx.pop_scope();
    }
    Ok(())
}

fn render_for(node: &ForNode, ctx: &mut Context, out: &mut String) -> TemplateResult<()> {
    let list = eval_expr(&node.iterable, ctx)?;
    let items = match list {
        Value::List(items) => items,
        other => vec![other],
    };
    for item in items {
        ctx.push_scope();
        ctx.set(node.var.clone(), item);
        render_nodes(&node.body, ctx, out)?;
        ctx.pop_scope();
    }
    Ok(())
}

fn render_set(node: &SetNode, ctx: &mut Context) -> TemplateResult<()> {
    let val = eval_expr_node(&node.expr, ctx)?;
    ctx.set(node.var.clone(), val);
    Ok(())
}

//
// -- Evaluation
//
fn eval_expr_node(node: &ExprNode, ctx: &Context) -> TemplateResult<Value> {
    let val = eval_expr(&node.expr, ctx)?;
    apply_filters(val, &node.filters)
}

fn eval_condition(cond: &Condition, ctx: &Context) -> TemplateResult<Value> {
    let val = eval_expr(&cond.expr, ctx)?;
    apply_filters(val, &cond.filters)
}

fn eval_expr(expr: &Expr, ctx: &Context) -> TemplateResult<Value> {
    match expr {
        Expr::StrLit(s) => Ok(Value::Str(s.clone())),
        Expr::IntLit(i) => Ok(Value::Int(*i)),
        Expr::FloatLit(f) => Ok(Value::Float(*f)),
        Expr::BoolLit(b) => Ok(Value::Bool(*b)),
        Expr::Null => Ok(Value::Null),
        Expr::Compare { left, op, right } => {
            let l = eval_expr(left, ctx)?;
            let r = eval_expr(right, ctx)?;
            let result = match op {
                CompareOp::Eq => l == r,
                CompareOp::Ne => l != r,
                CompareOp::Lt => compare_ord(&l, &r) == Some(std::cmp::Ordering::Less),
                CompareOp::Gt => compare_ord(&l, &r) == Some(std::cmp::Ordering::Greater),
                CompareOp::Le => matches!(
                    compare_ord(&l, &r),
                    Some(std::cmp::Ordering::Less) | Some(std::cmp::Ordering::Equal)
                ),
                CompareOp::Ge => matches!(
                    compare_ord(&l, &r),
                    Some(std::cmp::Ordering::Greater) | Some(std::cmp::Ordering::Equal)
                ),
            };
            Ok(Value::Bool(result))
        }
        Expr::Var(parts) => {
            let root = &parts[0];
            let val = ctx
                .get(root)
                .ok_or_else(|| TemplateError::UndefinedVariable { name: root.clone() })?;

            if parts.len() == 1 {
                Ok(val.clone())
            } else {
                Err(TemplateError::RenderError(format!(
                    "Dot-chain access `{}` is not yet supported on this value type",
                    parts.join(".")
                )))
            }
        }
    }
}

fn apply_filters(mut val: Value, filters: &[Filter]) -> TemplateResult<Value> {
    for filter in filters {
        val = apply_filter(val, filter)?;
    }
    Ok(val)
}

fn apply_filter(val: Value, filter: &Filter) -> TemplateResult<Value> {
    match filter.name.as_str() {
        "upper" => match val {
            Value::Str(s) => Ok(Value::Str(s.to_uppercase())),
            _ => Err(type_err("upper", "Str")),
        },
        "lower" => match val {
            Value::Str(s) => Ok(Value::Str(s.to_lowercase())),
            _ => Err(type_err("lower", "Str")),
        },
        "trim" => match val {
            Value::Str(s) => Ok(Value::Str(s.trim().to_string())),
            _ => Err(type_err("trim", "Str")),
        },
        "capitalize" => match val {
            Value::Str(s) => {
                let mut c = s.chars();
                let cap = match c.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().to_string() + c.as_str(),
                };
                Ok(Value::Str(cap))
            }
            _ => Err(type_err("capitalize", "Str")),
        },
        "replace" => {
            let arg = filter.arg.as_deref().unwrap_or("");
            let mut parts = arg.splitn(2, ',');
            let from = parts.next().unwrap_or("").trim();
            let to = parts.next().unwrap_or("").trim();
            match val {
                Value::Str(s) => Ok(Value::Str(s.replace(from, to))),
                _ => Err(type_err("replace", "Str")),
            }
        }
        "truncate" => {
            let n: usize = filter.arg.as_deref().unwrap_or("80").parse().unwrap_or(80);
            match val {
                Value::Str(s) => {
                    if s.chars().count() <= n {
                        Ok(Value::Str(s))
                    } else {
                        let truncate: String = s.chars().take(n).collect();
                        Ok(Value::Str(format!("{truncate}...")))
                    }
                }
                _ => Err(type_err("truncate", "Str")),
            }
        }
        "quote" => match val {
            Value::Str(s) => Ok(Value::Str(format!("\"{}\"", s.replace('"', "\\\"")))),
            _ => Err(type_err("quote", "Str")),
        },
        "escape_selector" => match val {
            Value::Str(s) => Ok(Value::Str(css_escape(&s))),
            _ => Err(type_err("escape_selector", "Str")),
        },
        "abs" => match val {
            Value::Int(n) => Ok(Value::Int(n.abs())),
            Value::Float(f) => Ok(Value::Float(f.abs())),
            _ => Err(type_err("abs", "Int | Float")),
        },
        "first" => match val {
            Value::List(items) => Ok(items.into_iter().next().unwrap_or(Value::Null)),
            _ => Err(type_err("first", "List")),
        },
        "last" => match val {
            Value::List(mut items) => Ok(items.pop().unwrap_or(Value::Null)),
            _ => Err(type_err("last", "List")),
        },
        "length" => match val {
            Value::List(items) => Ok(Value::Int(items.len() as i64)),
            Value::Str(s) => Ok(Value::Int(s.chars().count() as i64)),
            _ => Err(type_err("length", "List | Str")),
        },
        "join" => {
            let sep = filter.arg.as_deref().unwrap_or(", ");
            match val {
                Value::List(items) => {
                    let joined = items
                        .iter()
                        .map(|v| v.to_scout_literal())
                        .collect::<Vec<_>>()
                        .join(sep);
                    Ok(Value::Str(joined))
                }
                _ => Err(type_err("join", "List")),
            }
        }
        "reverse" => match val {
            Value::List(mut items) => {
                items.reverse();
                Ok(Value::List(items))
            }
            Value::Str(s) => Ok(Value::Str(s.chars().rev().collect())),
            _ => Err(type_err("reverse", "List | Str")),
        },
        // -- Type coercions --
        "string" => Ok(Value::Str(val.to_scout_literal())),
        "int" => match val {
            Value::Int(n) => Ok(Value::Int(n)),
            Value::Float(f) => Ok(Value::Int(f as i64)),
            Value::Str(s) => s
                .trim()
                .parse::<i64>()
                .map(Value::Int)
                .map_err(|_| TemplateError::RenderError(format!("cannot coerce `{s}` to int"))),
            _ => Err(type_err("int", "Str | Float")),
        },
        // -- Conditional filters
        "default" => {
            if !val.is_truthy() {
                let fallback = filter.arg.as_deref().unwrap_or("").to_string();
                Ok(Value::Str(fallback))
            } else {
                Ok(val)
            }
        }
        // -- Scout-specific filters
        "selector" => match val {
            Value::Str(s) => Ok(Value::Str(format!("$\"{s}\""))),
            _ => Err(type_err("selector", "Str")),
        },
        "multi_selector" => match val {
            Value::Str(s) => Ok(Value::Str(format!("$$\"{s}\""))),
            _ => Err(type_err("multi_selector", "Str")),
        },
        unknown => Err(TemplateError::UnknownFilter {
            name: unknown.to_string(),
        }),
    }
}

fn compare_ord(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Some(x.cmp(y)),
        (Value::Float(x), Value::Float(y)) => x.partial_cmp(y),
        (Value::Int(x), Value::Float(y)) => (*x as f64).partial_cmp(y),
        (Value::Float(x), Value::Int(y)) => x.partial_cmp(&(*y as f64)),
        (Value::Str(x), Value::Str(y)) => Some(x.cmp(y)),
        _ => None,
    }
}

fn type_err(filter: &str, expected: &str) -> TemplateError {
    TemplateError::RenderError(format!("Filter `{filter}` expects a `{expected}` value"))
}

/// Minimal CSS Identifier escaping (backlash - escape non-alphanumeric chars)
fn css_escape(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c.to_string()
            } else {
                format!("\\{c}")
            }
        })
        .collect()
}
