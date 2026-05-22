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

mod dump;
mod lower;
mod stable_closure;
pub(crate) use dump::stable_dump_file;
pub use lower::mangle_nominal_fqn;

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::PathBuf;

use crate::ast;
use crate::span::Span;
use crate::ty::TypeId;

pub(crate) use lower::GenericTemplateSymbolSuffixIndex;
pub(crate) use lower::lower_generic_for_compilation_unit_multi_files_with_type_env;
pub use lower::{
    ExplicitMirInstanceLoweringOptions, HirLowerError, HirStageError, LoweredHir,
    lower_for_compilation_unit, lower_for_compilation_unit_multi_files,
    lower_for_compilation_unit_multi_files_with_explicit_mir_instances,
    lower_for_compilation_unit_multi_files_with_type_env,
    lower_for_compilation_unit_with_stable_cone_key, lower_for_dump, lower_typed_for_dump,
};
pub(crate) use lower::{
    LoweringInputs, canonical_generic_fun_signature_key,
    canonical_generic_property_getter_signature_key,
    generic_template_symbol_suffixes_for_compilation_unit,
};
pub(crate) use lower::{collect_generic_template_symbol_suffixes, stable_instance_fqn};
pub(crate) use stable_closure::{
    stable_closure_lexical_path_in_expr, stable_closure_lexical_path_in_fun,
};

pub(crate) fn lower_fun_with_type_bindings_and_mir_facts(
    inputs: LoweringInputs<'_>,
    fun: &ast::FunDecl,
    type_bindings: impl IntoIterator<Item = (String, TypeId)>,
) -> lower::LoweredFunWithMirFacts {
    lower::lower_fun_with_type_bindings_and_mir_facts(inputs, fun, type_bindings)
}

/// HIR/generic MIR 中用于承载 `<eff E>` row 变量的内部占位 `decl_file`。
///
/// 说明：
/// - 该占位仍复用 `TypeKind::Param` 的显示路径，因此 dump-hir / dump-mir 会稳定显示 `E`；
/// - 但它不是普通 type param，实例化时必须按 effect-row 语义展开为一整行 `EffectRow`。
pub(crate) const EFFECT_ROW_PARAM_DECL_FILE: &str = "<hir-effect-row-param>";

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

    pub(crate) const fn from_raw(raw: u32) -> Self {
        Self(raw)
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
#[derive(Clone)]
pub struct File {
    pub decls: Vec<Decl>,
    pub items: Vec<Item>,
}

impl fmt::Debug for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("File");
        if !self.decls.is_empty() {
            s.field("decls", &self.decls);
        }
        s.field("items", &self.items);
        s.finish()
    }
}

/// HIR declaration graph entry for non-function top-level declarations.
#[derive(Debug, Clone)]
pub enum Decl {
    TypeAlias(TypeAliasDecl),
    Nominal(NominalDecl),
    Object(ObjectDecl),
    ExtensionProperty(ExtensionPropertyDecl),
}

/// Declaration-site type parameter metadata retained by HIR.
#[derive(Debug, Clone)]
pub struct DeclTypeParam {
    pub span: Span,
    pub name: String,
    pub variance: Option<ast::TypeParamVariance>,
    pub ty: TypeId,
}

#[derive(Debug, Clone)]
pub struct TypeAliasDecl {
    pub span: Span,
    pub fqn: String,
    pub name: String,
    pub type_params: Vec<DeclTypeParam>,
    pub ty: TypeId,
}

#[derive(Debug, Clone)]
pub struct NominalDecl {
    pub span: Span,
    pub fqn: String,
    pub name: String,
    pub kind: ast::TypeKind,
    pub type_params: Vec<DeclTypeParam>,
    pub supertypes: Vec<SupertypeDecl>,
    pub interfaces: Vec<String>,
    pub constructors: Vec<CtorDecl>,
    pub members: Vec<DeclMember>,
}

#[derive(Debug, Clone)]
pub struct ObjectDecl {
    pub span: Span,
    pub fqn: String,
    pub name: String,
    pub kind: ast::ObjectKind,
    pub supertypes: Vec<SupertypeDecl>,
    pub interfaces: Vec<String>,
    pub initializer_root: String,
    pub members: Vec<DeclMember>,
}

#[derive(Debug, Clone)]
pub struct ExtensionPropertyDecl {
    pub span: Span,
    pub fqn: String,
    pub name: String,
    pub mutable: bool,
    pub type_params: Vec<DeclTypeParam>,
    pub receiver_ty: TypeId,
    pub ty: TypeId,
    pub getter: Option<AccessorContract>,
    pub setter: Option<AccessorContract>,
}

#[derive(Debug, Clone)]
pub struct SupertypeDecl {
    pub span: Span,
    pub fqn: Option<String>,
    pub ty: TypeId,
    pub ctor_arg_count: usize,
}

#[derive(Debug, Clone)]
pub enum DeclMember {
    Field(FieldDecl),
    Property(PropertyDecl),
    Fun(MemberFunDecl),
    EnumVariant(EnumVariantDecl),
    InitBlock { span: Span },
    Nested(Decl),
}

#[derive(Debug, Clone)]
pub struct FieldDecl {
    pub span: Span,
    pub fqn: String,
    pub name: String,
    pub mutable: bool,
    pub ty: TypeId,
    pub origin: FieldOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldOrigin {
    PrimaryCtorParam,
    BodyProperty,
    EnumVariantPayload,
}

#[derive(Debug, Clone)]
pub struct PropertyDecl {
    pub span: Span,
    pub fqn: String,
    pub name: String,
    pub mutable: bool,
    pub ty: TypeId,
    pub has_backing_field: bool,
    pub getter: Option<AccessorContract>,
    pub setter: Option<AccessorContract>,
}

#[derive(Debug, Clone)]
pub struct AccessorContract {
    pub span: Span,
    pub fqn: String,
}

#[derive(Debug, Clone)]
pub struct MemberFunDecl {
    pub span: Span,
    pub fqn: String,
    pub name: String,
    pub type_params: Vec<DeclTypeParam>,
    pub params: Vec<CtorParamDecl>,
    pub return_ty: TypeId,
}

#[derive(Debug, Clone)]
pub struct EnumVariantDecl {
    pub span: Span,
    pub fqn: String,
    pub name: String,
    pub fields: Vec<FieldDecl>,
}

#[derive(Debug, Clone)]
pub struct CtorDecl {
    pub span: Span,
    pub kind: ClassCtorKind,
    pub params: Vec<CtorParamDecl>,
    pub delegation: Option<ast::CtorDelegationKind>,
}

#[derive(Debug, Clone)]
pub struct CtorParamDecl {
    pub span: Span,
    pub name: String,
    pub ty: TypeId,
    pub has_default: bool,
    pub property: Option<ast::ValKind>,
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
    /// 函数本身的类型（函数类型）。
    pub ty: TypeId,
    pub params: Vec<Param>,
    pub return_ty: TypeId,
    pub body: Option<Block>,
}

impl fmt::Debug for FunDecl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("FunDecl");
        s.field("span", &self.span);
        s.field("fqn", &self.fqn);
        s.field("name", &self.name);
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
    /// class literal / type metadata literal: `TypeName::class`.
    ///
    /// v0 keeps the runtime value as the stable type-name string while retaining the source type
    /// metadata needed by later stages to upgrade this to a richer `TypeMeta` value.
    ClassLiteral(ClassLiteralExpr),
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
        effect_ty: TypeId,
        op: EffectOpRef,
        args: Vec<CallArg>,
    },
    /// effect handler 表达式：`handle { ... } with { ... }`（spec §5.4）。
    ///
    /// 当前阶段：
    /// - 支持 non-resuming arms（`->`）与 escape-continuation arms（`, k ->`，T0617）；
    /// - HIR 保留 arm 语义形态与显式 continuation binder 符号，供后续 lowering/codegen 识别。
    Handle(HandleExpr),
    Todo(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeMetadataLiteralKind {
    TypeNameString,
}

#[derive(Debug, Clone)]
pub struct ClassLiteralExpr {
    pub source_ty: TypeId,
    pub source_fqn: Option<String>,
    pub metadata_kind: TypeMetadataLiteralKind,
    pub result_ty: TypeId,
}

/// closure（lambda）捕获的一个外部局部变量（free variable）。
///
/// 说明：
/// - 这里的“外部”指相对于该 lambda 自身参数与内部局部声明而言；
/// - 当前阶段仅记录 `SymbolId + name + decl_span`，供后续 env layout / codegen 使用；
/// - 可变捕获（`var`）在 lowering 时会被标记为 `mutable: true`，让 closure body 从 env load
///   后创建的 per-call local 仍可重新绑定；该重新绑定不写回外层局部，也不跨调用持久化。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    pub id: SymbolId,
    pub name: String,
    pub decl_span: Span,
    pub mutable: bool,
}

/// closure（lambda）的 HIR 表示（TODO T0710）。
#[derive(Clone)]
pub struct ClosureExpr {
    pub span: Span,
    pub id: ClosureId,
    pub at_safe_span: Option<Span>,
    /// 捕获的外部局部变量集合（按 decl span 排序，便于稳定 dump/fixtures）。
    pub captures: Vec<Capture>,
    pub params: Vec<Param>,
    pub body: Box<Expr>,
}

impl fmt::Debug for ClosureExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("ClosureExpr");
        s.field("span", &self.span);
        s.field("id", &self.id);
        if let Some(at_safe_span) = self.at_safe_span {
            s.field("at_safe_span", &at_safe_span);
        }
        s.field("captures", &self.captures);
        s.field("params", &self.params);
        s.field("body", &self.body);
        s.finish()
    }
}

/// 一个 effect operation 的“引用”（以 FQN 表示）。
///
/// 说明：该结构主要用于 HIR dump/fixtures 的稳定输出；后续可替换为更结构化的 symbol 引用。
#[derive(Clone)]
pub struct EffectOpRef {
    pub span: Span,
    pub fqn: String,
    pub type_args: Vec<TypeId>,
}

impl fmt::Debug for EffectOpRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("EffectOpRef");
        s.field("span", &self.span);
        s.field("fqn", &self.fqn);
        if !self.type_args.is_empty() {
            s.field("type_args", &self.type_args);
        }
        s.finish()
    }
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
    /// Compiler-generated String literal whose decoded contents do not exist as a quoted source span.
    SynthString(String),
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

/// HIR handoff contract for an assignment statement's left-hand side place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignPlaceContract {
    pub span: Span,
    pub kind: AssignPlaceKind,
    pub place_ty: TypeId,
    pub value_ty: TypeId,
    pub mutable: bool,
    pub write_barrier: ast::AssignWriteBarrierRequirement,
    pub unsafe_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignPlaceKind {
    Local {
        id: SymbolId,
        name: String,
        decl_span: Span,
    },
    TopLevel {
        id: SymbolId,
        fqn: String,
    },
    Member {
        receiver_ty: TypeId,
        owner_fqn: Option<String>,
        member_fqn: String,
        member_name: String,
        member_span: Span,
        resolved: Option<MemberRef>,
    },
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
    /// 字段的真实 TypeId（若 lowering 阶段可恢复）。
    ///
    /// 说明：
    /// - 优先供 LLVM 后端恢复 tuple / nullable 等没有稳定 FQN 文本的字段类型；
    /// - `ty_fqn` 继续保留给早期只识别 nominal/builtin 的兼容路径。
    pub ty: Option<TypeId>,
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
    /// 字段的真实 TypeId（若 lowering 阶段可恢复）。
    ///
    /// 说明：
    /// - 用于让 LLVM 后端识别 tuple / nullable / 其它没有稳定 FQN 文本的 payload 字段；
    /// - `ty_fqn` 继续作为兼容兜底，避免现有 nominal/builtin 路径回退。
    pub ty: Option<TypeId>,
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
    /// class header 的 super ctor 调用绑定（若存在 `: Base(args...)`）。
    pub super_ctor_call: Option<CtorCallInfo>,
    /// class header 的 super ctor args（若存在 `: Base(args...)`）。
    pub super_ctor_args: Vec<CallArg>,
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
    /// 说明：当前阶段用它来在 codegen 时按已发布的 selected-ctor / arg-mapping contract 执行 ctor。
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
    pub call: Option<CtorCallInfo>,
    pub args: Vec<CallArg>,
}

#[derive(Debug, Clone)]
pub struct ClassCtorParam {
    pub id: SymbolId,
    pub name: String,
    pub decl_span: Span,
    pub ty: TypeId,
    pub has_default: bool,
    pub default_value: Option<Expr>,
    /// 该参数是否同时声明为 `val/var` 参数属性（仅 primary ctor 适用）。
    pub is_property: bool,
    /// 当 `is_property=true` 时，该属性对应的字段 FQN。
    pub property_field_fqn: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CtorCallInfo {
    pub class_fqn: String,
    pub ctor_span: Option<Span>,
    /// `arg_mapping[param_idx] = Some(arg_idx)`：该形参由调用点第 `arg_idx` 个显式实参提供；
    /// `None`：该形参由默认值补齐。
    pub arg_mapping: Vec<Option<usize>>,
}

/// 一个调用点在编译单元内的稳定位置键。
///
/// 说明：
/// - 多文件 lowering 中，裸 `Span` 只在“单个源文件内部”唯一；
/// - 因此任何需要跨文件合并的 side table，都必须把 `source_path` 一起作为 key；
/// - 当前主要用于 ctor 调用点与 `Continuation.resume` 调用点。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallSite {
    pub source_path: PathBuf,
    pub span: Span,
}

impl CallSite {
    pub fn new(source_path: impl Into<PathBuf>, span: Span) -> Self {
        Self {
            source_path: source_path.into(),
            span,
        }
    }
}

/// 一个动态 dispatch 调用点在编译单元内的稳定位置键。
///
/// 说明：
/// - `source_path + span` 负责跨文件定位同一源码调用点；
/// - `receiver_ty` 用于区分“同一 generic call-site 在不同单态实例下收敛出的不同 receiver 精确类型”；
/// - 这样 HIR/MIR/LLVM 都能在不回退到名字猜测的前提下，消费显式分类后的 dispatch 调用点。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DispatchCallSite {
    pub source_path: PathBuf,
    pub span: Span,
    pub receiver_ty: TypeId,
}

impl DispatchCallSite {
    pub fn new(source_path: impl Into<PathBuf>, span: Span, receiver_ty: TypeId) -> Self {
        Self {
            source_path: source_path.into(),
            span,
            receiver_ty,
        }
    }
}

/// HIR 上显式记录的动态 dispatch 种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchCallKind {
    Virtual,
    Interface,
}

/// 调用点的 ctor 绑定索引：`source_path + call span` → 已选中的 ctor 调用信息。
///
/// 说明：
/// - HIR v0 仍会把 ctor 调用的 callee 保留为 `UnresolvedIdent`，以保持 dump 输出稳定；
/// - LLVM codegen 需要知道“该调用实际是哪个 ctor、如何把 named/default args 绑定到形参”，
///   因此这里把 typecheck 已确认的绑定结果以 side table 的形式保留下来；
/// - key 使用整个 call expr 的 span，而不是 callee ident span，避免默认值补齐与 named arg
///   绑定信息在后端丢失；并且会携带 `source_path`，避免多文件 lowering 时的 span 冲突。
pub type CtorCallSiteIndex = HashMap<CallSite, CtorCallInfo>;

/// effect-op 调用点绑定信息：统一承载命名/位置实参与多 payload transport 的后端事实。
///
/// 说明：
/// - `arg_mapping[param_idx] = arg_idx` 表示第 `param_idx` 个 effect-op 形参由源码中的
///   第 `arg_idx` 个显式实参提供；
/// - `payload_tuple_ty` 仅在 2+ payload 时存在，表示后端 transport 采用的“按形参顺序打包”的 tuple 类型；
/// - 该 side table 不影响 `dump-hir` 输出稳定性。
#[derive(Debug, Clone)]
pub struct EffectOpCallInfo {
    pub arg_mapping: Vec<usize>,
    pub payload_tuple_ty: Option<TypeId>,
}

/// effect-op 调用点索引：`source_path + call span` → 已确认的参数绑定与 transport tuple。
pub type EffectOpCallSiteIndex = HashMap<CallSite, EffectOpCallInfo>;

/// 由 typecheck 确认的 direct-call target 绑定索引：`source_path + expr span` → target identity。
///
/// 说明：
/// - 这层保留的是“调用点已选中的顶层/成员函数身份”，而不是 backend 级符号；
/// - 主要供 generic MIR lowering / materialization / production reachability 在不回退到
///   backend 现场猜目标的前提下，恢复 operator overload、`compareTo` 等语法糖的真实 callee。
pub type TopLevelFunCallSiteIndex = HashMap<CallSite, ast::TopLevelFunCallBinding>;

/// 由 typecheck 确认的 canonical call-argument 绑定索引：`source_path + expr span` → param slots。
pub type CallArgBindingSiteIndex = HashMap<CallSite, ast::CallArgBinding>;

/// 由 typecheck 确认并由 HIR lowering 消费的 `with` copy-update 合同。
pub type WithUpdateSiteIndex = HashMap<CallSite, ast::WithUpdateContract>;

/// 由 typecheck/HIR lowering 确认的 assignment LHS typed place 合同。
pub type AssignPlaceSiteIndex = HashMap<CallSite, AssignPlaceContract>;

/// 动态 dispatch 调用点索引：`source_path + call span + receiver_ty` → dispatch kind。
pub type DispatchCallSiteIndex = HashMap<DispatchCallSite, DispatchCallKind>;

/// handler arm 多 binder payload tuple 索引：`source_path + op head span` → tuple `TypeId`。
pub type HandlePayloadTupleSiteIndex = HashMap<CallSite, TypeId>;

/// `nominal FQN -> ast::TypeKind` 的索引（由 HIR lowering 构建，供后端识别 effect/class/interface/...）。
pub type NominalKindIndex = HashMap<String, ast::TypeKind>;

/// `nominal FQN -> declaration-site variances` 的索引。
pub type NominalVarianceIndex = HashMap<String, Vec<Option<ast::TypeParamVariance>>>;

/// `nominal FQN -> 直接超类型 FQN 列表` 的索引。
pub type DirectSupertypesIndex = HashMap<String, Vec<String>>;

/// typecheck 已确认的 `Continuation.resume` 调用点集合（`source_path + call expr span`）。
///
/// 说明：
/// - 该 side table 只承载确定语义事实，不承载任何调用形状分类；
/// - effect segmentation 读取它来识别隐藏 suspend site。
pub type ContinuationResumeCallSiteIndex = HashSet<CallSite>;

/// `Continuation.resume` 中 receiver continuation 的 effect row 非 Pure 的调用点集合。
///
/// 说明：
/// - 只有这些 call site 才需要按“会再次向外 suspend 的 call-boundary”处理；
/// - `Continuation<Resume, Answer, eff Pure>.resume(...)` 仍只保留 hidden
///   `Raise<RuntimeError>` 边界。
pub type NonPureContinuationResumeCallSiteIndex = HashSet<CallSite>;

/// 一个 `when` pattern binder 的声明位置键。
///
/// 说明：
/// - `decl_span` 单独不足以跨文件唯一，因此要连同 `source_path` 一起作为 key；
/// - 该索引只服务于后端恢复 binder 的精确 `TypeId`，不影响 `dump-hir` 输出稳定性。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WhenPatBindingSite {
    pub source_path: PathBuf,
    pub decl_span: Span,
}

/// `when` pattern binder 的精确类型索引。
pub type WhenPatBindingTypeIndex = HashMap<WhenPatBindingSite, TypeId>;

/// 外部函数（`@Extern`）的最小后端视图。
///
/// 说明：
/// - 该信息作为后端 side table 保存，不影响 `dump-hir` 的输出稳定性；
/// - 当前阶段前端/HIR 已支持 `C` 与 `Scoop` 两类 extern ABI 元数据；
/// - `symbol` 为最终参与链接的符号名（例如 `@Extern("puts")` / `@Extern("scoop_println")`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallableAbiIdentity {
    ManagedOrdinary,
    NativeExtern,
    ManagedExtern,
    EffectBridge,
}

impl CallableAbiIdentity {
    pub const fn from_extern_abi(abi: ExternAbi) -> Self {
        match abi {
            ExternAbi::C => Self::NativeExtern,
            ExternAbi::Scoop => Self::ManagedExtern,
        }
    }

    pub const fn managed_callable(call_may_suspend: bool) -> Self {
        if call_may_suspend {
            Self::EffectBridge
        } else {
            Self::ManagedOrdinary
        }
    }

    pub const fn funptr() -> Self {
        Self::NativeExtern
    }

    pub const fn is_extern(self) -> bool {
        matches!(self, Self::NativeExtern | Self::ManagedExtern)
    }

    pub const fn uses_native_abi(self) -> bool {
        matches!(self, Self::NativeExtern)
    }

    pub const fn uses_effect_bridge_abi(self) -> bool {
        matches!(self, Self::EffectBridge)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternFun {
    pub abi: ExternAbi,
    pub symbol: String,
    /// `@Extern(..., callingConvention = "...")`（可选）：用于覆盖默认 C ABI（spec §15.5.4）。
    ///
    /// 说明：当前阶段后端只保证 `ExternAbi::C` 下的 `c/cdecl`；`ExternAbi::Scoop` 会在前端直接拒绝
    /// `callingConvention`，其它 calling convention 的支持留待后续扩展。
    pub calling_convention: Option<String>,
    /// `@Extern(lib = "...")`（可选）：需要链接的外部库名（传递给链接器作为 `-l<name>`）。
    ///
    /// 说明：链接阶段同时通过 `LoweredHir.extern_libs`（由 `collect_extern_libs` 收集的去重列表）
    /// 将所有 lib 传递给链接器。此字段记录单个函数关联的 lib，用于诊断与追溯。
    pub lib: Option<String>,
}

impl ExternFun {
    pub const fn callable_abi_identity(&self) -> CallableAbiIdentity {
        CallableAbiIdentity::from_extern_abi(self.abi)
    }
}

/// `@Extern` 的 ABI 约定。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExternAbi {
    #[default]
    C,
    Scoop,
}

impl ExternAbi {
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "c" => Some(Self::C),
            "scoop" => Some(Self::Scoop),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::C => "c",
            Self::Scoop => "scoop",
        }
    }
}

/// `fun FQN -> ExternFun` 的索引（由 HIR lowering 构建，供后端查询）。
pub type ExternFunIndex = HashMap<String, ExternFun>;

/// 有 body 的 `@CallingConvention` 函数发布的 object-level native callable symbol。
///
/// 该 symbol 只用于同一最终链接产物内的 native object 调用，不表示 package/dylib export，
/// 也不改变 Scoop 代码内对该函数的 ordinary managed ABI 调用方式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCallableFun {
    pub symbol: String,
    pub calling_convention: String,
}

/// `fun FQN -> NativeCallableFun` 的索引（由 HIR lowering 构建，供 LLVM 后端生成 wrapper）。
pub type NativeCallableFunIndex = HashMap<String, NativeCallableFun>;

/// 外部顶层变量（`@Extern val/var`）的 HIR handoff contract。
///
/// 说明：该 side table 把外部符号、链接语义与 unsafe access requirement 显式交给后续阶段，
/// 避免 MIR/LLVM 重新回 AST 或注解语法推断 extern global 语义。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternGlobal {
    pub fqn: String,
    pub source_path: PathBuf,
    pub span: Span,
    pub ty: TypeId,
    pub mutable: bool,
    pub symbol: String,
    pub linkage: ExternGlobalLinkage,
    pub storage: TopLevelVarStorage,
    pub initializer_absent: bool,
    pub unsafe_required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternGlobalLinkage {
    External,
}

/// `extern global FQN -> ExternGlobal` 的索引（由 HIR lowering 构建，供后续阶段查询）。
pub type ExternGlobalIndex = HashMap<String, ExternGlobal>;

/// 顶层可变全局变量（`@ThreadLocal` / `@Global`）的最小后端视图。
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

/// 普通顶层 immutable value（非 `const` 的 `val`）的最小后端视图。
///
/// 说明：
/// - 这类绑定需要运行期“一次初始化 + 后续稳定读取”语义；
/// - 保持独立 side table，避免把 eager-init / backing global 等后端细节塞回通用 `ValDecl`；
/// - 后续顶层 pattern binding 可复用同一表示，为每个 binder 建立一条记录。
#[derive(Debug, Clone)]
pub struct TopLevelImmutableValue {
    pub fqn: String,
    /// 声明所在源文件路径；供 init function codegen 时切换源码上下文。
    pub source_path: PathBuf,
    pub span: Span,
    pub ty: TypeId,
    pub init: Option<Expr>,
}

/// `top-level val FQN -> TopLevelImmutableValue` 的索引（由 HIR lowering 构建，供后端查询）。
pub type TopLevelImmutableValueIndex = HashMap<String, TopLevelImmutableValue>;

/// 一个 object（含 companion object）的初始化顺序与成员信息。
#[derive(Debug, Clone)]
pub struct ObjectInit {
    pub fqn: String,
    pub span: Span,
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
        raw: String,
    },
    CharLit {
        span: Span,
        value: char,
    },
    StringLit {
        span: Span,
        value: String,
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
            WhenPat::StringLit { span, .. } => *span,
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
        let mut s = f.debug_struct("HandleArm");
        s.field("span", &self.span);
        s.field("op", &self.op);
        match self.kind {
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
