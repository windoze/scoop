//! 类型引用（type refs）与 effect 行（§6、§6.1、§5.2）。

use scoop2_base::{NodeId, Span};

use super::TypePath;

/// 类型引用（§6）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TypeRef {
    pub id: NodeId,
    pub span: Span,
    pub kind: TypeRefKind,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TypeRefKind {
    /// 路径类型：`List<Int>`、`scoop.core.Option<T>`。
    Path { path: TypePath, args: Vec<TypeArg> },
    /// Unit 类型 `()`（即 0 元组）。
    Unit,
    /// 元组类型：`(A, B)`；1 元组写作 `(A,)`。
    ///
    /// 分组 `(T)` 是透明的，parser 直接返回内层 `T`，不产生节点。
    Tuple(Vec<TypeRef>),
    /// 函数类型：`(A, B) -> R / Row`（effect 注解在返回类型之后）。
    Function {
        params: Vec<TypeRef>,
        ret: Box<TypeRef>,
        effect: Option<EffectRowExpr>,
    },
    /// 接收者函数类型：`T.(A) -> R`（§6 `receiverFnTail`）。
    ///
    /// `T?.(A) -> R` 中 receiver 是 `Nullable(T)`（`?` 先于 receiver tail 应用）。
    ReceiverFunction {
        receiver: Box<TypeRef>,
        params: Vec<TypeRef>,
        ret: Box<TypeRef>,
        effect: Option<EffectRowExpr>,
    },
    /// 可空类型 `T?`：每个后缀 `?` 恰好包一层 `Option`（§6，spec §2.4）。
    ///
    /// `T??` 是两层嵌套的 `Nullable(Nullable(T))`，不拍平。
    Nullable(Box<TypeRef>),
}

/// 类型实参（§5.2，use site）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TypeArg {
    pub id: NodeId,
    pub span: Span,
    pub kind: TypeArgKind,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TypeArgKind {
    Type(TypeRef),
    /// 星投影 `*`（仅存在于类型实参位置）。
    Star,
    /// effect 行实参 `eff Row`（至多一个且必须是最后一个实参）。
    Effect(EffectRowExpr),
}

/// effect 行表达式（§6.1）：`A + B`、`Pure`、`(Row)!`。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EffectRowExpr {
    pub id: NodeId,
    pub span: Span,
    /// `+` 连接的项（至少一个）。
    ///
    /// `Pure` 不特判：它就是单段路径文本为 `Pure` 的普通项（text match，
    /// 语义由后续阶段解释为空行）。
    pub terms: Vec<EffectRowTerm>,
    /// 尾部 `!`（闭合行，spec §5.8.4）的 span。
    ///
    /// 绑定到**整行**而非最后一项（优先级低于 `+`）。
    pub closed: Option<Span>,
}

/// effect 行的一项：`pathType`（可带类型实参，如 `Raise<IOError>`）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EffectRowTerm {
    pub id: NodeId,
    pub span: Span,
    pub path: TypePath,
    pub args: Vec<TypeArg>,
}
