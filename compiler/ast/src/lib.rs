//! AST definitions: the data channel between parser and HIR.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.1 and
//! `docs/milestone2/DESIGN.md` for the M2 subset.

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

/// A type annotation, e.g. in struct fields and `val x: T = ...`.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeRef {
    pub kind: TypeRefKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeRefKind {
    Named(Ident),
    Tuple(Vec<TypeRef>),
    /// The `Unit` type name (also written `()` in type position).
    Unit,
    /// `T?` — desugars to `Option<T>` in HIR (spec 7.1).
    Nullable(Box<TypeRef>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceFile {
    pub declarations: Vec<Decl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Decl {
    Function(FunctionDecl),
    Struct(StructDecl),
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructDecl {
    pub name: Ident,
    pub fields: Vec<FieldDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDecl {
    pub name: Ident,
    pub ty: TypeRef,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    pub name: Ident,
    /// Generic type parameters (`fun <T> f(...)`); empty for
    /// non-generic functions.
    pub type_params: Vec<Ident>,
    pub params: Vec<Param>,
    /// Return type annotation; absent means `Unit`.
    pub return_ty: Option<TypeRef>,
    pub body: FunctionBody,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: Ident,
    pub ty: TypeRef,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FunctionBody {
    Block(Block),
    /// `fun f(...) [: T] = expr`
    Expr(Box<Expr>),
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
    /// `return` with an optional value (bare `return` in `Unit`
    /// functions).
    Return {
        value: Option<Expr>,
    },
    ValDecl(ValDecl),
    Assign(Assign),
    If(If),
    While(While),
    Block(Block),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValDecl {
    pub mutable: bool,
    pub name: Ident,
    /// Type annotation; genuinely optional (inferred when absent).
    pub ty: Option<TypeRef>,
    pub init: Expr,
    pub span: Span,
}

/// Assignment to a local `var` (M2: plain identifier targets only).
#[derive(Debug, Clone, PartialEq)]
pub struct Assign {
    pub target: Ident,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct If {
    pub cond: Expr,
    pub then_block: Block,
    /// The else branch genuinely may not exist.
    pub else_block: Option<Block>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct While {
    pub cond: Expr,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    StringLiteral {
        value: String,
        span: Span,
    },
    IntLiteral {
        value: i64,
        span: Span,
    },
    BoolLiteral {
        value: bool,
        span: Span,
    },
    /// The `Unit` literal, written `()` or `Unit`.
    UnitLiteral {
        span: Span,
    },
    TupleLiteral {
        elements: Vec<Expr>,
        span: Span,
    },
    /// `Name(arg, ...)` where `Name` resolves to a struct.
    StructInit {
        name: Ident,
        args: Vec<Expr>,
        span: Span,
    },
    Var(Ident),
    FieldAccess(FieldAccess),
    Call(CallExpr),
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    Unary {
        op: UnOp,
        operand: Box<Expr>,
        span: Span,
    },
    /// `expr!!` — unwrap an `Option`, trapping on `None` (M3; real
    /// exception in M8).
    NullAssert {
        operand: Box<Expr>,
        span: Span,
    },
    /// `lhs ?: rhs` (spec 7.3).
    Elvis {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::StringLiteral { span, .. }
            | Expr::IntLiteral { span, .. }
            | Expr::BoolLiteral { span, .. }
            | Expr::UnitLiteral { span }
            | Expr::TupleLiteral { span, .. }
            | Expr::StructInit { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Unary { span, .. }
            | Expr::NullAssert { span, .. }
            | Expr::Elvis { span, .. } => *span,
            Expr::Var(ident) => ident.span,
            Expr::FieldAccess(access) => access.span,
            Expr::Call(call) => call.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldAccess {
    pub receiver: Box<Expr>,
    pub selector: FieldSelector,
    /// `?.` (spec 7.3) instead of `.`.
    pub safe: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FieldSelector {
    /// `.<name>` on a struct.
    Name(Ident),
    /// `._<n>` on a tuple (1-based).
    Index(u32, Span),
}

/// M1: the callee is always a bare identifier (direct top-level call).
#[derive(Debug, Clone, PartialEq)]
pub struct CallExpr {
    pub callee: Ident,
    pub args: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

/// Indented text dump for golden tests (`scoopc build --emit=ast`).
pub fn dump(file: &SourceFile) -> String {
    let mut out = String::from("SourceFile\n");
    for decl in &file.declarations {
        match decl {
            Decl::Struct(s) => {
                out.push_str(&format!("  struct {}\n", s.name.text));
                for field in &s.fields {
                    out.push_str(&format!(
                        "    field {}: {}\n",
                        field.name.text,
                        dump_type_ref(&field.ty)
                    ));
                }
            }
            Decl::Function(f) => {
                let type_params = if f.type_params.is_empty() {
                    String::new()
                } else {
                    let names: Vec<&str> = f.type_params.iter().map(|p| p.text.as_str()).collect();
                    format!("<{}>", names.join(", "))
                };
                let params: Vec<String> = f
                    .params
                    .iter()
                    .map(|p| format!("{}: {}", p.name.text, dump_type_ref(&p.ty)))
                    .collect();
                let ret = f
                    .return_ty
                    .as_ref()
                    .map(|t| format!(": {}", dump_type_ref(t)))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "  fun {}{}({}){}\n",
                    f.name.text,
                    type_params,
                    params.join(", "),
                    ret
                ));
                match &f.body {
                    FunctionBody::Block(block) => dump_block(block, 2, &mut out),
                    FunctionBody::Expr(expr) => {
                        out.push_str(
                            "    =
",
                        );
                        dump_expr(expr, 3, &mut out);
                    }
                }
            }
        }
    }
    out
}

fn dump_type_ref(ty: &TypeRef) -> String {
    match &ty.kind {
        TypeRefKind::Named(name) => name.text.clone(),
        TypeRefKind::Unit => "Unit".to_string(),
        TypeRefKind::Tuple(elements) => {
            let inner: Vec<String> = elements.iter().map(dump_type_ref).collect();
            format!("({})", inner.join(", "))
        }
        TypeRefKind::Nullable(inner) => format!("{}?", dump_type_ref(inner)),
    }
}

fn dump_block(block: &Block, indent: usize, out: &mut String) {
    for statement in &block.statements {
        dump_statement(statement, indent, out);
    }
}

fn dump_statement(statement: &Statement, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    match &statement.kind {
        StatementKind::Expr(expr) => dump_expr(expr, indent, out),
        StatementKind::Return { value } => {
            out.push_str(&format!("{pad}return\n"));
            if let Some(value) = value {
                dump_expr(value, indent + 1, out);
            }
        }
        StatementKind::ValDecl(decl) => {
            let keyword = if decl.mutable { "var" } else { "val" };
            let ty = decl.ty.as_ref().map(dump_type_ref);
            let ty = ty.map(|t| format!(": {t}")).unwrap_or_default();
            out.push_str(&format!("{pad}{keyword} {}{ty}\n", decl.name.text));
            dump_expr(&decl.init, indent + 1, out);
        }
        StatementKind::Assign(assign) => {
            out.push_str(&format!("{pad}assign {}\n", assign.target.text));
            dump_expr(&assign.value, indent + 1, out);
        }
        StatementKind::If(if_) => {
            out.push_str(&format!("{pad}if\n"));
            dump_expr(&if_.cond, indent + 1, out);
            dump_block(&if_.then_block, indent + 1, out);
            if let Some(else_block) = &if_.else_block {
                out.push_str(&format!("{pad}else\n"));
                dump_block(else_block, indent + 1, out);
            }
        }
        StatementKind::While(while_) => {
            out.push_str(&format!("{pad}while\n"));
            dump_expr(&while_.cond, indent + 1, out);
            dump_block(&while_.body, indent + 1, out);
        }
        StatementKind::Block(block) => {
            out.push_str(&format!("{pad}block\n"));
            dump_block(block, indent + 1, out);
        }
    }
}

fn dump_expr(expr: &Expr, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    match expr {
        Expr::StringLiteral { value, .. } => {
            out.push_str(&format!("{pad}StringLiteral {value:?}\n"));
        }
        Expr::IntLiteral { value, .. } => out.push_str(&format!("{pad}IntLiteral {value}\n")),
        Expr::BoolLiteral { value, .. } => out.push_str(&format!("{pad}BoolLiteral {value}\n")),
        Expr::UnitLiteral { .. } => out.push_str(&format!("{pad}UnitLiteral\n")),
        Expr::TupleLiteral { elements, .. } => {
            out.push_str(&format!("{pad}TupleLiteral\n"));
            for element in elements {
                dump_expr(element, indent + 1, out);
            }
        }
        Expr::StructInit { name, args, .. } => {
            out.push_str(&format!("{pad}StructInit {}\n", name.text));
            for arg in args {
                dump_expr(arg, indent + 1, out);
            }
        }
        Expr::Var(ident) => out.push_str(&format!("{pad}Var {}\n", ident.text)),
        Expr::FieldAccess(access) => {
            let selector = match &access.selector {
                FieldSelector::Name(name) => name.text.clone(),
                FieldSelector::Index(index, _) => format!("_{index}"),
            };
            let marker = if access.safe { "?" } else { "" };
            out.push_str(&format!("{pad}FieldAccess {marker}{selector}\n"));
            dump_expr(&access.receiver, indent + 1, out);
        }
        Expr::Call(call) => {
            out.push_str(&format!("{pad}Call {}\n", call.callee.text));
            for arg in &call.args {
                dump_expr(arg, indent + 1, out);
            }
        }
        Expr::Binary { op, lhs, rhs, .. } => {
            out.push_str(&format!("{pad}Binary {op:?}\n"));
            dump_expr(lhs, indent + 1, out);
            dump_expr(rhs, indent + 1, out);
        }
        Expr::Unary { op, operand, .. } => {
            out.push_str(&format!("{pad}Unary {op:?}\n"));
            dump_expr(operand, indent + 1, out);
        }
        Expr::NullAssert { operand, .. } => {
            out.push_str(&format!("{pad}NullAssert\n"));
            dump_expr(operand, indent + 1, out);
        }
        Expr::Elvis { lhs, rhs, .. } => {
            out.push_str(&format!("{pad}Elvis\n"));
            dump_expr(lhs, indent + 1, out);
            dump_expr(rhs, indent + 1, out);
        }
    }
}
