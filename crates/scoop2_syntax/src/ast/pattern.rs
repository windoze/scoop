//! 模式（§9）：`val` 解构绑定（§9.1）与 `when` 分支模式（§9.2）共用一套类型。

use scoop2_base::{NodeId, Span};

use super::types::TypeRef;
use super::{CharLit, Ident, IntLit, StringLit, TypePath};

/// 模式节点。
#[derive(Debug, Clone)]
pub struct Pattern {
    pub id: NodeId,
    pub span: Span,
    pub kind: PatternKind,
}

#[derive(Debug, Clone)]
pub enum PatternKind {
    /// `_` 通配。
    Wildcard,
    /// 绑定名（`x`；when 中小写开头的裸 IDENT，§9.2 的大小写启发式由 parser 应用）。
    Bind(Ident),
    /// 字面量模式（int / char / string / bool；**仅 when**，§9.2）。
    Literal(PatternLiteral),
    /// 元组模式：`(p1, p2, ..)`。
    ///
    /// `..` rest 是 `kind` 为 [`PatternKind::Rest`] 的元素（至多一个且必须在最后，
    /// 由 parser 检查）。
    Tuple(Vec<Pattern>),
    /// 结构体模式：`Path { field, field: pat, .. }`（§9.1，仅解构）。
    Struct {
        path: TypePath,
        fields: Vec<StructPatternField>,
        /// 尾部裸 `..` 的 span。
        rest: Option<Span>,
    },
    /// variant 模式：`Path`、`Path(p1, ..)`。
    ///
    /// `args` 为 `None` 表示无括号形式（0 参数 variant，如 `None`）；
    /// `Some(vec![])` 表示空括号 `C()`。参数中的 `..` rest 同样是
    /// [`PatternKind::Rest`] 元素。
    Variant {
        path: TypePath,
        args: Option<Vec<Pattern>>,
    },
    /// `..` rest（仅作为元组 / variant 模式的元素出现）。
    Rest,
    /// `is TypeRef`（**仅 when**）。
    Is(TypeRef),
    /// `else`（**仅 when**，且只能作为首个分支，不能出现在 `|` 之后）。
    Else,
    /// or-pattern `A | B`（**仅 when**；`else` 不允许出现在 `|` 之后，§9.2）。
    Or(Vec<Pattern>),
}

/// when 分支中的字面量模式（§9.2：int / char / string / bool，
/// **float 字面量在模式中是 parse error**）。
#[derive(Debug, Clone)]
pub enum PatternLiteral {
    Int(IntLit),
    Char(CharLit),
    String(StringLit),
    /// `true` / `false`（源码中是普通 IDENT，由 parser 特判）。
    Bool {
        value: bool,
        span: Span,
    },
}

/// 结构体模式的一个字段：`name` 或 `name: pattern`。
#[derive(Debug, Clone)]
pub struct StructPatternField {
    pub id: NodeId,
    pub span: Span,
    pub name: Ident,
    /// `None` 为简写形式 `Point { x }`（等价于 `x: x`，语义由后续阶段处理）。
    pub pattern: Option<Pattern>,
}
