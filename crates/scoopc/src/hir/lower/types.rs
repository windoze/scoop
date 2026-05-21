//! HIR lowering 的共享类型与 HIR-owned compatibility side tables。
//!
//! 该模块只放“跨多处 lowering 逻辑共享”的定义（例如 `LoweredHir`、默认参数信息、delegated property 信息），
//! 以便后续把 `expr/stmt/block/sugar` 等实现分拆到独立文件时，避免循环依赖与 `pub(crate)` 漫延。
//! 跨阶段 semantic facts 的正式发布面是 `HirFacts`，不是这些 lowering side table。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::cone::SourceConeInfo;
use crate::parser::ParseError;
use crate::resolve::ResolveError;
use crate::source::SourceFile;
use crate::span::Span;
use crate::stable_id::{StableConeKey, StableTypeParamKey};
use crate::ty::{BuiltinTypes, TypeParamType, TypeStore};
use crate::typecheck::{
    AnnotationError, ExprTypeError, PropertyDeclError, StructDeclError, TypeEnvError,
    TypeHeaderError, TypeLowerError,
};

use super::super::{
    AssignPlaceSiteIndex, CallArgBindingSiteIndex, ClassInitIndex, ContinuationResumeCallSiteIndex,
    CtorCallSiteIndex, DirectSupertypesIndex, EnumLayoutIndex, ExternFunIndex, ExternGlobalIndex,
    File, FunDecl, NativeCallableFunIndex, NominalKindIndex, NominalVarianceIndex,
    NonPureContinuationResumeCallSiteIndex, ObjectInitIndex, StructLayoutIndex, SymbolId,
    TopLevelFunCallSiteIndex, TopLevelImmutableValueIndex, TopLevelVarIndex,
    WhenPatBindingTypeIndex, WithUpdateSiteIndex,
};

#[derive(Debug, Clone)]
pub(super) struct GenericDelegatedPropertyInfo {
    pub(super) name: String,
    pub(super) delegate_field_fqn: String,
    pub(super) property_meta_fqn: String,
    pub(super) delegate_class_fqn: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct DelegatedPropertyDeclContext<'a> {
    pub(super) source: &'a SourceFile,
    pub(super) file: &'a ast::File,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StdLazyThreadSafetyMode {
    None,
    Publication,
    Synchronized,
}

impl StdLazyThreadSafetyMode {
    pub(super) fn default_for_lazy_call() -> Self {
        // Kotlin-like：lazy 默认 thread-safe。
        Self::Synchronized
    }

    pub(super) fn requires_mutex(self) -> bool {
        matches!(self, Self::Publication | Self::Synchronized)
    }
}

#[derive(Debug, Clone)]
pub(super) struct LazyDelegatedPropertyInfo<'a> {
    pub(super) decl: DelegatedPropertyDeclContext<'a>,
    pub(super) name: String,
    /// 属性类型（用于生成缓存字段的类型与 lazy initializer 的返回类型上下文）。
    pub(super) ty: Option<ast::TypeRef>,
    /// `lazy(mode)` 的线程安全策略（默认为 `Synchronized`）。
    pub(super) mode: StdLazyThreadSafetyMode,
    /// lazy 缓存值字段（class field fqn）。
    pub(super) value_field_fqn: String,
    /// lazy 初始化标记字段（class field fqn）。
    pub(super) inited_field_fqn: String,
    /// lazy 的互斥锁字段（class field fqn；仅当 mode 需要互斥锁时才存在）。
    pub(super) mutex_field_fqn: Option<String>,
    /// initializer lambda 的 body（我们在 getter 内 inline 这段表达式，避免依赖 closure codegen）。
    pub(super) initializer_body: ast::Expr,
}

#[derive(Debug, Clone)]
pub(super) struct ObservableDelegatedPropertyInfo<'a> {
    pub(super) decl: DelegatedPropertyDeclContext<'a>,
    pub(super) name: String,
    pub(super) property_fqn: String,
    pub(super) ty: Option<ast::TypeRef>,
    pub(super) on_change: ast::LambdaExpr,
    /// observable/vetoable 的内部互斥锁字段（class field fqn）。
    ///
    /// 说明：该字段用于保证并发读写的可见性，并避免 data race（T1326b）。
    pub(super) mutex_field_fqn: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct VetoableDelegatedPropertyInfo<'a> {
    pub(super) decl: DelegatedPropertyDeclContext<'a>,
    pub(super) name: String,
    pub(super) property_fqn: String,
    pub(super) ty: Option<ast::TypeRef>,
    pub(super) on_change: ast::LambdaExpr,
    /// vetoable 的内部互斥锁字段（class field fqn）。
    ///
    /// 说明：该字段用于保证并发读写的可见性，并避免 data race（T1326b）。
    pub(super) mutex_field_fqn: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) enum DelegatedPropertyInfo<'a> {
    /// 通用 delegated property lowering：仍按 spec 生成 `getValue/setValue` 调用形状（T1210）。
    ///
    /// 注意：当前 LLVM 后端不要求该路径可执行（主要用于 dump-hir/fixtures 稳定输出）。
    Generic(GenericDelegatedPropertyInfo),
    /// 标准 delegates：`lazy`（spec §10.4）。
    Lazy(LazyDelegatedPropertyInfo<'a>),
    /// 标准 delegates：`observable`（spec §10.4）。
    Observable(ObservableDelegatedPropertyInfo<'a>),
    /// 标准 delegates：`vetoable`（spec §10.4）。
    Vetoable(VetoableDelegatedPropertyInfo<'a>),
    /// map-backed delegated properties（spec §10.4）。
    ///
    /// 早期阶段的实现策略：
    /// - 在 class 初始化阶段把 `val p by data` 的值“拷贝”到真实字段 `p`；
    /// - 读取 `p` 直接走字段访问；
    /// - 这避免了 `PropertyMeta` 运行期构造与 Map 查找（当前阶段尚未实现）。
    MapBacked,
}

pub(super) type DelegatedPropertyIndex<'a> = HashMap<String, DelegatedPropertyInfo<'a>>;

/// typed HIR stage 的结构化错误。
///
/// 该错误固定 source path、span、placeholder/contract reason 与所属 item/function，避免 HIR
/// completeness gate 退化成无定位的后端 unsupported 报错。
#[derive(Debug, Clone, PartialEq, Eq, Error, Diagnostic)]
#[error("HIR stage failed for `{owner}` at {source_path:?}:{span:?}: {reason}")]
#[diagnostic(code(scoop::hir::stage_error))]
pub struct HirStageError {
    source_path: PathBuf,
    span: Span,
    reason: String,
    owner: String,
}

impl HirStageError {
    pub fn new(
        source_path: impl Into<PathBuf>,
        span: Span,
        reason: impl Into<String>,
        owner: impl Into<String>,
    ) -> Self {
        Self {
            source_path: source_path.into(),
            span,
            reason: reason.into(),
            owner: owner.into(),
        }
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }
}

/// 一个顶层函数的“默认参数信息”（用于在 HIR lowering 阶段做 call-site 默认参数补齐）。
///
/// 说明：
/// - 当前阶段（T1305/T1323）支持“调用点省略默认参数”，并允许在使用命名参数时省略中间默认参数（Kotlin-like）；
/// - 为避免向 HIR items 注入合成 wrapper 函数（会影响 `.cone` 的 public API 导出），
///   我们把 `f(a0, a1)` 这类“少传参数”的调用点改写为 block：先按参数名把实参/默认值绑定为局部 `val`，
///   再调用原函数的完整参数形态。
#[derive(Debug, Clone)]
pub(super) struct DefaultArgFunInfo {
    /// 最少需要提供的实参数量（即：无默认值形参个数）。
    pub(super) required: usize,
    pub(super) params: Vec<DefaultArgParamInfo>,
}

#[derive(Debug, Clone)]
pub(super) struct DefaultArgParamInfo {
    pub(super) decl_span: Span,
    pub(super) name: String,
    pub(super) is_vararg: bool,
    pub(super) ty_ref: Option<ast::TypeRef>,
    pub(super) default_value: Option<ast::Expr>,
}

#[derive(Debug, Clone)]
pub(in crate::hir) struct DefaultArgStructInfo {
    pub(super) decl_file: PathBuf,
    pub(super) type_params: Vec<String>,
    pub(super) params: Vec<DefaultArgParamInfo>,
}

/// HIR lowering 错误（目前仅包装 parser/resolve 错误）。
#[derive(Debug, Error, Diagnostic)]
pub enum HirLowerError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Resolve(#[from] ResolveError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    VtableLayout(#[from] crate::vtable::VtableLayoutError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    ItableLayout(#[from] crate::itable::ItableLayoutError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeHeader(Box<TypeHeaderError>),

    #[error(transparent)]
    #[diagnostic(transparent)]
    StructDecl(Box<StructDeclError>),

    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeEnv(Box<TypeEnvError>),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Annotation(Box<AnnotationError>),

    #[error(transparent)]
    #[diagnostic(transparent)]
    PropertyDecl(Box<PropertyDeclError>),

    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeLower(Box<TypeLowerError>),

    #[error(transparent)]
    #[diagnostic(transparent)]
    ExprType(Box<ExprTypeError>),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Stage(#[from] HirStageError),

    #[error("{message}")]
    Frontend { message: String },
}

impl From<TypeHeaderError> for HirLowerError {
    fn from(error: TypeHeaderError) -> Self {
        Self::TypeHeader(Box::new(error))
    }
}

impl From<StructDeclError> for HirLowerError {
    fn from(error: StructDeclError) -> Self {
        Self::StructDecl(Box::new(error))
    }
}

impl From<TypeEnvError> for HirLowerError {
    fn from(error: TypeEnvError) -> Self {
        Self::TypeEnv(Box::new(error))
    }
}

impl From<AnnotationError> for HirLowerError {
    fn from(error: AnnotationError) -> Self {
        Self::Annotation(Box::new(error))
    }
}

impl From<Box<PropertyDeclError>> for HirLowerError {
    fn from(error: Box<PropertyDeclError>) -> Self {
        Self::PropertyDecl(error)
    }
}

impl From<TypeLowerError> for HirLowerError {
    fn from(error: TypeLowerError) -> Self {
        Self::TypeLower(Box::new(error))
    }
}

impl From<ExprTypeError> for HirLowerError {
    fn from(error: ExprTypeError) -> Self {
        Self::ExprType(Box::new(error))
    }
}

/// 一次 lowering 的产物：HIR + 对应的 `TypeStore`。
///
/// 说明：HIR 节点里的 `TypeId` 仅在同一个 `TypeStore` 里可解码/展示。
/// 后续 stage 需要的源码语义事实由 `HirStageOutput::hir_facts()` 发布；这里保留的
/// side table 只服务 HIR lowering、测试 scaffolding，以及尚待 P7 清理的 LLVM compatibility path。
#[derive(Debug, Clone)]
pub struct LoweredHir {
    pub file: File,
    /// 当前 lowering 已解析的 cone identity。
    ///
    /// 用途：
    /// - 让后续 LLVM / RTTI / stable-id 迁移继续沿用 lowering 已解析好的 cone 语义身份；
    /// - 避免 backend 再从 source path 临时猜一个 cone key。
    pub stable_cone_key: StableConeKey,
    /// source path -> owning cone metadata 的 lowering 缓存；跨阶段正式查询使用 `HirFacts`。
    pub source_cones: HashMap<PathBuf, SourceConeInfo>,
    /// 声明级 type/effect 参数到 stable owner/index key 的 lowering 缓存。
    pub stable_type_param_keys: HashMap<TypeParamType, StableTypeParamKey>,
    /// member `fun` 与值类型 computed property getter 降为可 codegen 的“顶层函数形态”。
    ///
    /// 说明：
    /// - 这是一个 side table：不影响 `dump-hir` 输出稳定性（`dump-hir` 只打印 `file`）；
    /// - 供 LLVM 后端把 `receiver.method(args...)` / `receiver.prop`（lowering 后的顶层调用）
    ///   解析到真实函数体（T1508a/T4010b1）。
    pub member_funs: Vec<FunDecl>,
    pub types: TypeStore,
    /// 由本次 lowering 过程中收集到的 struct 字段布局信息（供早期 LLVM codegen 查询）。
    pub struct_layouts: StructLayoutIndex,
    /// 由本次 lowering 过程中收集到的 enum variant 布局信息（供早期 LLVM codegen 查询）。
    pub enum_layouts: EnumLayoutIndex,
    /// `@Extern` 外部函数信息（供 LLVM codegen 声明正确的符号名与 ABI）。
    pub extern_funs: ExternFunIndex,
    /// 有 body 的 `@CallingConvention` 函数信息（供 LLVM codegen 生成 object-level native callable symbol）。
    pub native_callable_funs: NativeCallableFunIndex,
    /// `@Extern` 顶层变量信息（供 HIR/MIR handoff 显式发布 extern global roots）。
    pub extern_globals: ExternGlobalIndex,
    /// 链接阶段需要额外加入的外部库（来自 `@Extern(lib = "...")`；去重 + 稳定排序）。
    ///
    /// 说明：
    /// - 该信息作为后端/driver side table 保存，不影响 `dump-hir` 的输出稳定性；
    /// - 当前阶段仅支持最小 `-l<name>` 形式（不处理 `-L`/rpath 等）。
    pub extern_libs: Vec<String>,
    /// 顶层可变全局变量信息（`@ThreadLocal/@Global`），供后端生成静态存储（TODO T1023）。
    pub top_level_vars: TopLevelVarIndex,
    /// 普通顶层 immutable value 信息；供后端生成 once-init + 稳定读取主线。
    pub top_level_immutable_values: TopLevelImmutableValueIndex,
    /// typecheck 已确认的 direct-call target 绑定（`source_path + expr span`）。
    ///
    /// 说明：
    /// - 该 side table 不影响 `dump-hir` 输出稳定性；
    /// - generic MIR lowering / production reachability 会用它恢复 operator overload /
    ///   `compareTo` 等语法糖调用点的真实 callee 身份。
    pub top_level_fun_call_sites: TopLevelFunCallSiteIndex,
    /// typecheck 已确认的 canonical call-argument 参数槽绑定。
    pub call_arg_bindings: CallArgBindingSiteIndex,
    /// typecheck 已确认并由 HIR lowering 消费的 copy-update aggregate/update 合同。
    pub with_update_contracts: WithUpdateSiteIndex,
    /// typecheck/HIR lowering 已确认的 assignment LHS typed place 合同。
    pub assign_place_contracts: AssignPlaceSiteIndex,
    /// `object` / `companion object` 的初始化信息（供早期 LLVM codegen 查询）。
    pub object_inits: ObjectInitIndex,
    /// `class` 的初始化信息（Appendix B.2.2，供 LLVM codegen 查询）。
    pub class_inits: ClassInitIndex,
    /// class vtable slots（TODO T1507c2 / T1508b）。
    ///
    /// 说明：
    /// - 该信息作为后端 side table 保存，不影响 `dump-hir` 的输出稳定性；
    /// - LLVM 后端用它生成每个 class 的 vtable 常量，并在虚调用点选择 slot。
    pub class_vtables: crate::vtable::ClassVtableIndex,
    /// interface 元数据（stable interface_id + method slots）与 class itable entries（T1507c3 / T1508c）。
    ///
    /// 说明：
    /// - 该信息作为后端 side table 保存，不影响 `dump-hir` 的输出稳定性；
    /// - LLVM 后端用它生成每个 class 的 itable 常量，并在 interface 调用点选择 slot。
    pub interfaces: crate::itable::InterfaceIndex,
    pub class_itables: crate::itable::ClassItableIndex,
    /// ctor 调用点候选集合：用于让 codegen 识别 `UnresolvedIdent` 的 ctor 调用。
    pub ctor_call_sites: CtorCallSiteIndex,
    /// 动态 dispatch 调用点索引：供 MIR/LLVM 直接消费“已经分类好的 call site”。
    pub dispatch_call_sites: crate::hir::DispatchCallSiteIndex,
    /// effect-op 调用点绑定信息：供 LLVM 后端把多 payload transport 收口到参数顺序主线。
    pub effect_op_call_sites: crate::hir::EffectOpCallSiteIndex,
    /// handler arm 多 binder payload 的 tuple 类型索引。
    pub handle_payload_tuple_tys: crate::hir::HandlePayloadTupleSiteIndex,
    /// typecheck 已确认的 `Continuation.resume` 调用点集合。
    pub continuation_resume_call_sites: ContinuationResumeCallSiteIndex,
    /// receiver continuation 的 effect row 非 Pure 的 `Continuation.resume` 调用点集合。
    pub non_pure_continuation_resume_call_sites: NonPureContinuationResumeCallSiteIndex,
    /// `when` pattern binder 的精确类型索引；供后端恢复函数值等精确局部类型。
    pub when_pat_binding_tys: WhenPatBindingTypeIndex,
    /// nominal 类型种类索引（effect/class/interface/...）。
    pub nominal_kinds: NominalKindIndex,
    /// nominal 声明处 variance 索引。
    pub nominal_variances: NominalVarianceIndex,
    /// nominal 直接超类型索引。
    pub direct_supertypes: DirectSupertypesIndex,
    /// 当前 lowering 使用的 builtin TypeId 集合。
    pub builtins: BuiltinTypes,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum SymbolKey {
    Local {
        source_path: PathBuf,
        decl_span: Span,
    },
    TopLevel {
        fqn: String,
    },
}

/// 一个最小的 symbol interner：把“解析后的符号键”映射为一个紧凑的 `SymbolId`。
///
/// 说明：
/// - 该表仅用于 HIR dump/fixtures 的稳定标识，并不试图提供跨 session 的全局稳定性；
/// - `SymbolId` 的分配顺序依赖 traversal 顺序，但 traversal 对同一个 AST 是确定的，因此 golden 可回归。
#[derive(Debug, Default)]
pub(super) struct SymbolInterner {
    next: u32,
    by_key: HashMap<SymbolKey, SymbolId>,
}

impl SymbolInterner {
    pub(super) fn intern_local(&mut self, source_path: &Path, decl_span: Span) -> SymbolId {
        self.intern(SymbolKey::Local {
            source_path: source_path.to_path_buf(),
            decl_span,
        })
    }

    pub(super) fn intern_top_level(&mut self, fqn: String) -> SymbolId {
        self.intern(SymbolKey::TopLevel { fqn })
    }

    fn intern(&mut self, key: SymbolKey) -> SymbolId {
        if let Some(id) = self.by_key.get(&key).copied() {
            return id;
        }

        let id = SymbolId(self.next);
        self.next = self.next.saturating_add(1);
        self.by_key.insert(key, id);
        id
    }
}

/// `[...]` 数组字面量在 lowering 阶段需要知道“期望的容器类型”。
///
/// 说明：
/// - Scoop 的数组字面量仅在“有明确期望类型”的语境下成立（见 TODO T1317a）；
/// - `dump-hir`/HIR fixtures 当前不运行完整 typecheck，因此这里用一个极小的 hint
///   从“语法层面的类型注解/函数签名”向下传播到 `ExprKind::ArrayLit`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ArrayLitTarget {
    Array,
    MutableArray,
}

/// lowering 期间的”期望类型 hint”（仅覆盖当前需要的数组字面量目标类型）。
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ExpectedExpr {
    /// 一般表达式的期望类型。
    ///
    /// 当前主要用于在 typecheck side table 没有写回最终类型时，
    /// 仍能给需要 typecheck side table 的语义糖恢复正确的 HIR 结果类型。
    pub(super) value_ty: Option<crate::ty::TypeId>,
    pub(super) array_lit_target: Option<ArrayLitTarget>,
    /// 数组字面量的完整期望类型（例如 `Array<Int>` / `MutableArray<String>`）。
    ///
    /// 用途：
    /// - 在已知容器类型时，把元素的期望类型继续向下传给嵌套数组/struct 字面量；
    /// - 若 typecheck 已写回表达式最终类型，HIR lowering 也可回退使用该信息恢复结果类型。
    pub(super) array_lit_ty: Option<crate::ty::TypeId>,
    /// T0124: expected type for struct literal (used to infer type args for generic structs).
    pub(super) struct_lit_ty: Option<crate::ty::TypeId>,
}
