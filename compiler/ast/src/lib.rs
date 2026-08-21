//! AST definitions: the data channel between parser and HIR.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.1 and
//! `docs/milestone1/DESIGN.md` section 2.1 for the M1 subset.

/// Byte-offset range into a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(start: u32, end: u32) -> Self {
        Span { start, end }
    }
}

/// A single compiler diagnostic. `span` is genuinely optional: only
/// driver-level failures (unreadable file, linker failure) lack one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub span: Option<Span>,
    pub message: String,
}

impl Diagnostic {
    pub fn at(span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            span: Some(span),
            message: message.into(),
        }
    }

    /// Render as `<file>:<line>:<col>: error: <message>`.
    pub fn render(&self, file_name: &str, source: &str) -> String {
        match self.span {
            Some(span) => {
                let (line, col) = line_col(source, span.start);
                format!("{file_name}:{line}:{col}: error: {}", self.message)
            }
            None => format!("{file_name}: error: {}", self.message),
        }
    }
}

fn line_col(source: &str, offset: u32) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in source.char_indices() {
        if i as u32 >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    pub text: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceFile {
    pub functions: Vec<FunctionDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    pub name: Ident,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StatementKind {
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    StringLiteral { value: String, span: Span },
    Call(CallExpr),
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::StringLiteral { span, .. } => *span,
            Expr::Call(call) => call.span,
        }
    }
}

/// M1: the callee is always a bare identifier (direct top-level call).
#[derive(Debug, Clone, PartialEq)]
pub struct CallExpr {
    pub callee: Ident,
    pub args: Vec<Expr>,
    pub span: Span,
}

/// Indented text dump for golden tests (`scoopc build --emit=ast`).
pub fn dump(file: &SourceFile) -> String {
    let mut out = String::from("SourceFile\n");
    for function in &file.functions {
        out.push_str(&format!("  fun {}\n", function.name.text));
        for statement in &function.body.statements {
            match &statement.kind {
                StatementKind::Expr(expr) => dump_expr(expr, 2, &mut out),
            }
        }
    }
    out
}

fn dump_expr(expr: &Expr, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    match expr {
        Expr::StringLiteral { value, .. } => {
            out.push_str(&format!("{pad}StringLiteral {value:?}\n"));
        }
        Expr::Call(call) => {
            out.push_str(&format!("{pad}Call {}\n", call.callee.text));
            for arg in &call.args {
                dump_expr(arg, indent + 1, out);
            }
        }
    }
}
