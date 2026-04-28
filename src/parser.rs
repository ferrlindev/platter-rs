use crate::error::{TemplateError, TemplateResult};
use crate::lexer::{self, Span, Token};

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Raw(String),
    Expr(ExprNode),
    Comment(String),
    If(IfNode),
    For(ForNode),
    Set(SetNode),
    Include(String),
    RawBlock(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExprNode {
    pub expr: Expr,
    pub filters: Vec<Filter>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    pub name: String,
    pub arg: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Var(Vec<String>),
    StrLit(String),
    IntLit(i64),
    FloatLit(f64),
    BoolLit(bool),
    Null,
    Compare {
        left: Box<Expr>,
        op: CompareOp,
        right: Box<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    pub expr: Expr,
    pub filters: Vec<Filter>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IfNode {
    pub branches: Vec<(Condition, Vec<Node>)>,
    pub else_body: Option<Vec<Node>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForNode {
    pub var: String,
    pub iterable: Expr,
    pub body: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SetNode {
    pub var: String,
    pub expr: ExprNode,
}

// -- Entry point

/// Parse template source into an AST
/// `{% raw %} ... {% endraw %}` is handled at the source level before lexing,
/// so the inner content is never tokenized as template tags.
pub fn parse(source: &str) -> TemplateResult<Vec<Node>> {
    let preprocessed = preprocess_raw_blocks(source)?;
    let tokens = lexer::lex(&preprocessed)?;
    let mut parser = Parser { tokens, pos: 0 };
    parser.parse_nodes(None)
}

/// Replace `{% raw %}...{% endraw %}` sections with a unique placeholder,
/// storing the raw contents seperately, After lexing, the placeholder is recognized
/// as a `Node::RawBlock`
///
/// We do this at the string level so that `{{ }}`  / `{% %}` inside a raw
/// block are never seen by the lexer at all.
fn preprocess_raw_blocks(source: &str) -> TemplateResult<String> {
    let open = "{%";
    let close = "%}";
    let raw_open = "raw";
    let raw_close = "endraw";

    let mut result = String::with_capacity(source.len());
    let mut rest = source;

    while let Some(block_start) = rest.find(open) {
        // find the closing %}
        let after_open = &rest[block_start + open.len()..];
        let block_end = after_open
            .find(close)
            .ok_or_else(|| TemplateError::UnclosedTag {
                tag: "{%".to_string(),
                line: 0,
                col: 0,
            })?;
        let tag_content = after_open[..block_end].trim();

        if tag_content == raw_open {
            // Emit everything before {% raw %}
            result.push_str(&rest[..block_start]);
            let after_raw_tag = block_start + open.len() + block_end + close.len();
            rest = &rest[after_raw_tag..];

            // Find {% endraw %}
            let endraw_needle = format!("{open} {raw_close} {close}");
            let endraw_needle2 = format!("{open}{raw_close}{close}");
            let endraw_pos = rest
                .find(endraw_needle.as_str())
                .or_else(|| rest.find(endraw_needle2.as_str()))
                .or_else(|| find_tag(rest, raw_close))
                .ok_or_else(|| TemplateError::UnclosedBlock {
                    kind: "raw".to_string(),
                })?;

            let raw_content = &rest[..endraw_pos];
            // Emit a special placeholder block that survives lexing unchanged
            // We encode the rew content as a base64-like escaped block tag.
            // Actually  simpler: just emit it as a verbatim raw token  sentinel.
            // We'll use a unique delimiter unlikely  to appear in sct files.
            // Escape template delimiters inside the raw block so the lexer
            // won't tokenize them. We use a null-byte sentinel that survives
            // the lexer as part of a Raw token.
            let escaped = raw_content
                .replace("{{", "\x01\x01")
                .replace("}}", "\x02\x02")
                .replace("{%", "\x03\x03")
                .replace("%}", "\x04\x04")
                .replace("{#", "\x05\x05")
                .replace("#}", "\x06\x06");
            result.push_str("\x00RAW\x00");
            result.push_str(&escaped);
            result.push_str("\x00ENDRAW\x00");

            // Skip past endraw tag
            let endraw_tag_len = find_tag_len(rest, endraw_pos, raw_close);
            rest = &rest[endraw_pos + endraw_tag_len..];
        } else {
            // Not a raw block -- emit up through and including the %} and continue
            let full_tag_end = block_start + open.len() + block_end + close.len();
            result.push_str(&rest[..full_tag_end]);
            rest = &rest[full_tag_end..];
        }
    }

    result.push_str(rest);
    Ok(result)
}

fn find_tag(s: &str, tag: &str) -> Option<usize> {
    let mut i = 0;
    while i < s.len() {
        if s[i..].starts_with("{%") {
            if let Some(end) = s[i + 2..].find("%}") {
                let content = s[i + 2..i + 2 + end].trim();
                if content == tag {
                    return Some(i);
                }
                i = i + 2 + end + 2;
            } else {
                break;
            }
        } else {
            i += 1;
        }
    }
    None
}

fn find_tag_len(s: &str, pos: usize, tag: &str) -> usize {
    if let Some(end) = s[pos + 2..].find("%}") {
        2 + end + 2
    } else {
        2 + tag.len() + 2
    }
}

// --Parser
struct Parser {
    tokens: Vec<(Token, Span)>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|(t, _)| t)
    }
    fn next(&mut self) -> Option<(Token, Span)> {
        let tok = self.tokens.get(self.pos).cloned();
        self.pos += 1;
        tok
    }
    fn parse_nodes(&mut self, stop_tags: Option<&[&str]>) -> TemplateResult<Vec<Node>> {
        let mut nodes = Vec::new();

        loop {
            match self.peek() {
                None => break,
                Some(Token::Block(content)) => {
                    let content = content.clone();
                    let tag = first_word(&content);

                    if let Some(stops) = stop_tags {
                        if stops.contains(&tag) {
                            break;
                        }
                    }

                    self.next();

                    match tag {
                        "if" => nodes.push(self.parse_if(&content)?),
                        "for" => nodes.push(self.parse_for(&content)?),
                        "set" => nodes.push(self.parse_set(&content)?),
                        "include" => {
                            let path = content
                                .trim_start_matches("include")
                                .trim()
                                .trim_matches('"')
                                .to_string();
                            nodes.push(Node::Include(path));
                        }
                        "raw" => nodes.push(self.parse_raw_block()?),
                        "endif" | "endfor" | "else" | "elif" | "endraw" => {
                            return Err(TemplateError::RenderError(format!(
                                "Unexpected tag `{tag}`"
                            )));
                        }
                        other => {
                            return Err(TemplateError::RenderError(format!(
                                "Unknown block tag `{other}`"
                            )));
                        }
                    }
                }
                Some(Token::Expr(content)) => {
                    let content = content.clone();
                    self.next();
                    nodes.push(Node::Expr(parse_expr_node(&content)?));
                }
                Some(Token::Raw(s)) => {
                    let s = s.clone();
                    self.next();
                    // Decode any
                    nodes.extend(decode_raw_sentinels(&s));
                }
                Some(Token::Comment(c)) => {
                    let c = c.clone();
                    self.next();
                    nodes.push(Node::Comment(c));
                }
            }
        }
        Ok(nodes)
    }

    fn parse_if(&mut self, first_content: &str) -> TemplateResult<Node> {
        let cond_str = first_content.trim_start_matches("if").trim().to_string();
        let cond = parse_condition(&cond_str)?;
        let body = self.parse_nodes(Some(&["elif", "else", "endif"]))?;
        let mut branches = vec![(cond, body)];
        let mut else_body = None;

        loop {
            match self.peek() {
                Some(Token::Block(c)) if first_word(c) == "elif" => {
                    let c = c.clone();
                    self.next();
                    let cond_str = c.trim_start_matches("elif").trim().to_string();
                    let cond = parse_condition(&cond_str)?;
                    let body = self.parse_nodes(Some(&["elif", "else", "endif"]))?;
                    branches.push((cond, body));
                }
                Some(Token::Block(c)) if first_word(c) == "else" => {
                    self.next();
                    let body = self.parse_nodes(Some(&["endif"]))?;
                    else_body = Some(body);
                    self.expect_block("endif")?;
                    break;
                }
                Some(Token::Block(c)) if first_word(c) == "endif" => {
                    self.next();
                    break;
                }
                _ => {
                    return Err(TemplateError::UnclosedBlock {
                        kind: "if".to_string(),
                    })
                }
            }
        }

        Ok(Node::If(IfNode {
            branches,
            else_body,
        }))
    }

    fn parse_for(&mut self, content: &str) -> TemplateResult<Node> {
        let rest = content.trim_start_matches("for").trim();
        let parts: Vec<&str> = rest.splitn(3, ' ').collect();
        if parts.len() < 3 || parts[1] != "in" {
            return Err(TemplateError::MalformedForLoop {
                reason: format!("expected `for VAR in EXPR`, got `{rest}`"),
            });
        }
        let var = parts[0].to_string();
        let iterable = parse_expr(parts[2])?;
        let body = self.parse_nodes(Some(&["endfor"]))?;
        self.expect_block("endfor")?;
        Ok(Node::For(ForNode {
            var,
            iterable,
            body,
        }))
    }

    fn parse_set(&mut self, content: &str) -> TemplateResult<Node> {
        let rest = content.trim_start_matches("set").trim();
        let eq = rest
            .find('=')
            .ok_or_else(|| TemplateError::RenderError(format!("set tag missing `=`: `{rest}`")))?;
        let var = rest[..eq].trim().to_string();
        let expr_str = rest[eq + 1..].trim();
        let expr = parse_expr_node(expr_str)?;
        Ok(Node::Set(SetNode { var, expr }))
    }

    fn parse_raw_block(&mut self) -> TemplateResult<Node> {
        // Fallback: if raw blocks arrive through the token stream (shouldn't
        // happen after preprocessing, but handle gracefully)
        let mut buf = String::new();
        loop {
            match self.peek() {
                None => {
                    return Err(TemplateError::UnclosedBlock {
                        kind: "raw".to_string(),
                    })
                }
                Some(Token::Block(c)) if first_word(c) == "endraw" => {
                    self.next();
                    break;
                }
                Some(Token::Raw(s)) => {
                    buf.push_str(s);
                    break;
                }
                _ => {
                    self.next();
                }
            }
        }
        Ok(Node::RawBlock(buf))
    }

    fn expect_block(&mut self, tag: &str) -> TemplateResult<()> {
        match self.peek() {
            Some(Token::Block(c)) if first_word(c) == tag => {
                self.next();
                Ok(())
            }
            _ => Err(TemplateError::UnclosedBlock {
                kind: tag.to_string(),
            }),
        }
    }
}

/// Split as Raw token string on \x00RAW\x00.. \x00ENDRAW\x00 sentinels,
/// producing a mix of Raw and RawBlock nodes.
fn decode_raw_sentinels(s: &str) -> Vec<Node> {
    let open = "\x00RAW\x00";
    let close = "\x00ENDRAW\x00";
    let mut nodes = Vec::new();
    let mut rest = s;

    while let Some(start) = rest.find(open) {
        if start > 0 {
            nodes.push(Node::Raw(rest[..start].to_string()));
        }
        rest = &rest[start + open.len()..];
        if let Some(end) = rest.find(close) {
            nodes.push(Node::RawBlock(
                rest[..end]
                    .replace("\x01\x01", "{{")
                    .replace("\x02\x02", "}}")
                    .replace("\x03\x03", "{%")
                    .replace("\x04\x04", "%}")
                    .replace("\x05\x05", "{#")
                    .replace("\x06\x06", "#}"),
            ));
            rest = &rest[end + close.len()..];
        } else {
            // Malformed sentinel - treat rest as raw
            nodes.push(Node::Raw(rest.to_string()));
            return nodes;
        }
    }
    if !rest.is_empty() {
        nodes.push(Node::Raw(rest.to_string()));
    }
    nodes
}

// -- Expression parsing --
fn parse_expr_node(s: &str) -> TemplateResult<ExprNode> {
    let parts = split_pipe(s);
    if parts.is_empty() {
        return Err(TemplateError::InvalidExpression {
            expr: s.to_string(),
            reason: "empty expression".to_string(),
        });
    }
    let expr = parse_expr(parts[0].trim())?;
    let mut filters = Vec::new();
    for f in &parts[1..] {
        filters.push(parse_filter(f.trim())?);
    }
    Ok(ExprNode { expr, filters })
}

fn parse_condition(s: &str) -> TemplateResult<Condition> {
    let en = parse_expr_node(s)?;
    Ok(Condition {
        expr: en.expr,
        filters: en.filters,
    })
}

fn split_pipe(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut in_quote = false;
    let mut quote_char = '"';
    for (i, c) in s.char_indices() {
        if in_quote {
            if c == quote_char {
                in_quote = false;
            }
        } else if c == '"' || c == '\'' {
            in_quote = true;
            quote_char = c;
        } else if c == '|' {
            parts.push(&s[start..i]);
            start = i + 1;
        }
    }
    parts.push(&s[start..]);
    parts
}

fn parse_expr(s: &str) -> TemplateResult<Expr> {
    let s = s.trim();

    //Try binary comparisons first (before single-value parse)
    for (op_str, op) in &[
        ("==", CompareOp::Eq),
        ("!=", CompareOp::Ne),
        ("<=", CompareOp::Le),
        (">=", CompareOp::Ge),
        ("<", CompareOp::Lt),
        (">", CompareOp::Gt),
    ] {
        if let Some(pos) = find_op(s, op_str) {
            let left = parse_atom(s[..pos].trim())?;
            let right = parse_atom(s[pos + op_str.len()..].trim())?;
            return Ok(Expr::Compare {
                left: Box::new(left),
                op: op.clone(),
                right: Box::new(right),
            });
        }
    }

    parse_atom(s)
}

/// Parse a single atomic value (no operators)
fn parse_atom(s: &str) -> TemplateResult<Expr> {
    let s = s.trim();
    if s == "null" {
        return Ok(Expr::Null);
    }
    if s == "true" {
        return Ok(Expr::BoolLit(true));
    }
    if s == "false" {
        return Ok(Expr::BoolLit(false));
    }
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        return Ok(Expr::StrLit(s[1..s.len() - 1].to_string()));
    }
    if let Ok(n) = s.parse::<i64>() {
        return Ok(Expr::IntLit(n));
    }
    if let Ok(f) = s.parse::<f64>() {
        return Ok(Expr::FloatLit(f));
    }
    if s.chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
        && !s.is_empty()
    {
        let parts: Vec<String> = s.split('.').map(|p| p.to_string()).collect();
        return Ok(Expr::Var(parts));
    }
    Err(TemplateError::InvalidExpression {
        expr: s.to_string(),
        reason: "could not parse as a value or variable".to_string(),
    })
}

/// Find a binary operator in 's' that is not inside quotes.
fn find_op(s: &str, op: &str) -> Option<usize> {
    let chars: Vec<char> = s.chars().collect();
    let op_chars: Vec<char> = op.chars().collect();
    let mut in_quote = false;
    let mut quote_char = '"';
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_quote {
            if c == quote_char {
                in_quote = false;
            }
            i += 1;
            continue;
        }
        if c == '"' || c == '\'' {
            in_quote = true;
            quote_char = c;
            i += 1;
            continue;
        }
        if chars[i..].starts_with(&op_chars) {
            return Some(s.char_indices().nth(i).map(|(b, _)| b).unwrap_or(i));
        }
        i += 1;
    }
    None
}

fn parse_filter(s: &str) -> TemplateResult<Filter> {
    if let Some(paren) = s.find('(') {
        let name = s[..paren].trim().to_string();
        let rest = s[paren + 1..].trim_end_matches(')');
        let arg = Some(rest.trim().trim_matches('"').trim_matches('\'').to_string());
        Ok(Filter { name, arg })
    } else {
        Ok(Filter {
            name: s.trim().to_string(),
            arg: None,
        })
    }
}

fn first_word(s: &str) -> &str {
    s.split_whitespace().next().unwrap_or("")
}
