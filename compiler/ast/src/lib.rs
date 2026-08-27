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
    /// Index into the driver's input file list; 0-based. Single-file
    /// compiles (and the parser, which sees one file) always use 0.
    pub file: usize,
    pub span: Option<Span>,
    pub message: String,
}

impl Diagnostic {
    pub fn at(span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            file: 0,
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
    /// `Name<T1, T2>` — a generic type application (e.g. `Box<Int>`).
    Generic(Ident, Vec<TypeRef>),
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
    Enum(EnumDecl),
    Class(ClassDecl),
    Interface(InterfaceDecl),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassModifier {
    /// Default: cannot be inherited.
    Final,
    Open,
    Abstract,
}

/// `class Name(props) : Base(args), I1, I2 { members }` (spec 9.1).
#[derive(Debug, Clone, PartialEq)]
pub struct ClassDecl {
    pub modifier: ClassModifier,
    pub name: Ident,
    /// Primary-constructor properties (`val` / `var`).
    pub constructor: Vec<ConstructorProp>,
    /// Base class and its constructor arguments (`: Base(args)`).
    pub base_class: Option<(Ident, Vec<Expr>)>,
    pub interfaces: Vec<Ident>,
    pub methods: Vec<FunctionDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstructorProp {
    pub mutable: bool,
    pub name: Ident,
    pub ty: TypeRef,
    pub span: Span,
}

/// `interface I { fun m(x: Int): String ... }` — method signatures
/// only in M6 (no properties, no default implementations).
#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceDecl {
    pub name: Ident,
    pub methods: Vec<FunctionDecl>,
    pub span: Span,
}

/// `enum E<T> { ... }` (spec 4.2).
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    pub name: Ident,
    pub type_params: Vec<Ident>,
    pub variants: Vec<VariantDecl>,
    /// Implemented interfaces (spec 4.4.3).
    pub interfaces: Vec<Ident>,
    /// Member functions (spec 4.2).
    pub methods: Vec<FunctionDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VariantDecl {
    pub name: Ident,
    pub kind: VariantDeclKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VariantDeclKind {
    /// `SimpleVariant`
    Unit,
    /// `VariantWithValue(Int, String)` — unnamed fields.
    Positional(Vec<TypeRef>),
    /// `Variant { f1: Int, f2: String }` — block-style named fields.
    Named(Vec<VariantFieldDecl>),
    /// `Variant(val f1: Int, val f2: String = "...")` —
    /// constructor-style named fields (spec 4.2); defaults are
    /// constant expressions in M4 (DESIGN.md 5.4).
    Constructor(Vec<VariantFieldDecl>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct VariantFieldDecl {
    pub name: Ident,
    pub ty: TypeRef,
    /// Constructor-style variants only.
    pub default: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructDecl {
    pub name: Ident,
    pub fields: Vec<FieldDecl>,
    /// Implemented interfaces (`struct S(...) : I1, I2`, spec 4.4.3).
    pub interfaces: Vec<Ident>,
    /// Member functions (value receiver, spec 4.1/4.4.3).
    pub methods: Vec<FunctionDecl>,
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
    /// M4: at most `@Intrinsic("name")`, sysroot only (DESIGN.md 1.3).
    pub annotations: Vec<Annotation>,
    /// `override` (required when overriding, forbidden otherwise).
    pub is_override: bool,
    /// `abstract` (bodyless; only in abstract classes / interfaces).
    pub is_abstract: bool,
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
pub struct Annotation {
    pub name: Ident,
    /// The single string argument of `@Intrinsic("name")`.
    pub value: Option<String>,
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
    /// Bodyless: `abstract fun`, interface method signatures, and
    /// `@Intrinsic` functions (spec 13.1).
    None,
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
    /// Pattern `when` (spec 5). Statement-level only in M4.
    When(When),
    ValDecl(ValDecl),
    Assign(Assign),
    If(If),
    While(While),
    Block(Block),
}

/// `when (subject) { arms... }` with an optional trailing `else`.
#[derive(Debug, Clone, PartialEq)]
pub struct When {
    pub subject: Expr,
    pub arms: Vec<WhenArm>,
    pub else_body: Option<Block>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhenArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Block,
    pub span: Span,
}

/// A pattern (spec 4.6 / 5). Syntactically, enum variant patterns and
/// struct patterns share their shapes; HIR resolves which is which.
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// A plain binding name.
    Binding(Ident),
    /// `_`
    Wildcard { span: Span },
    /// A literal matched by equality (`0`, `"x"`, `true`).
    Literal { expr: Box<Expr>, span: Span },
    /// `Path?(p1, p2)` — enum positional variant or struct positional;
    /// `rest` is the `..` marker.
    Positional {
        /// `E.V` as `[E, V]`, or bare `[V]`.
        path: Vec<Ident>,
        elements: Vec<Pattern>,
        rest: Option<Span>,
        span: Span,
    },
    /// `Path?{ f1, f2: renamed, .. }` — enum named-field variant or
    /// struct field pattern.
    Named {
        path: Vec<Ident>,
        fields: Vec<FieldPattern>,
        rest: Option<Span>,
        span: Span,
    },
    /// `(p1, p2)` — tuple pattern.
    Tuple {
        elements: Vec<Pattern>,
        rest: Option<Span>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldPattern {
    pub name: Ident,
    /// `field: renamed` — the binding name when it differs.
    pub rename: Option<Ident>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValDecl {
    pub mutable: bool,
    /// Binding target: a plain identifier is `Pattern::Binding`;
    /// destructuring uses tuple/struct patterns (spec 4.6).
    pub target: Pattern,
    /// Type annotation; genuinely optional (inferred when absent).
    pub ty: Option<TypeRef>,
    pub init: Expr,
    pub span: Span,
}

/// Assignment. `Local` targets a `var`; `Index` targets a
/// `MutableArray` element (spec 10.5).
#[derive(Debug, Clone, PartialEq)]
pub struct Assign {
    pub target: AssignTarget,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AssignTarget {
    Local(Ident),
    /// `receiver[index] = value`
    Index {
        receiver: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    /// `receiver.field = value` (only `var` properties of classes).
    Field {
        receiver: Box<Expr>,
        name: Ident,
        span: Span,
    },
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
    /// `this` (inside member functions).
    This {
        span: Span,
    },
    /// `receiver.name(args)` — method call (M6).
    MethodCall {
        receiver: Box<Expr>,
        name: Ident,
        args: Vec<Expr>,
        span: Span,
    },
    /// `expr is T` / `expr !is T`.
    Is {
        operand: Box<Expr>,
        ty: TypeRef,
        negated: bool,
        span: Span,
    },
    /// `expr as T` / `expr as? T` (`optional` = `as?`, spec 4.4.4).
    Cast {
        operand: Box<Expr>,
        ty: TypeRef,
        optional: bool,
        span: Span,
    },
    /// `[e1, e2, ...]` — array literal (spec 10.2).
    ArrayLiteral {
        elements: Vec<Expr>,
        span: Span,
    },
    /// `receiver[index]` — subscript read (spec 10.5).
    Index {
        receiver: Box<Expr>,
        index: Box<Expr>,
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
            | Expr::Elvis { span, .. }
            | Expr::This { span }
            | Expr::MethodCall { span, .. }
            | Expr::Is { span, .. }
            | Expr::Cast { span, .. }
            | Expr::ArrayLiteral { span, .. }
            | Expr::Index { span, .. } => *span,
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
    /// `===` / `!==` — reference identity (spec 4.4.2; value types
    /// are a compile error).
    RefEq,
    RefNe,
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
            Decl::Enum(e) => {
                let type_params = if e.type_params.is_empty() {
                    String::new()
                } else {
                    let names: Vec<&str> = e.type_params.iter().map(|p| p.text.as_str()).collect();
                    format!("<{}>", names.join(", "))
                };
                out.push_str(&format!("  enum {}{}\n", e.name.text, type_params));
                for variant in &e.variants {
                    match &variant.kind {
                        VariantDeclKind::Unit => {
                            out.push_str(&format!("    {}\n", variant.name.text))
                        }
                        VariantDeclKind::Positional(types) => {
                            let types: Vec<String> = types.iter().map(dump_type_ref).collect();
                            out.push_str(&format!(
                                "    {}({})\n",
                                variant.name.text,
                                types.join(", ")
                            ));
                        }
                        VariantDeclKind::Named(fields) | VariantDeclKind::Constructor(fields) => {
                            let kind = if matches!(variant.kind, VariantDeclKind::Constructor(_)) {
                                "ctor"
                            } else {
                                "named"
                            };
                            out.push_str(&format!("    {} <{}>\n", variant.name.text, kind));
                            for field in fields {
                                let default = field
                                    .default
                                    .as_ref()
                                    .map(|_| " = <expr>")
                                    .unwrap_or_default();
                                out.push_str(&format!(
                                    "      {}: {}{}\n",
                                    field.name.text,
                                    dump_type_ref(&field.ty),
                                    default
                                ));
                            }
                        }
                    }
                }
            }
            Decl::Class(c) => {
                let modifier = match c.modifier {
                    ClassModifier::Final => "",
                    ClassModifier::Open => "open ",
                    ClassModifier::Abstract => "abstract ",
                };
                let ctor: Vec<String> = c
                    .constructor
                    .iter()
                    .map(|p| {
                        format!(
                            "{}{}: {}",
                            if p.mutable { "var " } else { "val " },
                            p.name.text,
                            dump_type_ref(&p.ty)
                        )
                    })
                    .collect();
                let base = c
                    .base_class
                    .as_ref()
                    .map(|(name, args)| format!(" : {}(<{} args>)", name.text, args.len()))
                    .unwrap_or_default();
                let ifaces = if c.interfaces.is_empty() {
                    String::new()
                } else {
                    let names: Vec<&str> = c.interfaces.iter().map(|i| i.text.as_str()).collect();
                    format!(", {}", names.join(", "))
                };
                out.push_str(&format!(
                    "  {modifier}class {}({}){}{}\n",
                    c.name.text,
                    ctor.join(", "),
                    base,
                    ifaces
                ));
                for method in &c.methods {
                    out.push_str(&format!("    fun {}\n", method.name.text));
                }
            }
            Decl::Interface(i) => {
                out.push_str(&format!("  interface {}\n", i.name.text));
                for method in &i.methods {
                    out.push_str(&format!("    fun {}\n", method.name.text));
                }
            }
            Decl::Struct(s) => {
                out.push_str(&format!("  struct {}\n", s.name.text));
                for field in &s.fields {
                    out.push_str(&format!(
                        "    field {}: {}\n",
                        field.name.text,
                        dump_type_ref(&field.ty)
                    ));
                }
                for method in &s.methods {
                    out.push_str(&format!("    fun {}\n", method.name.text));
                }
            }
            Decl::Function(f) => {
                for annotation in &f.annotations {
                    let value = annotation
                        .value
                        .as_ref()
                        .map(|v| format!("({v:?})"))
                        .unwrap_or_default();
                    out.push_str(&format!("    @{}{}\n", annotation.name.text, value));
                }
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
                let flags = format!(
                    "{}{}",
                    if f.is_abstract { "abstract " } else { "" },
                    if f.is_override { "override " } else { "" }
                );
                out.push_str(&format!(
                    "  {flags}fun {}{}({}){}\n",
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
                    FunctionBody::None => {}
                }
            }
        }
    }
    out
}

fn dump_type_ref(ty: &TypeRef) -> String {
    match &ty.kind {
        TypeRefKind::Named(name) => name.text.clone(),
        TypeRefKind::Generic(name, args) => {
            let inner: Vec<String> = args.iter().map(dump_type_ref).collect();
            format!("{}<{}>", name.text, inner.join(", "))
        }
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
            out.push_str(&format!(
                "{pad}{keyword} {}{ty}\n",
                dump_pattern(&decl.target)
            ));
            dump_expr(&decl.init, indent + 1, out);
        }
        StatementKind::When(when) => {
            out.push_str(&format!("{pad}when\n"));
            dump_expr(&when.subject, indent + 1, out);
            for arm in &when.arms {
                out.push_str(&format!(
                    "{}  arm {}{}\n",
                    pad,
                    dump_pattern(&arm.pattern),
                    if arm.guard.is_some() {
                        " if <guard>"
                    } else {
                        ""
                    }
                ));
                dump_block(&arm.body, indent + 2, out);
            }
            if let Some(else_body) = &when.else_body {
                out.push_str(&format!("{pad}  else\n"));
                dump_block(else_body, indent + 2, out);
            }
        }
        StatementKind::Assign(assign) => {
            match &assign.target {
                AssignTarget::Local(name) => out.push_str(&format!("{pad}assign {}\n", name.text)),
                AssignTarget::Index { .. } => out.push_str(&format!("{pad}assign []\n")),
                AssignTarget::Field { name, .. } => {
                    out.push_str(&format!("{pad}assign .{}\n", name.text))
                }
            }
            match &assign.target {
                AssignTarget::Index {
                    receiver, index, ..
                } => {
                    dump_expr(receiver, indent + 1, out);
                    dump_expr(index, indent + 1, out);
                }
                AssignTarget::Field { receiver, .. } => {
                    dump_expr(receiver, indent + 1, out);
                }
                AssignTarget::Local(_) => {}
            }
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

/// Compact one-line pattern rendering for dumps.
pub fn dump_pattern(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Binding(name) => name.text.clone(),
        Pattern::Wildcard { .. } => "_".to_string(),
        Pattern::Literal { expr, .. } => format!("{:?}", expr).chars().take(40).collect(),
        Pattern::Positional {
            path,
            elements,
            rest,
            ..
        } => {
            let path = path
                .iter()
                .map(|p| p.text.as_str())
                .collect::<Vec<_>>()
                .join(".");
            let mut parts: Vec<String> = elements.iter().map(dump_pattern).collect();
            if rest.is_some() {
                parts.push("..".to_string());
            }
            format!("{}({})", path, parts.join(", "))
        }
        Pattern::Named {
            path, fields, rest, ..
        } => {
            let path = path
                .iter()
                .map(|p| p.text.as_str())
                .collect::<Vec<_>>()
                .join(".");
            let mut parts: Vec<String> = fields
                .iter()
                .map(|f| match &f.rename {
                    Some(rename) => format!("{}: {}", f.name.text, rename.text),
                    None => f.name.text.clone(),
                })
                .collect();
            if rest.is_some() {
                parts.push("..".to_string());
            }
            format!("{}{{{}}}", path, parts.join(", "))
        }
        Pattern::Tuple { elements, rest, .. } => {
            let mut parts: Vec<String> = elements.iter().map(dump_pattern).collect();
            if let Some(rest) = rest {
                parts.push(format!("..@{}", rest.start));
            }
            format!("({})", parts.join(", "))
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
        Expr::This { .. } => out.push_str(&format!("{pad}This\n")),
        Expr::MethodCall {
            receiver,
            name,
            args,
            ..
        } => {
            out.push_str(&format!("{pad}MethodCall {}\n", name.text));
            dump_expr(receiver, indent + 1, out);
            for arg in args {
                dump_expr(arg, indent + 1, out);
            }
        }
        Expr::Is {
            operand,
            ty,
            negated,
            ..
        } => {
            out.push_str(&format!("{pad}Is {} {negated}\n", dump_type_ref(ty)));
            dump_expr(operand, indent + 1, out);
        }
        Expr::Cast {
            operand,
            ty,
            optional,
            ..
        } => {
            out.push_str(&format!(
                "{pad}Cast {} optional={optional}\n",
                dump_type_ref(ty)
            ));
            dump_expr(operand, indent + 1, out);
        }
        Expr::ArrayLiteral { elements, .. } => {
            out.push_str(&format!("{pad}ArrayLiteral\n"));
            for element in elements {
                dump_expr(element, indent + 1, out);
            }
        }
        Expr::Index {
            receiver, index, ..
        } => {
            out.push_str(&format!("{pad}Index\n"));
            dump_expr(receiver, indent + 1, out);
            dump_expr(index, indent + 1, out);
        }
    }
}
