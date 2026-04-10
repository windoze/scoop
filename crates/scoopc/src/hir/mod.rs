//! HIR（High-level IR）。
//!
//! HIR 的定位：在 parser 产出的 AST 之上，引入一棵**已解析（name resolved）且已类型化（typed）**
//! 的中间表示，用于后续阶段：
//! - MIR lowering（显式控制流/临时变量）
//! - 单态化（monomorphization）
//! - LLVM codegen
//!
//! 当前阶段（TODO T0701）先落地“数据结构骨架 + 最小可用的 lowering”，用于：
//! - `scoop dump-hir <file>` 输出 Debug 视图，便于后续迭代与 fixtures 回归
//!
//! 注意：本模块尚未接入完整 typecheck/infer，因此未覆盖的表达式/语句会以 `Any` 作为类型占位，
//! 并用 `Todo(...)` 节点保留结构位置，避免 `panic!()` 阻断调试。

mod lower;
pub use lower::mangle_nominal_fqn;

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

use crate::ast;
use crate::span::Span;
use crate::ty::TypeId;

pub use lower::{
    HirLowerError, LoweredHir, lower_for_compilation_unit, lower_for_compilation_unit_multi_files,
    lower_for_dump,
};
pub(crate) use lower::{LoweringInputs, lower_fun_with_type_bindings};

/// HIR 中引用一个“已解析的符号”的稳定标识。
///
/// 说明：
/// - 当前阶段它主要用于把 AST 中的 ident 引用（`x`/`foo`）绑定到某个“解析结果”（local/top-level）；
/// - 后续若引入真正的全局 symbol table / 增量编译缓存，可把该 ID 扩展为跨 session 稳定的形式。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(u32);

impl SymbolId {
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for SymbolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "S{}", self.0)
    }
}

/// 一个 closure（lambda）在 HIR 中的稳定标识。
///
/// 说明：
/// - 该 ID 仅在“单文件 lowering”的 HIR 中稳定（用于 dump/fixtures 的可回归输出）；
/// - 后续若引入跨文件/跨 session 的 closure 符号表，可替换为更稳定的形式。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClosureId(u32);

impl ClosureId {
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for ClosureId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "C{}", self.0)
    }
}

/// 一个源文件 lowering 后的 HIR。
#[derive(Debug, Clone)]
pub struct File {
    pub items: Vec<Item>,
}

/// 顶层条目（top-level items）。
#[derive(Debug, Clone)]
pub enum Item {
    Fun(FunDecl),
    Val(ValDecl),
    /// 未纳入当前阶段 HIR 的条目占位（例如 type/object/typealias 等）。
    Todo {
        span: Span,
        kind: &'static str,
    },
}

/// 函数声明（顶层或方法；当前阶段只做顶层 fun 的最小承载）。
#[derive(Clone)]
pub struct FunDecl {
    pub span: Span,
    pub fqn: String,
    pub name: String,
    /// 声明所在源文件路径；供多文件 codegen 在需要回查源码文本时确定所属文件。
    pub source_path: PathBuf,
    /// 是否为 `const fun`（spec §6.2）。
    pub is_const: bool,
    /// 函数本身的类型（函数类型）。
    pub ty: TypeId,
    pub params: Vec<Param>,
    pub return_ty: TypeId,
    pub body: Option<Block>,
}

impl fmt::Debug for FunDecl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 说明：
        // - 该 Debug 输出用于 `scoop dump-hir` 与 `tests/fixtures/hir/*.hir` golden 回归；
        // - 为保持现有输出稳定，`is_const=false` 时不打印该字段；
        // - 当 `is_const=true` 时才显式输出，便于后续 comptime/解释器阶段接入。
        let mut s = f.debug_struct("FunDecl");
        s.field("span", &self.span);
        s.field("fqn", &self.fqn);
        s.field("name", &self.name);
        if self.is_const {
            s.field("is_const", &true);
        }
        s.field("ty", &self.ty);
        s.field("params", &self.params);
        s.field("return_ty", &self.return_ty);
        s.field("body", &self.body);
        s.finish()
    }
}

/// 函数参数（HIR 视图）。
#[derive(Debug, Clone)]
pub struct Param {
    pub span: Span,
    pub id: SymbolId,
    pub name: String,
    pub ty: TypeId,
}

/// `val`/`var` 声明（顶层或局部）。
#[derive(Debug, Clone)]
pub struct ValDecl {
    pub span: Span,
    pub id: Option<SymbolId>,
    pub name: Option<String>,
    pub mutable: bool,
    pub ty: TypeId,
    pub init: Option<Expr>,
}

/// 表达式块（block expression）。
#[derive(Debug, Clone)]
pub struct Block {
    pub span: Span,
    pub ty: TypeId,
    pub stmts: Vec<Stmt>,
}

/// 语句（statement）。
#[derive(Debug, Clone)]
pub struct Stmt {
    pub span: Span,
    pub ty: TypeId,
    pub kind: StmtKind,
}

#[derive(Debug, Clone)]
pub enum StmtKind {
    Empty,
    Expr(Expr),
    Val(ValDecl),
    /// 赋值语句：`lhs = rhs`。
    ///
    /// 说明：虽然 parser 在 AST 中以 `ExprKind::Assign` 承载该语法，但在 HIR 中我们把它视为语句，
    /// 便于后续 MIR lowering 生成显式的 store/写回语义。
    Assign {
        lhs: Expr,
        eq_span: Span,
        rhs: Expr,
    },
    /// `while (cond) { ... }`。
    While {
        cond: Expr,
        body: Block,
    },
    /// `break`（当前阶段不支持 label）。
    Break {
        break_span: Span,
    },
    /// `continue`（当前阶段不支持 label）。
    Continue {
        continue_span: Span,
    },
    Return {
        value: Option<Expr>,
    },
    Todo(&'static str),
}

/// 表达式（expression）。
#[derive(Debug, Clone)]
pub struct Expr {
    pub span: Span,
    pub ty: TypeId,
    pub kind: ExprKind,
}

/// struct literal 的字段初始化项（HIR 视图）。
#[derive(Debug, Clone)]
pub struct StructLitField {
    pub span: Span,
    pub name: String,
    pub name_span: Span,
    pub colon_span: Span,
    pub value: Expr,
}

/// 插值字符串的片段（spec §8.2）。
#[derive(Debug, Clone)]
pub enum InterpolatedStringPart {
    /// 纯文本片段（保留源码 span；转义/去重写回等语义由后续阶段决定）。
    Text { span: Span },
    /// 插值表达式片段：`{ expr }`。
    Expr { expr: Expr },
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Missing,
    Literal(LiteralKind),
    VarRef(ValueRef),
    /// 未能在 resolver 阶段绑定的标识符。
    ///
    /// 典型场景：enum variant ctor `Some(1)` / 0-参数 variant 值 `None`。
    /// - resolver 会对 `Call(Ident)` 的 callee 允许“未 resolve”（把更贴近语义的诊断留给 typecheck）；
    /// - HIR 为了保持结构可回归，需要保留该名字，供后续 lowering/codegen 在“期望类型语境”下判定含义。
    UnresolvedIdent {
        name: String,
    },
    /// struct literal：`TypeName { field: expr, ... }`。
    StructLit {
        ty: TypeId,
        fields: Vec<StructLitField>,
    },
    /// tuple 字面量：`(a, b, ...)`（spec §2.3.3）。
    ///
    /// 说明：
    /// - `()`（Unit）在 HIR 中用 `Literal(Unit)` 表示；
    /// - 该节点仅用于“可回归的早期 codegen/HIR dump”，更完整的 tuple lowering 见后续任务。
    TupleLit {
        elements: Vec<Expr>,
    },
    /// 插值字符串：`f"Hello, {name}!"` / `f"""...{x}..."""`（spec §8.2/§8.3）。
    ///
    /// 说明：
    /// - HIR 保留 parser 拆分后的 Text/Expr 片段列表；
    /// - 当前阶段 codegen 直接把它 lowering 为“拼接后的 runtime `ScoopString`”。
    InterpolatedString {
        /// 是否为 raw f-string（`f"""..."""`）。
        raw: bool,
        parts: Vec<InterpolatedStringPart>,
    },
    /// 前缀一元运算：`!expr` / `-expr` / `~expr`（spec §2.3.4）。
    Unary {
        op: ast::UnaryOp,
        op_span: Span,
        expr: Box<Expr>,
    },
    /// 二元运算表达式：`lhs op rhs`（spec §2.3.4）。
    Binary {
        lhs: Box<Expr>,
        op: ast::BinaryOp,
        op_span: Span,
        rhs: Box<Expr>,
    },
    /// 运行期类型判断：`expr is Type` / `expr !is Type`。
    ///
    /// 说明：
    /// - typecheck 阶段允许其出现在 `if/when` 等条件位置用于 smart cast；
    /// - codegen 阶段需要将其落到运行期对象模型（type descriptor / itable）检查。
    TypeCheck {
        expr: Box<Expr>,
        op: ast::TypeCheckOp,
        op_span: Span,
        target_ty: TypeId,
    },
    /// 显式运行期转换：`expr as Type` / `expr as? Type`。
    ///
    /// 说明：
    /// - `as` 失败语义：`Raise.raise(RuntimeError.ClassCastFailed)`；
    /// - `as?` 失败语义：返回 `None`（即 `Option<T>`）。
    Cast {
        expr: Box<Expr>,
        op: ast::CastOp,
        op_span: Span,
        target_ty: TypeId,
    },
    Block(Block),
    /// closure（lambda）表达式：`{ params -> body }` / `{ body }`。
    ///
    /// 当前阶段（T0711）：
    /// - lowering 会计算 capture set（自由变量集合）并写入 `ClosureExpr.captures`；
    /// - env struct 的具体布局与 codegen 表示仍在后续任务逐步补齐（此处只保留捕获列表）。
    Closure(ClosureExpr),
    /// `if (cond) thenExpr else elseExpr?`（表达式形式）。
    If {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Option<Box<Expr>>,
    },
    /// `when (subject) { pat -> expr; ... }`（表达式形式）。
    When {
        subject: Box<Expr>,
        arms: Vec<WhenArm>,
    },
    /// 成员访问表达式：`receiver.member`。
    ///
    /// 说明：
    /// - 该节点用于承载 resolver 写回的成员绑定结果（字段/方法/扩展成员的 FQN）；便于后续 MIR lowering/codegen；
    /// - safe-call（`?.`）等更复杂形态将在后续任务中补齐。
    MemberAccess {
        receiver: Box<Expr>,
        member: MemberAccess,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<CallArg>,
    },
    /// effect operation 调用（spec §5.2/§5.4）。
    ///
    /// 说明：
    /// - 在 AST 中该形态通常表现为 `Effect.op(args...)` 的调用表达式；
    /// - 在后续 lowering（MIR/effect lowering）中会拥有特殊控制流语义（非普通函数调用）。
    Perform {
        op: EffectOpRef,
        args: Vec<CallArg>,
    },
    /// effect handler 表达式：`handle { ... } with { ... }`（spec §5.4）。
    ///
    /// 当前阶段：
    /// - 支持 non-resuming arms（`->`）与 immediate-resume arms（`-> resume`，T0616）；
    /// - escape continuation（`, k ->`）相关字段留待后续任务补齐。
    Handle(HandleExpr),
    Todo(&'static str),
}

/// closure（lambda）捕获的一个外部局部变量（free variable）。
///
/// 说明：
/// - 这里的“外部”指相对于该 lambda 自身参数与内部局部声明而言；
/// - 当前阶段仅记录 `SymbolId + name + decl_span`，供后续 env layout / codegen 使用；
/// - 可变捕获（`var`）在 lowering 时会被标记为 `mutable: true`，并在 MIR lowering（T0714）
///   侧通过“捕获 box / 读写经由 box”来实现别名语义。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    pub id: SymbolId,
    pub name: String,
    pub decl_span: Span,
    pub mutable: bool,
}

/// closure（lambda）的 HIR 表示（TODO T0710）。
#[derive(Debug, Clone)]
pub struct ClosureExpr {
    pub span: Span,
    pub id: ClosureId,
    /// 捕获的外部局部变量集合（按 decl span 排序，便于稳定 dump/fixtures）。
    pub captures: Vec<Capture>,
    pub params: Vec<Param>,
    pub body: Box<Expr>,
}

/// 一个 effect operation 的“引用”（以 FQN 表示）。
///
/// 说明：该结构主要用于 HIR dump/fixtures 的稳定输出；后续可替换为更结构化的 symbol 引用。
#[derive(Debug, Clone)]
pub struct EffectOpRef {
    pub span: Span,
    pub fqn: String,
}

#[derive(Debug, Clone)]
pub enum LiteralKind {
    /// Integer literal resolved on demand from source text (`Expr.span` + source provenance).
    Int,
    Float64(f64),
    Float32(f32),
    /// Char literal fully parsed during HIR lowering so later phases do not need to re-slice source text.
    Char(char),
    /// String literal resolved on demand from source text (`Expr.span` + source provenance).
    String,
    Unit,
    Bool(bool),
    /// Synthesized integer literal (compiler-generated desugaring, e.g., for-loop index init/step).
    /// Always typed as Int (i64 signed).
    SynthInt(i64),
}

/// 已解析的“值引用”（local/top-level）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueRef {
    Local {
        id: SymbolId,
        name: String,
        decl_span: Span,
    },
    TopLevel {
        id: SymbolId,
        fqn: String,
    },
}

/// 成员访问中的“已解析引用”（来自 resolver 写回的 `ResolvedMemberRef`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberRef {
    Value { id: SymbolId, fqn: String },
    Fun { id: SymbolId, fqn: String },
    ExtensionValue { id: SymbolId, fqn: String },
    ExtensionFun { id: SymbolId, fqn: String },
}

/// 成员访问中的标识符（`receiver.member`）。
///
/// 说明：
/// - `span/name` 用于保持 dump/fixtures 输出稳定且可读；
/// - `resolved` 为空时表示 resolver 未能给出绑定结果（仍保留结构以避免 panic）。
#[derive(Debug, Clone)]
pub struct MemberAccess {
    pub span: Span,
    pub name: String,
    pub resolved: Option<MemberRef>,
}

/// 调用实参（位置参数或命名参数）。
#[derive(Debug, Clone)]
pub enum CallArg {
    Positional(Expr),
    Named {
        name: String,
        name_span: Span,
        value: Expr,
    },
}

/// 一个 struct（值类型）的“字段布局”信息（早期用于 LLVM codegen）。
///
/// 说明：
/// - 当前后端只需要“字段顺序 + 字段类型”，以便对字段生成稳定的 GEP 索引；
/// - padding/对齐由 LLVM data layout 决定（TODO T0811 目标）。
#[derive(Debug, Clone)]
pub struct StructLayout {
    pub fqn: String,
    pub fields: Vec<StructFieldLayout>,
    /// `@CLayout(aligned, packed)`（spec §15.5.2）附加布局信息（仅对 GC-free struct 有意义）。
    ///
    /// 说明：
    /// - 该字段为 side table 信息：不影响 `dump-hir` 的输出稳定性；
    /// - 具体 ABI 规则由 typecheck 做门禁，后端（LLVM）根据这些参数生成 packed/aligned 行为。
    pub c_layout: Option<StructCLayout>,
}

/// `@CLayout(aligned, packed)` 的最小后端视图（供 LLVM codegen 使用）。
#[derive(Debug, Clone, Copy)]
pub struct StructCLayout {
    /// 显式指定的结构体整体对齐（单位：字节）。`None` 表示未指定（使用默认 ABI）。
    pub aligned: Option<u32>,
    /// pack 值（单位：字节）。`None` 表示未指定；支持 1/2/4/8/16（`#pragma pack(N)` 语义）。
    pub packed: Option<u32>,
}

/// struct 的单个字段布局信息。
#[derive(Debug, Clone)]
pub struct StructFieldLayout {
    pub span: Span,
    pub name: String,
    pub fqn: String,
    /// 字段类型的 FQN（当前仅对 `TypeRef::Path` 可解析；其它类型留空）。
    pub ty_fqn: Option<String>,
}

/// `struct FQN -> StructLayout` 的索引（由 HIR lowering 构建，供后端查询）。
pub type StructLayoutIndex = HashMap<String, StructLayout>;

/// 一个 enum（值类型）的“布局信息”（早期用于 LLVM codegen）。
///
/// 说明：
/// - 当前阶段（T0813）只需要 `{tag, payload}` 的最小表示，因此只保留：
///   - variant 顺序与 tag（按声明顺序分配，从 0 开始）；
///   - payload 字段的类型（仅对 `TypeRef::Path` 可解析；其它类型留空）。
/// - 更复杂的布局策略（niche / boxing / size disparity lint）留给后续任务（T0826）。
#[derive(Debug, Clone)]
pub enum EnumRepr {
    /// 常规 rich enum：tagged union（tag 由编译器按声明顺序分配）。
    TaggedUnion,
    /// value-only enum：ABI/内存布局与某个整型标量完全一致（spec §2.3.2.1）。
    ValueOnly {
        /// 底层整型类型的 FQN（例如 `scoop.core.Int` / `scoop.core.UInt8`）。
        underlying_ty_fqn: Option<String>,
    },
}

/// enum 的布局摘要（供后端查询）。
#[derive(Debug, Clone)]
pub struct EnumLayout {
    pub fqn: String,
    pub repr: EnumRepr,
    pub variants: Vec<EnumVariantLayout>,
}

/// enum 的一个 variant 的布局信息。
#[derive(Debug, Clone)]
pub struct EnumVariantLayout {
    pub span: Span,
    pub name: String,
    /// variant 的判别值：
    /// - 对于 `EnumRepr::TaggedUnion`：按声明顺序分配的 tag（从 0 开始）；
    /// - 对于 `EnumRepr::ValueOnly`：来自源码 `A = 0` 的显式判别值（以 u64 bits 存储）。
    pub tag: u64,
    pub fields: Vec<EnumVariantFieldLayout>,
}

/// enum variant 的一个字段布局信息。
#[derive(Debug, Clone)]
pub struct EnumVariantFieldLayout {
    pub span: Span,
    pub name: String,
    /// 字段类型的 FQN（当前仅对 `TypeRef::Path` 可解析；其它类型留空）。
    pub ty_fqn: Option<String>,
}

/// `enum FQN -> EnumLayout` 的索引（由 HIR lowering 构建，供后端查询）。
pub type EnumLayoutIndex = HashMap<String, EnumLayout>;

/// `object` / `companion object` 的初始化信息索引（Appendix B.9）。
///
/// 说明：
/// - 该索引不影响 `dump-hir` 的输出稳定性（只作为后端 side table 使用）；
/// - 当前阶段只收集 object 体内的“可观测初始化副作用”：
///   - 属性 backing field 的 init 表达式（`val/var x = expr`）
///   - `init { ... }` 初始化块
pub type ObjectInitIndex = HashMap<String, ObjectInit>;

/// `class` 的初始化信息索引（Appendix B.2.2）。
///
/// 说明：
/// - 与 `ObjectInitIndex` 类似，该索引作为后端 side table 使用，不影响 `dump-hir` 的输出稳定性；
/// - 当前阶段主要用于 LLVM 后端在“构造调用点”内执行 Kotlin-like 初始化顺序：
///   - 先初始化 primary ctor 的 `val/var` 参数属性；
///   - 再按源码顺序执行 property initializer 与 `init {}` blocks；
///   - 最后执行 secondary ctor body（若调用点选择了 secondary ctor）。
pub type ClassInitIndex = HashMap<String, ClassInit>;

/// 一个 class 的初始化顺序、字段信息与构造器集合。
#[derive(Debug, Clone)]
pub struct ClassInit {
    pub fqn: String,
    /// class 声明所在源文件路径；供多文件 codegen 在初始化表达式中回查字面量源码。
    pub source_path: PathBuf,
    /// 直接 superclass 的 FQN（仅 class 单继承；interface 不在此处记录）。
    pub super_class_fqn: Option<String>,
    /// class header 的 super ctor args 括号 span（若存在 `: Base(...)`）。
    pub super_ctor_args_span: Option<Span>,
    /// class header 的 super ctor args（若存在 `: Base(args...)`）。
    pub super_ctor_args: Vec<Expr>,
    /// `this` 在该 class 初始化语境中的局部符号 ID（resolver 用 class name span 作为 decl_span）。
    pub this_id: SymbolId,
    /// class 实例的字段列表（按稳定顺序，用于后端分配 layout）。
    pub fields: Vec<ClassField>,
    /// `field fqn -> fields[] index` 的快速索引。
    pub field_indices: HashMap<String, u32>,
    /// primary ctor 的初始化步骤（按源码顺序执行；不包含 ctor 参数属性赋值）。
    pub steps: Vec<ClassInitStep>,
    /// 该 class 的构造器集合（primary + secondary）。
    ///
    /// 说明：当前阶段用它来在 codegen 时按“参数形状”选择要执行的 ctor。
    pub ctors: Vec<ClassCtor>,
}

/// class 的一个字段（最小后端视图）。
#[derive(Debug, Clone)]
pub struct ClassField {
    pub fqn: String,
    pub name: String,
    pub mutable: bool,
    pub ty: TypeId,
}

/// class 初始化的一步（按源码顺序执行）。
#[derive(Debug, Clone)]
pub enum ClassInitStep {
    /// 执行某个 property 的 initializer，并写入字段。
    PropertyInit { field_fqn: String, init: Expr },
    /// 执行 `init { ... }` block。
    InitBlock { block: Block },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassCtorKind {
    Primary,
    Secondary,
}

/// 一个 class 构造器（primary 或 secondary）的最小后端视图。
#[derive(Debug, Clone)]
pub struct ClassCtor {
    pub kind: ClassCtorKind,
    pub span: Span,
    pub params: Vec<ClassCtorParam>,
    /// secondary ctor 的 delegation（`this(...)` / `super(...)`）；primary ctor 为 None。
    pub delegation: Option<ClassCtorDelegation>,
    /// ctor body：secondary ctor 为 Some；primary ctor 为 None（其执行体由 `steps` 描述）。
    pub body: Option<Block>,
}

#[derive(Debug, Clone)]
pub struct ClassCtorDelegation {
    pub kind: ast::CtorDelegationKind,
    pub span: Span,
    pub args: Vec<Expr>,
}

#[derive(Debug, Clone)]
pub struct ClassCtorParam {
    pub id: SymbolId,
    pub name: String,
    pub decl_span: Span,
    pub ty: TypeId,
    pub has_default: bool,
    /// 该参数是否同时声明为 `val/var` 参数属性（仅 primary ctor 适用）。
    pub is_property: bool,
    /// 当 `is_property=true` 时，该属性对应的字段 FQN。
    pub property_field_fqn: Option<String>,
}

/// 调用点的“构造候选集合”索引：callee span → candidate type fqns。
///
/// 说明：
/// - resolver 会在 `ValueIdent.call` 中写回 call candidates（T0319），但 HIR v0 仍会把 ctor 调用
///   的 callee 降为 `UnresolvedIdent`，以保持 dump 输出稳定；
/// - LLVM codegen 需要知道“该 UnresolvedIdent 实际上是 ctor 调用”，因此这里把候选集合以 side table
///   的形式保留下来。
pub type CtorCallSiteIndex = HashMap<Span, Vec<String>>;

/// 外部函数（`@Extern`）的最小后端视图。
///
/// 说明：
/// - 该信息作为后端 side table 保存，不影响 `dump-hir` 的输出稳定性；
/// - 当前阶段（T1006）仅支持 C ABI；
/// - `symbol` 为最终参与链接的符号名（例如 `@Extern("puts")` / `@Extern("scoop_println")`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternFun {
    pub abi: ExternAbi,
    pub symbol: String,
    /// `@CallingConvention("...")`（可选）：用于覆盖默认 C ABI（spec §15.5.4）。
    ///
    /// 说明：当前阶段后端只保证 `c/cdecl`；其它 calling convention 的支持留待后续扩展。
    pub calling_convention: Option<String>,
    /// `@Extern(lib = "...")`（可选）：需要链接的外部库名（传递给链接器作为 `-l<name>`）。
    ///
    /// 说明：链接阶段同时通过 `LoweredHir.extern_libs`（由 `collect_extern_libs` 收集的去重列表）
    /// 将所有 lib 传递给链接器。此字段记录单个函数关联的 lib，用于诊断与追溯。
    pub lib: Option<String>,
}

/// `@Extern` 的 ABI 约定（当前阶段只落地 C ABI）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternAbi {
    C,
}

/// `fun FQN -> ExternFun` 的索引（由 HIR lowering 构建，供后端查询）。
pub type ExternFunIndex = HashMap<String, ExternFun>;

/// 顶层可变全局变量（`@ThreadLocal` / `@Global`）的最小后端视图（TODO T1023）。
///
/// 说明：
/// - 这些变量在 typecheck 阶段已被门禁为 GC-free，因此当前不需要参与 GC roots 扫描；
/// - 早期阶段我们只要求“可生成静态存储并在函数内读写”，更复杂的初始化语义可后续补齐。
#[derive(Debug, Clone)]
pub struct TopLevelVar {
    pub fqn: String,
    /// 声明所在源文件路径；供静态 initializer 在 codegen 期解析源码字面量。
    pub source_path: PathBuf,
    pub span: Span,
    pub storage: TopLevelVarStorage,
    pub ty: TypeId,
    /// initializer（可选）：用于 codegen 阶段决定是否能落到静态常量初始化。
    pub init: Option<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopLevelVarStorage {
    /// `@ThreadLocal`：每个 OS 线程拥有独立的存储实例（TLS）。
    ThreadLocal,
    /// `@Global`：进程全局静态存储（所有线程共享）。
    Global,
}

/// `var FQN -> TopLevelVar` 的索引（由 HIR lowering 构建，供后端查询）。
pub type TopLevelVarIndex = HashMap<String, TopLevelVar>;

/// 顶层 `const val` 的最小后端视图。
///
/// 说明：
/// - 当前只为 LLVM codegen 提供“按声明类型内联 initializer”所需的信息；
/// - 保持独立 side table，避免把 `source_path` / `is_const` 等后端细节塞回通用 `ValDecl`。
#[derive(Debug, Clone)]
pub struct TopLevelConst {
    pub fqn: String,
    /// 声明所在源文件路径；供多文件 codegen 在回查字面量原文时切换 source context。
    pub source_path: PathBuf,
    pub span: Span,
    pub ty: TypeId,
    pub init: Option<Expr>,
}

/// `const val FQN -> TopLevelConst` 的索引（由 HIR lowering 构建，供后端查询）。
pub type TopLevelConstIndex = HashMap<String, TopLevelConst>;

/// 一个 object（含 companion object）的初始化顺序与成员信息。
#[derive(Debug, Clone)]
pub struct ObjectInit {
    pub fqn: String,
    /// object 声明所在源文件路径；供初始化表达式 codegen 回查源码字面量。
    pub source_path: PathBuf,
    /// object 体内声明的属性（按 name 索引）。
    pub properties: HashMap<String, ObjectProperty>,
    /// 初始化步骤（按源码顺序稳定化，用于一次初始化 codegen）。
    pub steps: Vec<ObjectInitStep>,
}

/// object 的一个属性声明信息（最小后端视图）。
#[derive(Debug, Clone)]
pub struct ObjectProperty {
    pub name: String,
    pub mutable: bool,
    pub ty: TypeId,
    pub has_init: bool,
}

/// object 一次初始化的步骤（按源码顺序执行）。
#[derive(Debug, Clone)]
pub enum ObjectInitStep {
    /// 执行 `val/var name = init` 的 init 表达式，并写入 backing storage。
    PropertyInit { name: String, init: Expr },
    /// 执行 `init { ... }` 块。
    InitBlock { block: Block },
}

/// `when` 的一个分支（arm）：`pat (if guard)? -> body`。
#[derive(Debug, Clone)]
pub struct WhenArm {
    pub span: Span,
    pub pat: WhenPat,
    pub guard: Option<Expr>,
    pub arrow_span: Span,
    pub body: Expr,
}

/// `when` 分支的模式（早期最小子集；后续会与通用 Pattern 统一）。
#[derive(Debug, Clone)]
pub enum WhenPat {
    Else {
        span: Span,
    },
    /// or-pattern：`A | B | C`
    Or {
        span: Span,
        pats: Vec<WhenPat>,
    },
    /// `_`：通配符模式（匹配任意值）。
    Wildcard {
        span: Span,
    },
    /// rest：`..`（忽略剩余字段/元素；仅允许出现在 tuple/variant pattern 内）。
    Rest {
        span: Span,
    },
    /// `is Type`。
    Is {
        span: Span,
        ty: TypeId,
    },
    /// 绑定变量模式：`x`（把匹配到的值绑定到变量 `x`）。
    Bind {
        span: Span,
        id: SymbolId,
        name: String,
    },
    /// tuple 模式：`(p1, p2, ...)`。
    Tuple {
        span: Span,
        elements: Vec<WhenPat>,
    },
    /// enum variant 模式：`Some(x)` / `None`（0 参数 variant）。
    Variant {
        span: Span,
        name_span: Span,
        name: String,
        args: Vec<WhenPat>,
    },
    IntLit {
        span: Span,
    },
    CharLit {
        span: Span,
        value: char,
    },
    StringLit {
        span: Span,
    },
    BoolLit {
        span: Span,
        value: bool,
    },
}

impl WhenPat {
    pub fn span(&self) -> Span {
        match self {
            WhenPat::Else { span } => *span,
            WhenPat::Or { span, .. } => *span,
            WhenPat::Wildcard { span } => *span,
            WhenPat::Rest { span } => *span,
            WhenPat::Is { span, .. } => *span,
            WhenPat::Bind { span, .. } => *span,
            WhenPat::Tuple { span, .. } => *span,
            WhenPat::Variant { span, .. } => *span,
            WhenPat::IntLit { span, .. } => *span,
            WhenPat::CharLit { span, .. } => *span,
            WhenPat::StringLit { span } => *span,
            WhenPat::BoolLit { span, .. } => *span,
        }
    }
}

/// `handle` 表达式（HIR 视图）。
#[derive(Debug, Clone)]
pub struct HandleExpr {
    pub body: Block,
    pub arms: Vec<HandleArm>,
    pub finally: Option<Block>,
}

/// `handle` 的一个 handler arm（HIR 视图）。
#[derive(Clone)]
pub struct HandleArm {
    pub span: Span,
    pub op: HandleOp,
    pub kind: HandleArmKind,
    pub body: Expr,
}

impl fmt::Debug for HandleArm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 为了保持 HIR fixtures 的 dump 输出尽量稳定：
        // - non-resuming arm 不额外打印 kind（与旧版输出保持一致）
        // - `-> resume` arm 仅在必要时打印 resume symbol 以便回归与调试
        let mut s = f.debug_struct("HandleArm");
        s.field("span", &self.span);
        s.field("op", &self.op);
        match self.kind {
            HandleArmKind::ImmediateResume { resume } => {
                s.field("resume", &resume);
            }
            HandleArmKind::EscapeContinuation { continuation } => {
                s.field("continuation", &continuation);
            }
            HandleArmKind::NonResuming => {}
        }
        s.field("body", &self.body);
        s.finish()
    }
}

/// handler arm 的语义形态（spec §5.4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleArmKind {
    /// `->`：非恢复 arm；handled computation 被放弃（try/catch lowering 产物）。
    NonResuming,
    /// `-> resume`：立即恢复 arm（T0616）。
    ///
    /// `resume(value)` 是一个隐式注入的局部符号：其 `SymbolId` 存在于本字段中，
    /// 供后续 lowering/codegen 识别并生成 state machine 跳转。
    ImmediateResume { resume: SymbolId },
    /// `, k ->`：逃逸 continuation arm（T0617）。
    ///
    /// `k.resume(value)` 会在后续 lowering/codegen 中生成对 runtime continuation 的调用。
    EscapeContinuation { continuation: SymbolId },
}

/// handler arm head 中的 effect operation：`Effect.op(binders...)`（HIR 视图）。
#[derive(Debug, Clone)]
pub struct HandleOp {
    pub span: Span,
    pub effect_ty: TypeId,
    pub op: EffectOpRef,
    pub binders: Vec<HandleBinder>,
}

/// handler arm 的一个参数 binder：`name` 或 `name: Type`（HIR 视图）。
#[derive(Debug, Clone)]
pub struct HandleBinder {
    pub span: Span,
    pub id: SymbolId,
    pub name: String,
    pub ty: TypeId,
}
