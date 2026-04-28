/// Tokens produced by the template lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Raw(String),
    Expr(String),
    Block(String),
    Comment(String),
}

/// Tracks position in source for error reporting
#[derive(Debug, Clone, Copy)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

impl Span {
    fn new() -> Self {
        Span { line: 1, col: 1 }
    }
    fn advance(&mut self, ch: char) {
        if ch == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
    }
}

use crate::error::{TemplateError, TemplateResult};

pub fn lex(source: &str) -> TemplateResult<Vec<(Token, Span)>> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = source.chars().collect();

    let len = chars.len();
    let mut i = 0;
    let mut span = Span::new();
    let mut raw_start = 0;
    let mut raw_span = span;

    macro_rules! flush_raw {
        () => {
            if raw_start < i {
                let raw: String = chars[raw_start..i].iter().collect();
                if !raw.is_empty() {
                    tokens.push((Token::Raw(raw), raw_span));
                }
            }
        };
    }

    while i < len {
        let c0 = chars[i];
        let c1 = if i + 1 < len { chars[i + 1] } else { '\0' };

        match (c0, c1) {
            // Expression tag {{ ... }}
            ('{', '{') => {
                flush_raw!();
                let tag_span = span;
                i += 2;
                span.advance('{');
                span.advance('{');
                let (content, end_i, end_span) = read_until(&chars, i, span, "}}", "{{", tag_span)?;
                tokens.push((Token::Expr(content.trim().to_string()), tag_span));
                i = end_i;
                span = end_span;
                raw_start = i;
                raw_span = span;
            }
            //Block tag{% ... %}
            ('{', '%') => {
                flush_raw!();
                let tag_span = span;
                i += 2;
                span.advance('{');
                span.advance('%');
                let (content, end_i, end_span) = read_until(&chars, i, span, "%}", "{%", tag_span)?;
                tokens.push((Token::Block(content.trim().to_string()), tag_span));
                i = end_i;
                span = end_span;
                raw_start = i;
                raw_span = span;
            }
            ('{', '#') => {
                flush_raw!();
                let tag_span = span;
                i += 2;
                span.advance('{');
                span.advance('#');
                let (content, end_i, end_span) = read_until(&chars, i, span, "#}", "{#", tag_span)?;
                tokens.push((Token::Comment(content.trim().to_string()), tag_span));
                i = end_i;
                span = end_span;
                raw_start = i;
                raw_span = span;
            }
            _ => {
                span.advance(c0);
                i += 1;
            }
        }
    }

    // Flush remaining raw text
    if raw_start < len {
        let raw: String = chars[raw_start..].iter().collect();
        if !raw.is_empty() {
            tokens.push((Token::Raw(raw), raw_span));
        }
    }

    Ok(tokens)
}

fn read_until(
    chars: &[char],
    start: usize,
    start_span: Span,
    close: &str,
    open: &str,
    open_span: Span,
) -> TemplateResult<(String, usize, Span)> {
    let close_chars: Vec<char> = close.chars().collect();
    let clen = close_chars.len();
    let mut i = start;
    let mut span = start_span;
    let mut buf = String::new();

    while i < chars.len() {
        // Check for closing delimiter
        if i + clen <= chars.len() && chars[i..i + clen] == close_chars[..] {
            i += clen;
            for &c in &close_chars {
                span.advance(c);
            }
            return Ok((buf, i, span));
        }
        let c = chars[i];
        buf.push(c);
        span.advance(c);
        i += 1;
    }

    Err(TemplateError::UnclosedTag {
        tag: open.to_string(),
        line: open_span.line,
        col: open_span.col,
    })
}
