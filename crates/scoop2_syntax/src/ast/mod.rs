//! Scoop 下一代前端的 AST 定义。
//!
//! 设计要点（对应 `docs/spec/grammar.md`，即 normative grammar）：
//!
//! - **每个节点携带 `NodeId` + `Span`**：节点统一为 `X { id, span, kind }` 形态
//!   （`Expr` / `Stmt` / `Item` / `TypeMember` / `TypeRef` / `TypeArg` /
//!   `Pattern` / `EffectRowExpr` 等），或带 `id`/`span` 的具名字段结构体
//!   （`Param`、`CallArg`、`WhenArm` 等）。叶子文本单元（[`Ident`]、
//!   [`TypePath`]、[`Modifier`]）只携带 `Span`。组合节点的 span 覆盖其完整
//!   源码区间。
//! - **所有标识符都是 [`Symbol`]**（interned），AST 拥有全部数据，不借用源文本。
//! - **没有 placeholder / error 变体**：缺失成分一律用 `Option<T>` 表示；
//!   错误恢复由 parser 产出“partial-but-valid”节点 + 诊断。
//! - **字面量在 AST 中已解码**：[`IntLit`] / [`FloatLit`] / [`CharLit`] /
//!   [`StringLit`] 的值由 `from_token_text` 通过 lexer 的字面量 helper 解码
//!   （lexer 已校验，故解码 infallible）。
//! - **`try/catch` 没有 AST 节点**：parser 按 §8.6 直接脱糖为
//!   `handle` over `scoop.core.Raise.raise`（见 [`expr::ExprKind::Handle`]）。
//!
//! 模块划分：
//!
//! - 本文件：核心类型（文件/包/导入、标识符与路径、注解、修饰符、字面量）；
//! - [`decl`]：item / 声明（含 generics 声明位）；
//! - [`expr`]：表达式与语句；
//! - [`types`]：类型引用与 effect 行；
//! - [`pattern`]：模式（`val` 解构与 `when` 分支共用）。

pub mod decl;
pub mod expr;
pub mod pattern;
pub mod types;

use scoop2_base::{NodeId, Span, Symbol};

use crate::lexer::{char_literal, float_literal, int_literal, string_literal};

pub use decl::*;
pub use expr::*;
pub use pattern::*;
pub use types::*;

/// 标识符：interned 符号 + 出现位置。
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Ident {
    pub symbol: Symbol,
    pub span: Span,
}

/// 点分路径（`a.b.C`）。
///
/// 统一用于 package / import / 注解路径 / 类型路径 / handle effect 路径 /
/// variant 模式路径。仅表达语法形态；哪一段是包、哪一段是类型由 resolve 决定。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TypePath {
    pub segments: Vec<Ident>,
    pub span: Span,
}

/// 源文件（§2）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct File {
    pub id: NodeId,
    pub span: Span,
    /// `@file:...` 文件级注解（use-site target 固定为 `file`，仅出现在文件开头）。
    pub file_annotations: Vec<AnnotationUse>,
    pub package: Option<PackageDecl>,
    pub imports: Vec<ImportDecl>,
    pub items: Vec<Item>,
}

/// `package a.b.c`。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PackageDecl {
    pub id: NodeId,
    pub span: Span,
    pub path: TypePath,
}

/// `import a.b.* as c`（§2）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImportDecl {
    pub id: NodeId,
    pub span: Span,
    pub path: TypePath,
    /// 通配导入 `.*` 的 span；非通配为 `None`。通配导入不允许 alias（parser 报错）。
    pub wildcard: Option<Span>,
    pub alias: Option<Ident>,
}

/// 注解使用：`@target:path(args...)`（§4）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnnotationUse {
    pub id: NodeId,
    pub span: Span,
    /// use-site target（`@file:` / `@param:` / `@property:` 等；parser 不校验合法性）。
    pub target: Option<Ident>,
    pub path: TypePath,
    pub args: Vec<AnnotationArg>,
}

/// 注解实参：`(IDENT ('=' | ':'))? expr`（§4）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnnotationArg {
    pub id: NodeId,
    pub span: Span,
    /// 命名实参名；位置实参为 `None`。
    pub name: Option<Ident>,
    pub value: Expr,
}

/// 声明修饰符（§3）。
///
/// `inline` 已被语言移除（§10）：parser 遇到会记录 dedicated 诊断，
/// AST 不为其建模。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ModifierKind {
    Public,
    Internal,
    Private,
    Open,
    Abstract,
    Sealed,
    Override,
    Operator,
    /// `annotation class` 的 `annotation` 修饰符（限制由 typecheck 检查）。
    Annotation,
}

/// 一个修饰符的出现（parser 会对修饰符集合排序去重，源码顺序不保留）。
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Modifier {
    pub kind: ModifierKind,
    pub span: Span,
}

/// 整数字面量后缀（§1.2 `INT_SUFFIX`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IntSuffix {
    /// `u` / `U`
    U,
    /// `l` / `L`
    L,
    /// `ul` / `uL` / `UL` / `Ul`
    UL,
}

/// 已解码的整数字面量。
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct IntLit {
    pub value: u128,
    pub suffix: Option<IntSuffix>,
    pub span: Span,
}

impl IntLit {
    /// 从 lexer 已校验的 token 文本解码（如 `0xFFuL`、`1_000`）。
    ///
    /// # Panics
    /// 文本未通过 lexer 校验时 panic（属于编译器内部 bug）。
    pub fn from_token_text(text: &str, span: Span) -> Self {
        let suffix = match int_literal::parse_int_literal_suffix(text) {
            int_literal::IntLiteralSuffix::None => None,
            int_literal::IntLiteralSuffix::UInt => Some(IntSuffix::U),
            int_literal::IntLiteralSuffix::Long => Some(IntSuffix::L),
            int_literal::IntLiteralSuffix::ULong => Some(IntSuffix::UL),
        };
        Self {
            value: int_literal::parse_int_literal(text),
            suffix,
            span,
        }
    }
}

/// 浮点字面量后缀（§1.2 `FLOAT_SUFFIX`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FloatSuffix {
    /// `f` / `f32`（无后缀即 Float64，用 `None` 表示）。
    F32,
}

/// 已解码的浮点字面量。
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct FloatLit {
    pub value: f64,
    pub suffix: Option<FloatSuffix>,
    pub span: Span,
}

impl FloatLit {
    /// 从 lexer 已校验的 token 文本解码（如 `3.14`、`0.5f32`）。
    ///
    /// # Panics
    /// 文本未通过 lexer 校验时 panic（属于编译器内部 bug）。
    pub fn from_token_text(text: &str, span: Span) -> Self {
        let parsed = float_literal::parse_float_literal(text);
        let suffix = match parsed.suffix {
            float_literal::FloatLiteralSuffix::Float64 => None,
            float_literal::FloatLiteralSuffix::Float32 => Some(FloatSuffix::F32),
        };
        Self {
            value: parsed.value,
            suffix,
            span,
        }
    }
}

/// 已解码的 Char 字面量。
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct CharLit {
    pub value: char,
    pub span: Span,
}

impl CharLit {
    /// 从 lexer 已校验的 token 文本解码（如 `'a'`、`'\u0041'`）。
    ///
    /// # Panics
    /// 文本未通过 lexer 校验时 panic（属于编译器内部 bug）。
    pub fn from_token_text(text: &str, span: Span) -> Self {
        let value = char_literal::parse_char_literal(text)
            .expect("char literal token text is lexer-validated");
        Self { value, span }
    }
}

/// 已解码的字符串字面量（普通或 raw 三引号；f-string 见
/// [`expr::ExprKind::InterpolatedString`]）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StringLit {
    pub value: String,
    pub span: Span,
}

impl StringLit {
    /// 从 lexer 已校验的 token 文本解码（含引号，如 `"a\n"`）。
    ///
    /// # Panics
    /// 文本未通过 lexer 校验时 panic（属于编译器内部 bug）。
    pub fn from_token_text(text: &str, span: Span) -> Self {
        let value = string_literal::parse_string_literal_utf8(text)
            .expect("string literal token text is lexer-validated");
        Self { value, span }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sp() -> Span {
        Span::new(0, 3)
    }

    #[test]
    fn int_lit_decodes_value_and_suffix() {
        let lit = IntLit::from_token_text("0xFFuL", sp());
        assert_eq!(lit.value, 255);
        assert_eq!(lit.suffix, Some(IntSuffix::UL));

        let lit = IntLit::from_token_text("1_000", sp());
        assert_eq!(lit.value, 1000);
        assert_eq!(lit.suffix, None);

        let lit = IntLit::from_token_text("0b1010u", sp());
        assert_eq!(lit.value, 10);
        assert_eq!(lit.suffix, Some(IntSuffix::U));

        let lit = IntLit::from_token_text("42L", sp());
        assert_eq!(lit.value, 42);
        assert_eq!(lit.suffix, Some(IntSuffix::L));
    }

    #[test]
    fn float_lit_decodes_value_and_suffix() {
        let lit = FloatLit::from_token_text("3.5", sp());
        assert_eq!(lit.value, 3.5);
        assert_eq!(lit.suffix, None);

        let lit = FloatLit::from_token_text("1.25f32", sp());
        assert_eq!(lit.value, 1.25);
        assert_eq!(lit.suffix, Some(FloatSuffix::F32));

        let lit = FloatLit::from_token_text("1_000.5f", sp());
        assert_eq!(lit.value, 1000.5);
        assert_eq!(lit.suffix, Some(FloatSuffix::F32));
    }

    #[test]
    fn char_lit_decodes_escapes() {
        assert_eq!(CharLit::from_token_text("'a'", sp()).value, 'a');
        assert_eq!(CharLit::from_token_text("'\\n'", sp()).value, '\n');
        assert_eq!(CharLit::from_token_text("'\\u0041'", sp()).value, 'A');
    }

    #[test]
    fn string_lit_decodes_normal_and_raw() {
        assert_eq!(StringLit::from_token_text("\"a\\nb\"", sp()).value, "a\nb");
        assert_eq!(
            StringLit::from_token_text("\"\"\"a\\nb\"\"\"", sp()).value,
            "a\\nb"
        );
    }
}

/// 测试辅助：手工构建 AST 时分配 NodeId / intern 标识符。
#[cfg(test)]
pub(crate) mod testutil {
    use std::cell::RefCell;

    use scoop2_base::{Interner, NodeId, NodeIdAllocator, Span};

    use super::{Ident, TypePath};

    /// 手工构建 AST 的辅助：NodeId 分配 + interning。
    ///
    /// 内部使用 [`RefCell`]，所有方法都取 `&self`，因此可以在构建表达式中
    /// 自由嵌套调用（如 `b.item(..., b.ident(...))`）。
    pub(crate) struct TestBuilder {
        interner: RefCell<Interner>,
        alloc: RefCell<NodeIdAllocator>,
    }

    impl TestBuilder {
        pub(crate) fn new() -> Self {
            Self {
                interner: RefCell::new(Interner::new()),
                alloc: RefCell::new(NodeIdAllocator::new()),
            }
        }

        pub(crate) fn id(&self) -> NodeId {
            self.alloc.borrow_mut().alloc()
        }

        pub(crate) fn ident(&self, name: &str, span: Span) -> Ident {
            Ident {
                symbol: self.interner.borrow_mut().intern(name),
                span,
            }
        }

        /// 用等长段拼一个路径（每段 span 依次排列，仅供测试）。
        pub(crate) fn path(&self, start: usize, names: &[&str]) -> TypePath {
            let mut segments = Vec::new();
            let mut offset = start;
            for name in names {
                segments.push(self.ident(name, Span::new(offset, offset + name.len())));
                offset += name.len() + 1;
            }
            TypePath {
                segments,
                span: Span::new(start, offset.saturating_sub(1).max(start)),
            }
        }

        /// 借用 interner 执行只读操作（例如渲染 dump）。
        pub(crate) fn with_interner<R>(&self, f: impl FnOnce(&Interner) -> R) -> R {
            f(&self.interner.borrow())
        }
    }
}
