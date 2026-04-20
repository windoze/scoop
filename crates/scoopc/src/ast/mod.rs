//! AST（抽象语法树）。
//!
//! 目前阶段的 AST 目标：
//! - 足够表达“文件头（package/import）+ 顶层声明（fun/val/var 等）”的结构
//! - 节点主要用 `Span` 指回源文本，避免早期过度分配
//!
//! 注意：随着 parser/typechecker 完善，AST 结构可能会演进。

use std::cell::{OnceCell, RefCell};
use std::collections::{HashMap, HashSet};

use crate::span::Span;
use crate::ty::TypeId;

#[derive(Clone)]
pub struct File {
    /// 文件级注解列表：`@file:...`（spec §15.3）。
    ///
    /// 说明：
    /// - 仅承载语法结构；target/retention 等语义规则由后续 typecheck 任务实现（T1016）。
    /// - 为保持既有 parse fixtures 的 AST snapshot 稳定：
    ///   - 该字段为空时，Debug 输出不会打印它。
    pub file_annotations: Vec<AnnotationUse>,
    pub package: Option<PackageDecl>,
    pub imports: Vec<ImportDecl>,
    pub items: Vec<Item>,
    /// typecheck 写回的表达式类型 side table（按源码 span 索引）。
    ///
    /// 说明：
    /// - 该表不参与 AST Debug 输出，避免影响 parse fixtures；
    /// - HIR lowering 在 build/test 路径下会读取该表，把“expected type 已由 typecheck 推断出”的
    ///   array literal / nested array 等表达式降为可执行 HIR；
    /// - dump-hir 路径不运行完整 typecheck，因此这里允许保持为空。
    pub(crate) inferred_expr_tys: RefCell<HashMap<Span, TypeId>>,
    /// typecheck 写回的“局部绑定 span -> 推导后的 TypeId” side table。
    ///
    /// 说明：
    /// - 用于保存不依赖显式类型注解、但后端仍需要精确类型的局部绑定（例如 `handle` arm binder）；
    /// - HIR lowering 在 build/test 路径下会读取该表，避免这类 binder 退回到 `Any`；
    /// - 该表不参与 AST Debug 输出，避免影响 parse fixtures。
    pub(crate) inferred_binding_tys: RefCell<HashMap<Span, TypeId>>,
    /// typecheck 写回的“perform span -> performed effect 实例 TypeId” side table。
    ///
    /// 说明：
    /// - 该表只记录真正会进入 perform-slot / unified handler dispatch 的 effect 实例；
    /// - HIR lowering 读取它，把 `perform` 从“只知道 op FQN”提升为“同时知道 effect 实例类型”。
    pub(crate) inferred_performed_effect_tys: RefCell<HashMap<Span, TypeId>>,
    /// typecheck 写回的“handle arm op span -> handled effect 实例 TypeId” side table。
    ///
    /// 说明：
    /// - HIR lowering 读取该表，才能把 dispatch 建立在真实 handled-effect 合同上；
    /// - 对于 `Effect<T>.op<U>(...)` 这类显式 type args 的 arm head，也必须以这里写回的实例类型为准。
    pub(crate) inferred_handle_arm_effect_tys: RefCell<HashMap<Span, TypeId>>,
    /// typecheck 补回的 safe member access 解析结果（按 member name 的源码 span 索引）。
    ///
    /// 说明：
    /// - 当 receiver 为 nullable（`T?` / `Option<T>`）时，resolver 往往无法仅凭语法确定
    ///   `receiver?.member` 的目标；
    /// - typecheck 在拿到 inner receiver type 后会补做一次成员/extension property 决议，并把结果
    ///   写回此表，供 HIR lowering / codegen 复用；
    /// - 该表同样不参与 AST Debug 输出，避免影响 parse fixtures。
    pub(crate) safe_member_access_resolved: RefCell<HashMap<Span, ResolvedMemberRef>>,
    /// typecheck 最终确认的“member span -> 成员解析结果” side table。
    ///
    /// 说明：
    /// - 覆盖普通 member access / member call / safe member access 的最终决议；
    /// - 用于承载“resolver 先按语法环境给出初始决议，但 typecheck 在拿到 receiver 实际类型后
    ///   做了晚解析/改写”的场景（例如 receiver lambda 的隐式 `this`）；
    /// - HIR lowering / codegen 应优先读取这张表，而不是盲信 AST 上 `member.resolved` 的初始值。
    pub(crate) typechecked_member_resolved: RefCell<HashMap<Span, ResolvedMemberRef>>,
    /// typecheck 已确认的 `Continuation.resume` 调用点（按整个 call expr 的源码 span 索引）。
    ///
    /// 说明：
    /// - 该表只承载“已被 typecheck 证实”的 builtin 语义点，不承载任何基于语法形状的推断；
    /// - effect segmentation 读取它来识别隐藏 suspend site，避免再按 member 名称或 receiver 形状猜测。
    pub(crate) continuation_resume_call_sites: RefCell<HashSet<Span>>,
    /// typecheck 已确认的“非 Pure continuation.resume 调用点”。
    ///
    /// 说明：
    /// - `Continuation<Resume, Answer, eff Pure>.resume(...)` 只需要 hidden
    ///   `Raise<RuntimeError>` 边界；
    /// - 只有 `E` 非 Pure 时，effect segmentation 才应把该 call site 视为真正的
    ///   outward-suspending call-boundary，并走 resume.after.call replay 主线。
    pub(crate) non_pure_continuation_resume_call_sites: RefCell<HashSet<Span>>,
    /// typecheck 选中的“顶层函数值”目标（按表达式 span 索引）。
    ///
    /// 说明：
    /// - 用于承载 `foo` / `foo<T>` 在值位置被当作函数值时的精确目标；
    /// - HIR lowering 读取它，把该表达式合成为零捕获 closure，而不是误当成普通顶层值读取；
    /// - `type_args` 保留 typecheck 阶段的具体实例化结果，供后续 monomorphized FQN 选择复用。
    pub(crate) top_level_fun_value_refs: RefCell<HashMap<Span, TopLevelFunValueRef>>,
    /// typecheck 选中的 effect-op 调用绑定信息（按调用 span 索引）。
    ///
    /// 说明：
    /// - `arg_mapping[param_idx] = arg_idx` 表示 effect op 的第 `param_idx` 个形参由源码中的
    ///   第 `arg_idx` 个显式实参提供；
    /// - HIR lowering / LLVM codegen 读取它，把多 payload transport 收口到“按形参顺序组织 payload，
    ///   但按源码顺序求值显式实参”的统一主线；
    /// - 避免 perform lowering 再按命名 / 位置实参形状重新猜测 payload 布局。
    pub(crate) typechecked_effect_op_call_bindings: RefCell<HashMap<Span, EffectOpCallBinding>>,
    /// typecheck 选中的 ctor 调用绑定信息（按调用 span 索引）。
    ///
    /// 说明：
    /// - 覆盖普通 `Class(...)` 构造调用、class header `: Base(...)` super ctor 调用，
    ///   以及 secondary ctor 的 `: this(...)` / `: super(...)`；
    /// - 记录“最终选中的 ctor 目标 + 形参槽位到调用点实参索引的绑定”，避免 HIR/codegen 再按
    ///   callee 形状或 arity 重新猜测；
    /// - `arg_mapping[param_idx] = Some(arg_idx)` 表示该形参由调用点第 `arg_idx` 个显式实参提供，
    ///   `None` 表示由默认值补齐。
    pub(crate) typechecked_ctor_call_bindings: RefCell<HashMap<Span, CtorCallBinding>>,
}

impl std::fmt::Debug for File {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("File");
        if !self.file_annotations.is_empty() {
            s.field("file_annotations", &self.file_annotations);
        }
        s.field("package", &self.package);
        s.field("imports", &self.imports);
        s.field("items", &self.items);
        s.finish()
    }
}

impl File {
    pub fn replace_inferred_expr_tys(&self, inferred: HashMap<Span, TypeId>) {
        *self.inferred_expr_tys.borrow_mut() = inferred;
    }

    pub fn inferred_expr_ty(&self, span: Span) -> Option<TypeId> {
        self.inferred_expr_tys.borrow().get(&span).copied()
    }

    pub fn replace_inferred_binding_tys(&self, inferred: HashMap<Span, TypeId>) {
        *self.inferred_binding_tys.borrow_mut() = inferred;
    }

    pub fn inferred_binding_ty(&self, span: Span) -> Option<TypeId> {
        self.inferred_binding_tys.borrow().get(&span).copied()
    }

    pub fn replace_inferred_performed_effect_tys(&self, inferred: HashMap<Span, TypeId>) {
        *self.inferred_performed_effect_tys.borrow_mut() = inferred;
    }

    pub fn inferred_performed_effect_ty(&self, span: Span) -> Option<TypeId> {
        self.inferred_performed_effect_tys
            .borrow()
            .get(&span)
            .copied()
    }

    pub fn replace_inferred_handle_arm_effect_tys(&self, inferred: HashMap<Span, TypeId>) {
        *self.inferred_handle_arm_effect_tys.borrow_mut() = inferred;
    }

    pub fn inferred_handle_arm_effect_ty(&self, span: Span) -> Option<TypeId> {
        self.inferred_handle_arm_effect_tys
            .borrow()
            .get(&span)
            .copied()
    }

    pub fn replace_safe_member_access_resolved(&self, resolved: HashMap<Span, ResolvedMemberRef>) {
        *self.safe_member_access_resolved.borrow_mut() = resolved;
    }

    pub fn safe_member_access_resolved(&self, span: Span) -> Option<ResolvedMemberRef> {
        self.safe_member_access_resolved
            .borrow()
            .get(&span)
            .cloned()
    }

    pub fn replace_typechecked_member_resolved(&self, resolved: HashMap<Span, ResolvedMemberRef>) {
        *self.typechecked_member_resolved.borrow_mut() = resolved;
    }

    pub fn typechecked_member_resolved(&self, span: Span) -> Option<ResolvedMemberRef> {
        self.typechecked_member_resolved
            .borrow()
            .get(&span)
            .cloned()
    }

    pub fn replace_continuation_resume_call_sites(&self, sites: HashSet<Span>) {
        *self.continuation_resume_call_sites.borrow_mut() = sites;
    }

    pub fn continuation_resume_call_sites(&self) -> HashSet<Span> {
        self.continuation_resume_call_sites.borrow().clone()
    }

    pub fn replace_non_pure_continuation_resume_call_sites(&self, sites: HashSet<Span>) {
        *self.non_pure_continuation_resume_call_sites.borrow_mut() = sites;
    }

    pub fn non_pure_continuation_resume_call_sites(&self) -> HashSet<Span> {
        self.non_pure_continuation_resume_call_sites
            .borrow()
            .clone()
    }

    pub fn replace_top_level_fun_value_refs(&self, refs: HashMap<Span, TopLevelFunValueRef>) {
        *self.top_level_fun_value_refs.borrow_mut() = refs;
    }

    pub fn top_level_fun_value_ref(&self, span: Span) -> Option<TopLevelFunValueRef> {
        self.top_level_fun_value_refs.borrow().get(&span).cloned()
    }

    pub fn replace_typechecked_effect_op_call_bindings(
        &self,
        bindings: HashMap<Span, EffectOpCallBinding>,
    ) {
        *self.typechecked_effect_op_call_bindings.borrow_mut() = bindings;
    }

    pub fn typechecked_effect_op_call_binding(&self, span: Span) -> Option<EffectOpCallBinding> {
        self.typechecked_effect_op_call_bindings
            .borrow()
            .get(&span)
            .cloned()
    }

    pub fn replace_typechecked_ctor_call_bindings(&self, bindings: HashMap<Span, CtorCallBinding>) {
        *self.typechecked_ctor_call_bindings.borrow_mut() = bindings;
    }

    pub fn typechecked_ctor_call_binding(&self, span: Span) -> Option<CtorCallBinding> {
        self.typechecked_ctor_call_bindings
            .borrow()
            .get(&span)
            .cloned()
    }
}

#[derive(Debug, Clone)]
pub struct TopLevelFunValueRef {
    pub fqn: String,
    pub type_args: Vec<TypeId>,
}

#[derive(Debug, Clone)]
pub struct EffectOpCallBinding {
    pub arg_mapping: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct CtorCallBinding {
    pub owner_fqn: String,
    pub ctor_span: Option<Span>,
    pub arg_mapping: Vec<Option<usize>>,
}

#[derive(Debug, Clone)]
pub struct PackageDecl {
    pub span: Span,
    pub path: Vec<Ident>,
}

#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub span: Span,
    pub path: Vec<Ident>,
    pub has_star: bool,
    pub alias: Option<Ident>,
}

#[derive(Debug, Clone)]
pub enum Item {
    TypeAlias(Box<TypeAliasDecl>),
    Fun(Box<FunDecl>),
    /// package 顶层 `comptime if`（条件编译）：块内为“顶层 items 列表”。
    ///
    /// 说明：
    /// - 该节点只承载语法结构；真正的“分支裁剪/选择”发生在 resolver 之前（TODO T1220b）。
    /// - 块内不允许出现语句/表达式；若出现应在 parse 阶段报错（TODO T1220a）。
    ComptimeIf(Box<ComptimeIfItem>),
    /// 顶层扩展属性声明（spec §10.3）。
    ///
    /// 语法形态示例：
    /// - `val String.lastIndex: Int get() = ...`
    /// - `var StringBuilder.lastChar: Int get() = ... set(v) { ... }`
    ///
    /// 说明：
    /// - 扩展属性不属于某个 `TypeDecl` 的 `TypeBody`，因此不是 `TypeMember::Property`；
    /// - 它的语义检查（computed/无 backing field 等）由后续 typecheck 任务逐步补齐（见 TODO T0433）。
    ExtensionProperty(Box<ExtensionPropertyDecl>),
    Type(Box<TypeDecl>),
    Object(Box<ObjectDecl>),
    Val(Box<ValDecl>),
}

/// 顶层 item 块：`{ <item>* }`。
///
/// 与普通 `Block { stmts }` 的区别：
/// - `ItemBlock` 只出现在“package-level comptime if”的分支里；
/// - 其中的元素必须是顶层 items（fun/class/val/...），不允许语句/表达式。
#[derive(Debug, Clone)]
pub struct ItemBlock {
    pub span: Span,
    pub items: Vec<Item>,
}

/// package-level `comptime if (...) { ... } else ...`。
///
/// 注意：语句级 `comptime if` 使用的是 `StmtKind::ComptimeIf(ComptimeIf)`；
/// 这里的 `ComptimeIfItem` 用于顶层 item 列表的条件编译。
#[derive(Debug, Clone)]
pub struct ComptimeIfItem {
    pub span: Span,
    pub comptime_span: Span,
    pub if_span: Span,
    pub cond: Expr,
    pub then_branch: ItemBlock,
    pub else_branch: Option<Box<ComptimeIfItemElse>>,
}

#[derive(Debug, Clone)]
pub enum ComptimeIfItemElse {
    Block(ItemBlock),
    If(Box<ComptimeIfItem>),
}

/// 类型别名声明：`typealias Name = Type`（Appendix B.10）。
///
/// 说明：
/// - typealias 的语义（展开/循环检测/跨包导出等）在 resolver/typecheck 阶段实现（见 TODO T0446/T1302）。
#[derive(Clone)]
pub struct TypeAliasDecl {
    pub span: Span,
    /// 声明上的注解列表：`@Name(...)`（spec §15.3）。
    ///
    /// 说明：当前阶段仅做语法层“解析并存储”；合法性/目标校验由后续 typecheck 任务实现。
    pub annotations: Vec<AnnotationUse>,
    pub modifiers: Vec<Modifier>,
    pub name: Ident,
    /// typealias 的泛型参数列表：`typealias Name<T> = ...`（Appendix B.10）。
    ///
    /// 说明：与 `fun`/`type` 的 type params 一样，当前阶段仅做语法解析与结构化存储；
    /// 具体“作用域/实例化/导出”规则由 typecheck/cone 阶段决定。
    pub type_params: Vec<TypeParam>,
    pub ty: TypeRef,
}

impl std::fmt::Debug for TypeAliasDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("TypeAliasDecl");
        s.field("span", &self.span);
        if !self.annotations.is_empty() {
            s.field("annotations", &self.annotations);
        }
        if !self.modifiers.is_empty() {
            s.field("modifiers", &self.modifiers);
        }
        s.field("name", &self.name);
        if !self.type_params.is_empty() {
            s.field("type_params", &self.type_params);
        }
        s.field("ty", &self.ty);
        s.finish()
    }
}

/// 声明修饰符（modifiers）。
///
/// 说明：
/// - 目前阶段（T0245）仅做语法层“解析并存储”，不做合法性/组合校验；
/// - 解析时会做去重与排序，因此**顺序无关**（便于 fixtures/AST snapshot 稳定回归）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Modifier {
    // visibility
    Public,
    Internal,
    Private,
    // inheritance / dispatch
    Open,
    Abstract,
    Sealed,
    // misc
    /// `async fun`：异步函数标记（spec §5.3 / §5.7）。
    ///
    /// 说明：当前阶段仅做语法层解析与存储；语义（签名降糖、Task 模型）由 typecheck/lowering 负责。
    Async,
    Inline,
    Override,
    /// 编译期可求值/可用于编译期执行的标记（spec §6）。
    ///
    /// 说明：当前阶段仅做语法层解析与存储；语义检查与执行由后续阶段实现。
    Const,
    /// 注解类标记：`annotation class`（spec §15.2）。
    ///
    /// 当前阶段仅用于 parser 把 `annotation` 作为修饰符解析并存储；
    /// 语义限制（例如只允许用于 class、参数必须是 `val` 等）由后续阶段实现。
    Annotation,
}

/// 注解使用（annotation use）：`@Name(...)`（spec §15.3）。
///
/// 说明：注解语义（target/retention/参数类型等）由后续 typecheck 负责；
/// 当前阶段只做语法结构建模，用于 fixtures 与后续检查器接入。
#[derive(Clone)]
pub struct AnnotationUse {
    pub span: Span,
    /// use-site target（可选）：`@property:Foo` / `@param:Bar`。
    ///
    /// 说明：当前阶段只做语法解析与结构化存储，不校验 target 合法性。
    pub use_site_target: Option<Ident>,
    /// 注解名路径：`@A` / `@A.B`。
    pub path: Vec<Ident>,
    /// 参数列表（可选）：`@Name(arg1, name: "x")`。
    pub args: Vec<AnnotationArg>,
}

/// 注解参数（仅建模语法）：`name: value` / `value`。
#[derive(Clone)]
pub struct AnnotationArg {
    pub span: Span,
    pub name: Option<Ident>,
    /// 当前阶段约束（T1001 + T1013）：
    /// - 允许字面量值（int/string）；
    /// - 允许 `Ident(.Ident)*` 的最小枚举值引用（用于 `@Target(AnnotationTarget.X, ...)`）。
    pub value: Expr,
}

impl std::fmt::Debug for AnnotationUse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("AnnotationUse");
        s.field("span", &self.span);
        if self.use_site_target.is_some() {
            s.field("use_site_target", &self.use_site_target);
        }
        s.field("path", &self.path);
        if !self.args.is_empty() {
            s.field("args", &self.args);
        }
        s.finish()
    }
}

impl std::fmt::Debug for AnnotationArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("AnnotationArg");
        s.field("span", &self.span);
        if self.name.is_some() {
            s.field("name", &self.name);
        }
        s.field("value", &self.value);
        s.finish()
    }
}

/// 类型参数的变型标记（spec §3.2~§3.3）。
///
/// 说明：当前阶段（T0249）仅做语法层“解析并存储”，不做任何合法性校验。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeParamVariance {
    In,
    Out,
}

/// 声明处的类型参数（type parameter）。
///
/// 当前阶段：
/// - (T0218) 支持无约束的 `T` / `U`
/// - (T0249) 额外支持 `in T` / `out T` 声明处变型
/// - 不支持上界/下界（`:` / `where`）（留给后续任务）
#[derive(Clone)]
pub struct TypeParam {
    pub span: Span,
    pub variance: Option<TypeParamVariance>,
    pub name: Ident,
}

impl std::fmt::Debug for TypeParam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 为了保持 fixtures 的 AST snapshot 稳定回归：
        // - `variance` 为 None 时不输出该字段（与 T0218 之前保持一致）
        let mut s = f.debug_struct("TypeParam");
        s.field("span", &self.span);
        if self.variance.is_some() {
            s.field("variance", &self.variance);
        }
        s.field("name", &self.name);
        s.finish()
    }
}

/// 泛型 `where` 子句（spec §3 / Appendix B）。
///
/// 语法形态（Kotlin 风格）：
/// - `fun <T> f(x: T): T where T: Show`
/// - `class Box<T> where T: Clone`
///
/// 说明：
/// - 当前阶段（T0260）仅做语法层解析与结构化存储；
/// - 约束语义、满足性与冲突诊断留给 resolver/typecheck（见 TODO T0320+）。
#[derive(Clone)]
pub struct WhereClause {
    pub span: Span,
    pub constraints: Vec<WhereConstraint>,
}

impl std::fmt::Debug for WhereClause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("WhereClause");
        s.field("span", &self.span);
        s.field("constraints", &self.constraints);
        s.finish()
    }
}

/// `where T: Bound` 中的一条约束。
#[derive(Clone)]
pub struct WhereConstraint {
    pub span: Span,
    pub ty_param: Ident,
    pub bound: TypeRef,
}

impl std::fmt::Debug for WhereConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("WhereConstraint");
        s.field("span", &self.span);
        s.field("ty_param", &self.ty_param);
        s.field("bound", &self.bound);
        s.finish()
    }
}

/// effect row 参数（`eff E = Pure`）（spec §3.4 / §5.8）。
///
/// 说明：
/// - `eff` 是上下文关键字：只在 `<...>` 泛型参数/实参列表内部被当作关键字处理；
/// - 当前阶段（T0250）仅做语法层解析与存储：记录参数名与可选默认值；
/// - 复杂 row 约束、推断与实例化由后续阶段实现。
#[derive(Clone)]
pub struct EffectRowParam {
    pub span: Span,
    pub name: Ident,
    pub default: Option<EffectRowExpr>,
}

impl std::fmt::Debug for EffectRowParam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("EffectRowParam");
        s.field("span", &self.span);
        s.field("name", &self.name);
        if self.default.is_some() {
            s.field("default", &self.default);
        }
        s.finish()
    }
}

/// 类型声明的主构造头（primary constructor header）。
///
/// 语法形态（Kotlin 风格，简化版）：
/// - `class Name(param: T, ...)`
///
/// 说明：当前阶段（T0248）只做解析与结构化存储；
/// `val/var` 参数是否“同时声明字段/属性”会在后续阶段逐步补齐（T0438 起在 `Param.kind` 中保留语法信息）。
#[derive(Debug, Clone)]
pub struct PrimaryCtorDecl {
    pub params_span: Span,
    pub params: Vec<Param>,
}

/// 超类型（supertype）条目：`BaseType(...)` / `IInterface`。
#[derive(Clone)]
pub struct SuperType {
    pub span: Span,
    pub ty: TypeRef,
    /// 基类构造调用参数列表的 span（括号范围）。
    pub ctor_args_span: Option<Span>,
    /// 基类构造调用参数列表（可选）。
    ///
    /// 说明：
    /// - 当 `ctor_args_span=None` 时该列表必须为空；
    /// - 当 `ctor_args_span=Some(..)` 时，列表可能为空（例如 `Base()`）。
    pub ctor_args: Vec<Expr>,
}

impl std::fmt::Debug for SuperType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("SuperType");
        s.field("span", &self.span);
        s.field("ty", &self.ty);
        if self.ctor_args_span.is_some() {
            s.field("ctor_args_span", &self.ctor_args_span);
        }
        if !self.ctor_args.is_empty() {
            s.field("ctor_args", &self.ctor_args);
        }
        s.finish()
    }
}

#[derive(Clone)]
pub struct TypeDecl {
    pub span: Span,
    pub annotations: Vec<AnnotationUse>,
    pub modifiers: Vec<Modifier>,
    pub kind: TypeKind,
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub eff_param: Option<EffectRowParam>,
    pub where_clause: Option<WhereClause>,
    /// 主构造头参数列表：`class Name(...)`。
    pub primary_ctor: Option<PrimaryCtorDecl>,
    /// 继承/实现列表：`class Dog(...) : Animal(...), IFoo`。
    pub supertypes: Vec<SuperType>,
    /// 类型体（`{ ... }`）。
    ///
    /// 当前阶段：
    /// - parser 仍可能仅保证括号平衡与 span 正确
    /// - 成员列表的解析会在后续任务中逐步补齐
    pub body: Option<TypeBody>,
}

impl std::fmt::Debug for TypeDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("TypeDecl");
        s.field("span", &self.span);
        if !self.annotations.is_empty() {
            s.field("annotations", &self.annotations);
        }
        if !self.modifiers.is_empty() {
            s.field("modifiers", &self.modifiers);
        }
        s.field("kind", &self.kind);
        s.field("name", &self.name);
        s.field("type_params", &self.type_params);
        if self.eff_param.is_some() {
            s.field("eff_param", &self.eff_param);
        }
        if self.where_clause.is_some() {
            s.field("where_clause", &self.where_clause);
        }
        if self.primary_ctor.is_some() {
            s.field("primary_ctor", &self.primary_ctor);
        }
        if !self.supertypes.is_empty() {
            s.field("supertypes", &self.supertypes);
        }
        s.field("body", &self.body);
        s.finish()
    }
}

/// object 声明（Appendix B.9）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    /// `object Name { ... }`
    Object,
    /// `companion object { ... }` / `companion object Name { ... }`
    Companion,
}

/// Kotlin-like 单例 object 声明。
///
/// 说明：当前阶段（T0258）仅做语法解析与结构化存储：
/// - 单例语义、成员访问与初始化时机留给后续 resolver/typecheck/codegen。
#[derive(Clone)]
pub struct ObjectDecl {
    pub span: Span,
    pub annotations: Vec<AnnotationUse>,
    pub modifiers: Vec<Modifier>,
    pub kind: ObjectKind,
    /// 对于 `companion object { ... }`，name 允许缺省。
    pub name: Option<Ident>,
    /// `object Name : IFoo { ... }` 的超类型列表（可选）。
    pub supertypes: Vec<SuperType>,
    /// object body（`{ ... }`）。
    pub body: Option<TypeBody>,
}

impl std::fmt::Debug for ObjectDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("ObjectDecl");
        s.field("span", &self.span);
        if !self.annotations.is_empty() {
            s.field("annotations", &self.annotations);
        }
        if !self.modifiers.is_empty() {
            s.field("modifiers", &self.modifiers);
        }
        s.field("kind", &self.kind);
        if self.name.is_some() {
            s.field("name", &self.name);
        }
        if !self.supertypes.is_empty() {
            s.field("supertypes", &self.supertypes);
        }
        s.field("body", &self.body);
        s.finish()
    }
}

/// 类型体（`{ ... }`）——可包含成员列表。
///
/// 注意：这里与 `Block` 不同：
/// - `Block` 用于函数体/表达式块（后续会包含语句）
/// - `TypeBody` 用于 `class/interface/struct/enum/effect` 的成员声明列表
#[derive(Debug, Clone)]
pub struct TypeBody {
    pub span: Span,
    pub members: Vec<TypeMember>,
}

/// enum variant 声明（spec §2.3.2）。
///
/// 说明：
/// - variant 语法形态：`Name` / `Name(val field: T, ...)`
/// - 当前阶段仅做语法建模与结构化存储；更完整的 enum 语义由 typecheck/lowering 补齐（见 TODO T0425+）。
#[derive(Clone)]
pub struct EnumVariantDecl {
    pub span: Span,
    pub annotations: Vec<AnnotationUse>,
    pub name: Ident,
    /// variant 携带的字段列表（用 `Param` 复用 `name + ty + default_value` 的结构）。
    ///
    /// 注意：语法上要求 `val field: T`；当前 parser 会消费 `val` 关键字并写入 `Param.kind = Some(Val)`。
    pub params: Vec<Param>,
    /// value-only enum 的判别值：`A = 0` 中的 `0`（spec §2.3.2.1）。
    ///
    /// 说明：
    /// - 当前阶段仅做语法解析与结构化存储；
    /// - 判别值必须是可编译期求值的整型常量（语义检查/常量求值由 typecheck 负责）。
    pub discriminant: Option<Expr>,
}

impl std::fmt::Debug for EnumVariantDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("EnumVariantDecl");
        s.field("span", &self.span);
        if !self.annotations.is_empty() {
            s.field("annotations", &self.annotations);
        }
        s.field("name", &self.name);
        if !self.params.is_empty() {
            s.field("params", &self.params);
        }
        if self.discriminant.is_some() {
            s.field("discriminant", &self.discriminant);
        }
        s.finish()
    }
}

/// 类型体中的成员声明（最小骨架）。
#[derive(Debug, Clone)]
pub enum TypeMember {
    EnumVariant(Box<EnumVariantDecl>),
    Property(Box<PropertyDecl>),
    /// class 初始化块：`init { ... }`（Appendix B.2.2）。
    ///
    /// 说明：当前阶段（T0256）仅做语法解析与结构化存储；
    /// 初始化顺序、`this` 语义与与构造器交互由后续 resolver/typecheck 决定。
    InitBlock(Box<InitBlockDecl>),
    /// class 次构造器（secondary constructor）：`constructor(...) { ... }`（Appendix B.2.2）。
    ///
    /// 说明：当前阶段（T0257）仅做语法解析与结构化存储；
    /// 初始化顺序、delegation call 合法性与重载规则由后续 resolver/typecheck 决定。
    SecondaryCtor(Box<SecondaryCtorDecl>),
    Fun(Box<FunDecl>),
    Type(Box<TypeDecl>),
    Object(Box<ObjectDecl>),
}

/// class 初始化块：`init { ... }`（Appendix B.2.2）。
#[derive(Debug, Clone)]
pub struct InitBlockDecl {
    pub span: Span,
    pub body: Block,
}

/// 次构造器 delegation call：`constructor(...) : this(...) { ... }` / `constructor(...) : super(...) { ... }`。
///
/// 说明：
/// - `constructor` / `this` / `super` 在 lexer 层当前仍是 `Ident`；
/// - delegation call 的参数列表按“调用参数列表”规则解析（支持命名参数/`*spread` 等语法），
///   但其语义门禁与调用规则由后续 typecheck/lowering 决定；
/// - 更完整的调用解析与语义检查交给后续阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CtorDelegationKind {
    This,
    Super,
}

/// 次构造器 delegation call（`:` + 目标 + 参数括号）。
#[derive(Clone)]
pub struct CtorDelegationCall {
    pub span: Span,
    pub colon_span: Span,
    pub kind: CtorDelegationKind,
    /// `this` / `super` token 的 span（当前为上下文关键字，仍按 Ident 处理）。
    pub target_span: Span,
    /// 调用参数列表的括号 span。
    pub args_span: Span,
    /// 调用参数列表（按 `ExprKind::Call.args` 规则解析）。
    pub args: Vec<Expr>,
}

impl std::fmt::Debug for CtorDelegationCall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("CtorDelegationCall");
        s.field("span", &self.span);
        s.field("colon_span", &self.colon_span);
        s.field("kind", &self.kind);
        s.field("target_span", &self.target_span);
        s.field("args_span", &self.args_span);
        if !self.args.is_empty() {
            s.field("args", &self.args);
        }
        s.finish()
    }
}

/// class 次构造器（secondary constructor）：`constructor(params) [: this(...)|super(...)] { ... }`。
///
/// 说明：当前阶段（T0257）只解析并结构化存储：
/// - 参数列表（复用 `Param`）
/// - 可选 delegation call（仅 span）
/// - body block
#[derive(Clone)]
pub struct SecondaryCtorDecl {
    pub span: Span,
    pub annotations: Vec<AnnotationUse>,
    pub modifiers: Vec<Modifier>,
    pub params_span: Span,
    pub params: Vec<Param>,
    pub delegation_call: Option<CtorDelegationCall>,
    pub body: Block,
}

impl std::fmt::Debug for SecondaryCtorDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("SecondaryCtorDecl");
        s.field("span", &self.span);
        if !self.annotations.is_empty() {
            s.field("annotations", &self.annotations);
        }
        if !self.modifiers.is_empty() {
            s.field("modifiers", &self.modifiers);
        }
        s.field("params_span", &self.params_span);
        s.field("params", &self.params);
        if self.delegation_call.is_some() {
            s.field("delegation_call", &self.delegation_call);
        }
        s.field("body", &self.body);
        s.finish()
    }
}

/// 属性声明（spec §10.1）。
///
/// 当前阶段（T0234）仅用于 type body 内（class/interface/struct/enum/effect）的成员；
/// 顶层/局部 `val/var` 仍使用 `ValDecl`。
#[derive(Clone)]
pub struct PropertyDecl {
    pub span: Span,
    pub annotations: Vec<AnnotationUse>,
    pub modifiers: Vec<Modifier>,
    pub kind: ValKind,
    pub name: Ident,
    pub ty: Option<TypeRef>,
    /// 属性初始化表达式（例如 `var x: Int = 1`）。
    ///
    /// 注意：属性也可以是“纯计算属性”（无 backing field），此时 `init` 可能为 None。
    pub init: Option<Expr>,
    /// 委托表达式（delegated property）：`val/var x: T by expr`（spec §10.4）。
    ///
    /// 说明：
    /// - 委托属性与 `init`/accessors 在语义上互斥：getter/setter 会在 lowering 中生成；
    /// - 本字段只承载语法与后续 typecheck 所需信息，具体 `$delegate` 字段生成见 lowering 任务。
    pub delegate: Option<Expr>,
    /// 自定义 getter（`get()`）。
    pub getter: Option<AccessorDecl>,
    /// 自定义 setter（`set(value)`）。
    pub setter: Option<AccessorDecl>,
}

impl std::fmt::Debug for PropertyDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("PropertyDecl");
        s.field("span", &self.span);
        if !self.annotations.is_empty() {
            s.field("annotations", &self.annotations);
        }
        if !self.modifiers.is_empty() {
            s.field("modifiers", &self.modifiers);
        }
        s.field("kind", &self.kind);
        s.field("name", &self.name);
        s.field("ty", &self.ty);
        s.field("init", &self.init);
        if self.delegate.is_some() {
            s.field("delegate", &self.delegate);
        }
        s.field("getter", &self.getter);
        s.field("setter", &self.setter);
        s.finish()
    }
}

impl PropertyDecl {
    /// 该属性是否是“直接存储字段”。
    ///
    /// 用途：
    /// - `struct` 的 unified construction / default field / `with` / destructuring 主线只应
    ///   作用于真实存储字段；
    /// - 带 delegate 或 accessor 的属性属于计算/转发属性，不应被误当作 direct field。
    pub fn is_direct_field(&self) -> bool {
        self.delegate.is_none() && self.getter.is_none() && self.setter.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessorKind {
    Get,
    Set,
}

/// 属性 accessor 声明：`get()` / `set(value)`。
#[derive(Debug, Clone)]
pub struct AccessorDecl {
    pub span: Span,
    pub kind: AccessorKind,
    /// setter 的参数名（getter 为 None）。
    pub param: Option<Ident>,
    pub body: AccessorBody,
}

#[derive(Debug, Clone)]
pub enum AccessorBody {
    /// `{ ... }`
    Block(Block),
    /// `= expr`
    Expr(Expr),
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    Class,
    Interface,
    Struct,
    Enum,
    Effect,
}

/// 函数声明的语义分类。
///
/// 说明：
/// - `Regular`：普通函数/方法（顶层或 type body 内的 `fun`）。
/// - `EffectOp`：effect 声明体内的 operation 签名（spec §5.2）。
///
/// 当前阶段仅用于 parser 区分 operation 与普通方法；更完整的 typecheck 规则见 TODO T0602+。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunDeclKind {
    Regular,
    EffectOp,
}

#[derive(Clone)]
pub struct FunDecl {
    pub span: Span,
    pub kind: FunDeclKind,
    pub annotations: Vec<AnnotationUse>,
    pub modifiers: Vec<Modifier>,
    /// 扩展函数 receiver（`fun T.name(...)` 中的 `T`）。
    ///
    /// 当前阶段（T0233）仅在 parser 中解析并保留该 TypeRef；
    /// 分发规则与 codegen 会在后续任务中补齐。
    pub receiver: Option<TypeRef>,
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub eff_param: Option<EffectRowParam>,
    pub where_clause: Option<WhereClause>,
    pub params_span: Span,
    pub params: Vec<Param>,
    pub return_ty: Option<TypeRef>,
    /// 函数声明处的 effect row 标注：`/ Pure` / `/ E` / `/ (E1 + E2)`（spec §5.8）。
    pub effects: Option<EffectRowExpr>,
    pub body: FunBody,
}

impl std::fmt::Debug for FunDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("FunDecl");
        s.field("span", &self.span);
        if self.kind != FunDeclKind::Regular {
            s.field("kind", &self.kind);
        }
        if !self.annotations.is_empty() {
            s.field("annotations", &self.annotations);
        }
        if !self.modifiers.is_empty() {
            s.field("modifiers", &self.modifiers);
        }
        s.field("receiver", &self.receiver);
        s.field("name", &self.name);
        s.field("type_params", &self.type_params);
        if self.eff_param.is_some() {
            s.field("eff_param", &self.eff_param);
        }
        if self.where_clause.is_some() {
            s.field("where_clause", &self.where_clause);
        }
        s.field("params_span", &self.params_span);
        s.field("params", &self.params);
        s.field("return_ty", &self.return_ty);
        if self.effects.is_some() {
            s.field("effects", &self.effects);
        }
        s.field("body", &self.body);
        s.finish()
    }
}

#[derive(Debug, Clone)]
pub enum FunBody {
    Block(Block),
    Missing,
}

/// 顶层扩展属性声明（spec §10.3）。
///
/// 扩展属性与扩展函数类似：
/// - 声明处有 receiver（`val Receiver.name`）
/// - 编译模型为静态 getter/setter（receiver 作为第一个参数）
///
/// 注意：
/// - 当前阶段仅做语法建模与结构化存储；真正 lowering 留给 IR 阶段任务。
#[derive(Clone)]
pub struct ExtensionPropertyDecl {
    pub span: Span,
    pub annotations: Vec<AnnotationUse>,
    pub modifiers: Vec<Modifier>,
    pub kind: ValKind,
    pub type_params: Vec<TypeParam>,
    pub receiver: TypeRef,
    pub name: Ident,
    pub ty: Option<TypeRef>,
    /// 语法上允许写 initializer，但扩展属性语义上不允许生成 backing field；
    /// 因此是否允许 initializer 由 typecheck 阶段决定（TODO T0433）。
    pub init: Option<Expr>,
    pub getter: Option<AccessorDecl>,
    pub setter: Option<AccessorDecl>,
}

impl std::fmt::Debug for ExtensionPropertyDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("ExtensionPropertyDecl");
        s.field("span", &self.span);
        if !self.annotations.is_empty() {
            s.field("annotations", &self.annotations);
        }
        if !self.modifiers.is_empty() {
            s.field("modifiers", &self.modifiers);
        }
        s.field("kind", &self.kind);
        if !self.type_params.is_empty() {
            s.field("type_params", &self.type_params);
        }
        s.field("receiver", &self.receiver);
        s.field("name", &self.name);
        s.field("ty", &self.ty);
        s.field("init", &self.init);
        s.field("getter", &self.getter);
        s.field("setter", &self.setter);
        s.finish()
    }
}

#[derive(Debug, Clone)]
pub struct Block {
    pub span: Span,
    /// 块内语句列表。
    ///
    /// 当前阶段（T0207）仅保证：
    /// - 能解析空语句（`;`）与表达式语句（原子表达式）
    /// - 能解析局部 `val/var` 绑定语句（T0208）
    /// - 能解析 `return` / `return expr` 语句（T0226）
    /// - 其它语句形态会以 `StmtKind::Missing` 占位（保持 cursor 前进与括号平衡）
    pub stmts: Vec<Stmt>,
}

/// 表达式（最小子集）。
///
/// 说明：当前阶段只需要支撑 initializer 与函数体解析的增量推进，因此先保留一个非常小的集合。
/// 后续任务会逐步补齐调用、成员访问、二元运算、控制流等表达式节点。
#[derive(Debug, Clone)]
pub struct Expr {
    pub span: Span,
    pub kind: ExprKind,
}

/// 字段路径：`a.b.c`（用于值类型更新 `with` 表达式等场景）。
#[derive(Debug, Clone)]
pub struct FieldPath {
    pub span: Span,
    pub segments: Vec<Ident>,
}

/// `with` 更新项：`path: value`。
#[derive(Debug, Clone)]
pub struct WithUpdateField {
    pub span: Span,
    pub path: FieldPath,
    pub colon_span: Span,
    pub value: Expr,
}

/// typecheck 写回的 enum `with` copy-update 语义摘要。
///
/// 说明：
/// - key 仍由 `ExprKind::WithUpdate` 中的 path-prefix side table 管理；
/// - 这里只保存 lowering 需要的“当前 enum 的 concrete variant/field 形状”，
///   避免 HIR lowering 再次复刻 enum 泛型实参替换逻辑。
#[derive(Debug, Clone)]
pub struct WithUpdateResolvedEnum {
    pub enum_fqn: String,
    pub variants: Vec<WithUpdateResolvedEnumVariant>,
}

#[derive(Debug, Clone)]
pub struct WithUpdateResolvedEnumVariant {
    pub name: String,
    pub fields: Vec<WithUpdateResolvedEnumField>,
}

#[derive(Debug, Clone)]
pub struct WithUpdateResolvedEnumField {
    pub name: String,
    pub ty: TypeId,
}

/// struct literal 的字段初始化项：`name: expr`（spec §12）。
#[derive(Debug, Clone)]
pub struct StructLitField {
    pub span: Span,
    pub name: Ident,
    pub colon_span: Span,
    pub value: Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    // arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Rem,

    // ranges
    /// `a..b`（Appendix B.12）：语法级 range/progression（语义由后续 lowering/stdlib 决定）。
    RangeInclusive,

    // shifts
    Shl,
    Shr,

    // bitwise
    BitAnd,
    BitXor,
    BitOr,

    // comparisons
    Lt,
    Le,
    Gt,
    Ge,

    // equality
    Eq,
    Ne,

    // boolean logic
    LogAnd,
    LogOr,

    // null-coalescing / elvis
    Elvis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// `!expr`
    Not,
    /// `-expr`
    Neg,
    /// `~expr`
    BitNot,
}

/// 类型相关的表达式操作符：运行期类型判断。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeCheckOp {
    /// `expr is Type`
    Is,
    /// `expr !is Type`
    NotIs,
}

/// 类型相关的表达式操作符：显式转换。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastOp {
    /// `expr as Type`（失败时抛出/raise，由后续阶段决定）
    As,
    /// `expr as? Type`（失败时返回 `None`，由后续阶段决定）
    AsQ,
}

/// 插值字符串的片段（spec §8.2）。
#[derive(Debug, Clone)]
pub enum InterpolatedStringPart {
    /// 纯文本片段（保持源码 span；转义/去重写回等语义留给后续阶段）。
    Text { span: Span },
    /// 插值表达式片段：`{ expr }`。
    Expr { expr: Expr },
}

/// Lambda 表达式（spec §12）。
///
/// 语法形态（Kotlin 风格）：
/// - `{ params -> body }`
/// - `{ body }`
///
/// 说明：
/// - 参数类型注解可省略，通常由期望函数类型向下传播推断（spec §14.4）。
/// - `{ body }` 形式不含 `->`，因此 `arrow_span` 为 `None` 且 `params` 为空；隐式 `it` 等语义由后续 typecheck 决定。
#[derive(Clone)]
pub struct LambdaExpr {
    /// `@Safe` 的 span；普通 closure 为 `None`。
    pub at_safe_span: Option<Span>,
    /// 参数列表（参数类型可省略）。
    pub params: Vec<Param>,
    /// `->` 的 span；`{ body }` 形式为 `None`。
    pub arrow_span: Option<Span>,
    /// Lambda 主体表达式。若解析为 block body，可使用 `ExprKind::Block` 表示。
    pub body: Box<Expr>,
}

impl LambdaExpr {
    pub fn is_safe(&self) -> bool {
        self.at_safe_span.is_some()
    }
}

/// lambda 隐式单参数 `it` 的合成声明 span。
///
/// 约定：
/// - 使用 lambda 起始位置的零宽 span，避免与显式参数声明混淆；
/// - resolver 与 typecheck 共享该约定，以便 `ResolvedValueRef::Local` 与局部类型表对齐。
pub fn synthetic_lambda_implicit_it_decl_span(lambda_span: Span) -> Span {
    Span::new(lambda_span.start, lambda_span.start)
}

/// receiver lambda 隐式 `this` 的合成声明 span。
///
/// 约定：
/// - 使用 lambda 结束位置的零宽 span，与隐式 `it` 的起始零宽 span 区分；
/// - 该 span 只作为局部绑定身份标识，不对应源码中的显式声明。
pub fn synthetic_lambda_receiver_this_decl_span(lambda_span: Span) -> Span {
    Span::new(lambda_span.end, lambda_span.end)
}

impl std::fmt::Debug for LambdaExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("LambdaExpr");
        if let Some(at_safe_span) = self.at_safe_span {
            s.field("at_safe_span", &at_safe_span);
        }
        s.field("params", &self.params);
        s.field("arrow_span", &self.arrow_span);
        s.field("body", &self.body);
        s.finish()
    }
}

/// 值上下文中的标识符引用（expression ident）。
///
/// 说明：
/// - parser 阶段仅记录 `span`，不做任何名字解析；
/// - resolve 阶段（T0305）会把解析结果写回 `resolved`，用于后续 lowering/typecheck。
#[derive(Clone, PartialEq, Eq)]
pub struct ValueIdent {
    pub span: Span,
    pub resolved: Option<ResolvedValueRef>,
    /// 调用点解析信息（T0319）。
    ///
    /// 说明：
    /// - 仅当该 ident 出现在 `ExprKind::Call { callee: Ident(..) }` 的 callee 位置时才会被 resolver 写回；
    /// - 普通值引用解析仍使用 `resolved` 字段。
    pub call: Option<ResolvedCall>,
}

impl ValueIdent {
    pub fn new(span: Span) -> Self {
        Self {
            span,
            resolved: None,
            call: None,
        }
    }
}

impl std::fmt::Debug for ValueIdent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 为了保持 parse fixtures 的 AST snapshot 稳定回归：
        // - 未解析时输出应与旧版 `Ident { span }` 完全一致；
        // - 只有在 resolver 写回 `resolved` 后才额外打印该字段。
        let mut s = f.debug_struct("Ident");
        s.field("span", &self.span);
        if self.resolved.is_some() {
            s.field("resolved", &self.resolved);
        }
        if self.call.is_some() {
            s.field("call", &self.call);
        }
        s.finish()
    }
}

/// Resolver 写回到 AST 的“值引用”解析结果（T0305）。
///
/// 当前阶段的简化：
/// - `TopLevel` 暂只记录 FQN（Fully Qualified Name），不区分 fun/value；
/// - `Local` 使用声明处 binder span 作为最小“身份标识”。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedValueRef {
    /// 局部绑定（函数参数、block 内 `val/var`）。
    Local { name: String, decl_span: Span },
    /// 顶层符号（同包或通过 import 引入）。
    TopLevel { fqn: String },
}

/// Resolver 写回到 AST 的“调用点”解析结果（T0319）。
///
/// 说明：resolve 阶段不做最终 overload 决议，只负责收集候选集合与记录调用形状；
/// 真正的 most-specific/歧义诊断由后续 typecheck/inference 完成。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCall {
    /// 候选集合（可能为空；为空表示 resolver 无法在 name resolution 层面给出任何候选）。
    pub candidates: Vec<CallCandidate>,
    /// 调用形状：参数的“位置/命名”结构（不关心表达式内容）。
    pub shape: CallShape,
}

/// 调用候选（T0319）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallCandidate {
    /// 一个函数候选集合（FQN 指向 overload set）。
    Fun { fqn: String },
    /// 一个构造函数候选集合（type FQN 指向 constructors overload set）。
    Constructor { ty_fqn: String },
}

/// 调用形状（T0319）：记录实参的“位置参数/命名参数”结构。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallShape {
    pub args: Vec<CallArgShape>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallArgShape {
    Positional {
        span: Span,
    },
    Named {
        name: String,
        name_span: Span,
        value_span: Span,
    },
}

/// 成员访问中的标识符（`receiver.member` / `receiver?.member`）。
///
/// 说明：
/// - parser 阶段仅记录 `span`，不做任何名字解析；
/// - resolve 阶段（T0310）会把解析结果写回 `resolved`，用于后续 lowering/typecheck。
#[derive(Clone, PartialEq, Eq)]
pub struct MemberIdent {
    pub span: Span,
    pub resolved: Option<ResolvedMemberRef>,
    /// 调用点解析信息（T0319）。
    ///
    /// 说明：仅当 `receiver.member` 出现在 `Call` 的 callee 位置时写回。
    pub call: Option<ResolvedCall>,
}

impl MemberIdent {
    pub fn new(span: Span) -> Self {
        Self {
            span,
            resolved: None,
            call: None,
        }
    }
}

impl std::fmt::Debug for MemberIdent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 保持 parse fixtures 的 AST snapshot 稳定回归：
        // - 未解析时输出应与旧版 `Ident { span }` 完全一致；
        // - 只有在 resolver 写回 `resolved` 后才额外打印该字段。
        let mut s = f.debug_struct("Ident");
        s.field("span", &self.span);
        if self.resolved.is_some() {
            s.field("resolved", &self.resolved);
        }
        if self.call.is_some() {
            s.field("call", &self.call);
        }
        s.finish()
    }
}

/// Resolver 写回到 AST 的“成员引用”解析结果（T0310）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedMemberRef {
    /// 解析到类型体中的字段/属性（value namespace）。
    Value { fqn: String },
    /// 解析到类型体中的方法（fun namespace）。
    Fun { fqn: String },
    /// 解析到扩展属性（extension property）。
    ///
    /// 注意：当前阶段（T0312）主要用于 member access 的“同名优先级”与后续 lowering/typecheck；
    /// 具体 extension property 的语法与语义会在后续任务中逐步补齐。
    ExtensionValue { fqn: String },
    /// 解析到扩展函数（extension function）。
    ///
    /// 该变体表示 `receiver.member` 并非来自类型体成员，而是来自“同包可见”的扩展声明。
    ExtensionFun { fqn: String },
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    /// 解析失败或尚未实现时的占位节点（保持 span 以便诊断/回归）。
    Missing,
    Ident(ValueIdent),
    IntLit,
    FloatLit,
    CharLit,
    StringLit,
    /// `()`：Unit 字面量（spec §2.3.3）。
    UnitLit,
    /// tuple 字面量：`(a, b, ...)`（spec §2.3.3）。
    ///
    /// 说明：
    /// - 空 `()` 由 `ExprKind::UnitLit` 表示；
    /// - 单元素 tuple 需写 trailing comma：`(x,)`。
    TupleLit {
        elements: Vec<Expr>,
    },
    /// array 字面量：`[a, b, ...]`（spec §15.2 注解参数 / 后续 collections 语义）。
    ///
    /// 说明：
    /// - 当前阶段主要用于注解参数（T1019）：允许把一组常量打包成 `Array<T>` 传入注解；
    /// - 运行期数组/索引语义与 codegen 支持留给后续任务补齐。
    ArrayLit {
        elements: Vec<Expr>,
    },
    /// 插值字符串：`f"Hello, {name}!"` / `f"""...{x}..."""`（spec §8.2/§8.3）。
    ///
    /// lexer 会把整个 f-string 当作一个 token；parser 会把其拆分为 Text/Expr 片段列表。
    InterpolatedString {
        /// 是否为 raw f-string（`f"""..."""`）。
        raw: bool,
        parts: Vec<InterpolatedStringPart>,
    },
    Block(Block),
    /// `do { ... }`（spec §7.6）：显式局部 block 表达式。
    ///
    /// 说明：
    /// - 用户写的 `do { ... }` 在 AST 层表示为 `DoBlock`，与控制流内部使用的 `Block` 区分。
    /// - 语义与 `Block` 相同：立即求值，tail expression 决定值。
    /// - 裸 `{ ... }` 在表达式位置统一按 closure/lambda 规则解析。
    DoBlock {
        /// `do` 关键字的 span。
        do_span: Span,
        body: Block,
    },
    /// `@Unsafe do { ... }`（spec §15.9.2）：局部 unsafe context 块。
    ///
    /// 说明：
    /// - 该节点只负责表达“在该 block 内允许执行需要 unsafe context 的操作”；
    /// - 具体 unsafe 原语（例如 `Ptr<T>`/内存读写）由后续任务引入（T1009）；
    /// - typecheck 会在进入该 block 时 push unsafe depth，并在退出时 pop（T1004）。
    UnsafeBlock {
        /// `@Unsafe` 的 span（不包含 `{ ... }`）。
        at_unsafe_span: Span,
        body: Block,
    },
    /// `@Safe do { ... }`（spec §15.9.5）：在 unsafe context 内显式“收窄”为 safe 的区域。
    ///
    /// 说明：
    /// - 该节点只表达“在该 block 内禁止需要 unsafe context 的操作”；
    /// - typecheck 会在进入该 block 时暂时抑制外层 unsafe context；
    /// - `@Safe` 内仍允许嵌套 `@Unsafe do { ... }` 局部重新开启 unsafe（TODO T1021）。
    SafeBlock {
        /// `@Safe` 的 span（不包含 `{ ... }`）。
        at_safe_span: Span,
        body: Block,
    },
    /// Lambda 表达式：`{ params -> body }` / `{ body }` / `@Safe { ... }`（spec §12 / §15.9.5）。
    ///
    /// 当前任务（T0221）仅引入 AST 节点建模；解析与 `{}` 歧义消解见后续任务（T0222/T0225）。
    Lambda(LambdaExpr),
    /// struct literal：`TypeName { field: expr, ... }`（spec §12）。
    ///
    /// 说明：
    /// - 当前任务（T0223）仅引入 AST 节点建模；
    /// - 解析见后续任务（T0224）；
    /// - 与 lambda 的 `{}` 歧义消解见后续任务（T0225）；
    /// - 当前阶段只支持显式字段初始化 `name: expr`（不支持省略写法）。
    StructLit {
        ty: TypePath,
        fields: Vec<StructLitField>,
    },
    /// class 字面量：`TypeName::class`（Kotlin-like，T1019）。
    ///
    /// 说明：
    /// - 当前阶段把它视为“编译期可用的类型名常量”，供注解参数等语境使用；
    /// - 更完整的 TypeMeta/反射语义由后续 comptime/reflection 任务落地（T1204/T1208）。
    ClassLit {
        ty: TypeRef,
    },
    /// `if (cond) thenExpr else elseExpr?`（表达式形式）。
    ///
    /// 说明：
    /// - 当前阶段（T0214）只支持括号条件；
    /// - `else` 允许缺省（语义由后续 typecheck 决定）。
    If {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Option<Box<Expr>>,
    },
    /// `when (subject) { pat -> expr; ... }`（表达式形式）。
    ///
    /// 当前阶段（T0215）仅支持 very small pattern 子集：
    /// - `is Type`
    /// - `else`
    /// - 常量字面量（int/string）
    ///
    /// 穷尽性检查与完整 pattern 语义留到后续 typecheck 阶段实现（spec §4）。
    When {
        subject: Box<Expr>,
        arms: Vec<WhenArm>,
    },
    /// `handle { ... } with { ... }`（spec §5.4）。
    ///
    /// 说明：
    /// - 支持 non-resuming arm：`Effect.op(args) -> body`；
    /// - 支持 escape continuation arm：`Effect.op(args), k -> body`（T0617）；
    /// - `finally { ... }` 目前仅做语法建模（完整语义见 spec §5.7 / 后续 lowering）。
    Handle {
        body: Block,
        arms: Vec<HandleArm>,
        finally: Option<Block>,
    },
    /// `async { ... }`（spec §5.7）：作为 `Async` effect 的语法糖。
    ///
    /// 说明：
    /// - 该节点只负责保留语法结构（关键字 + block）；
    /// - 具体 desugar（例如 lowering 到 `handle` + `Async.await`）由后续阶段决定；
    /// - 早期阶段实现会以“可回归”为目标，先落地一个最小可执行语义（单线程、无取消）。
    Async {
        body: Block,
    },
    /// `spawn { ... }`（spec §5.7）：为后续 structured concurrency 保留的语法壳。
    ///
    /// 说明：
    /// - 该节点只负责保留语法结构（关键字 + block）；
    /// - 当前阶段不会把它纳入 `Task` core 语义；typecheck 会显式报“留待后续”；
    /// - 真正的 structured concurrency / 调度 / 取消语义留待后续任务定型。
    Spawn {
        body: Block,
    },
    /// `await expr`（spec §5.7）：作为 `Async.await(...)` 的语法糖。
    ///
    /// 说明：
    /// - `await` 只在语法层作为前缀操作符存在；
    /// - 后续阶段会把它 lower 成一次 `Async` effect operation 的 perform 点。
    Await {
        await_span: Span,
        expr: Box<Expr>,
    },
    /// `join expr`：为后续 structured concurrency 保留的语法壳。
    ///
    /// 说明：
    /// - 当前阶段不会把它纳入 `Task` core 语义；typecheck 会显式报“留待后续”；
    /// - 真正的 structured concurrency / 调度 / 取消语义留待后续任务补齐。
    Join {
        join_span: Span,
        expr: Box<Expr>,
    },
    /// 成员访问表达式：`receiver.member`（postfix）。
    ///
    /// 说明：
    /// - 当前阶段仅建模普通 `.` 成员访问；
    /// - safe-call（`?.`）使用单独的 `ExprKind::SafeMemberAccess` 表示。
    MemberAccess {
        receiver: Box<Expr>,
        member: MemberIdent,
    },
    /// Splice 字段访问：`receiver.[field]`（spec §6.4）。
    ///
    /// 说明：
    /// - 该语法用于在 `comptime` 语境下通过 `FieldMeta` 动态选择字段；
    /// - 当前阶段仅做语法建模；合法性（只能用于 `comptime for` 且 `field` 为 `FieldMeta`）由后续阶段实现。
    SpliceField {
        receiver: Box<Expr>,
        field: Box<Expr>,
    },
    /// safe-call 成员访问表达式：`receiver?.member`（postfix）（Appendix B.3.1）。
    ///
    /// 说明：仅做语法建模；desugar/运行期语义留到后续阶段（typecheck/lowering）决定。
    SafeMemberAccess {
        receiver: Box<Expr>,
        op_span: Span,
        member: MemberIdent,
    },
    /// 显式类型实参应用：`callee<T1, T2, ...>`（Kotlin-like）。
    ///
    /// 说明：
    /// - 该节点用于把“值位置的 callee”与一组显式类型实参绑定在一起；
    /// - 典型用法是紧跟一次调用：`nameOf<T>()` / `fieldsOf<T>()`；
    /// - 为避免修改 `ExprKind::Call` 的 Debug 形态导致大量 AST golden 漂移，
    ///   这里选择引入独立的 `TypeApply` 节点作为轻量承载。
    TypeApply {
        callee: Box<Expr>,
        args: Vec<TypeRef>,
    },
    /// 调用表达式：`callee(args...)`（postfix）。
    ///
    /// 当前阶段：
    /// - （T0209）支持位置参数与逗号分隔参数列表；
    /// - （T0231）支持命名参数实参：`name = expr`（仅在参数列表中生效）；
    /// - （T0232）支持 Kotlin 风格 trailing lambda：`callee { ... }` 与 `callee(args) { ... }`（作为最后一个实参）。
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    /// spread 参数实参：`*expr`（Appendix B.5.5，Kotlin-like）。
    ///
    /// 说明：
    /// - 该节点只应出现在 `ExprKind::Call.args`（含命名参数的 value）内；
    /// - 语义（仅允许用于 vararg 参数、可 spread 的容器类型等）由 typecheck 负责（TODO T1308）。
    SpreadArg {
        star_span: Span,
        expr: Box<Expr>,
    },
    /// 命名参数实参：`name = expr`（Appendix B.5.3）。
    ///
    /// 说明：
    /// - 该节点**仅**应由调用参数列表解析产生（T0231），用于与赋值表达式 `ExprKind::Assign` 区分；
    /// - 参数重排、默认值补齐等调用语义留到后续阶段（typecheck/lowering）处理。
    NamedArg {
        name: Ident,
        eq_span: Span,
        value: Box<Expr>,
    },
    /// 非空断言：`expr!!`（postfix）。
    ///
    /// 说明：仅做语法建模；运行期异常语义留到后续阶段（typecheck/effect/codegen）决定。
    NotNullAssert {
        expr: Box<Expr>,
        op_span: Span,
    },
    /// 前缀一元运算：`!expr` / `-expr` / `~expr`（spec §2.3.4 / Appendix B.8）。
    Unary {
        op: UnaryOp,
        op_span: Span,
        expr: Box<Expr>,
    },
    /// 二元运算表达式：`lhs op rhs`。
    ///
    /// 说明：
    /// - 当前阶段（T0211）仅实现常见二元运算符的优先级与结合性（语法层面）；
    /// - 操作符重载与类型规则在 typecheck 阶段处理。
    Binary {
        lhs: Box<Expr>,
        op: BinaryOp,
        op_span: Span,
        rhs: Box<Expr>,
    },
    /// 赋值表达式：`lhs = rhs`。
    ///
    /// 说明：
    /// - 当前阶段（T0227）仅在语法层支持最小 lhs：标识符与成员访问（`a.b`）；
    /// - 复合赋值（`+=` 等）与解构赋值留到后续任务实现。
    Assign {
        lhs: Box<Expr>,
        eq_span: Span,
        rhs: Box<Expr>,
    },
    /// 运行期类型判断：`expr is Type` / `expr !is Type`。
    ///
    /// 说明：
    /// - 仅做语法建模；smart cast 与运行期语义留到后续阶段处理（typecheck/effect/codegen）。
    TypeCheck {
        expr: Box<Expr>,
        op: TypeCheckOp,
        op_span: Span,
        ty: TypeRef,
    },
    /// 显式类型转换：`expr as Type` / `expr as? Type`。
    ///
    /// 说明：仅做语法建模；失败语义（raise/返回 None）留到后续阶段处理。
    Cast {
        expr: Box<Expr>,
        op: CastOp,
        op_span: Span,
        ty: TypeRef,
    },
    /// 值类型更新表达式：`expr with { path: value, ... }`（spec §2.6）。
    ///
    /// 说明：
    /// - 语法建模见 T0216；
    /// - 字段存在性与类型检查已在 typecheck 阶段实现（T0415）；
    /// - HIR lowering 会按具体值类型把 `with` 展开为 copy-update block / `when`
    ///   重建（struct/tuple/enum）；
    /// - enum 路径以 variant 名开头，例如 `result with { Ok.point.x: 1 }`。
    WithUpdate {
        base: Box<Expr>,
        with_span: Span,
        updates: Vec<WithUpdateField>,
        /// typecheck 写回的 copy-update 路径前缀 -> 具体 aggregate type 映射。
        ///
        /// 约定：
        /// - `""` = base 表达式自身的具体值类型；
        /// - `"start"` / `"_0"` / `"start._0"` = 对应中间路径前缀的具体 aggregate type。
        ///
        /// lowering 会把这些 `TypeId` 重新 intern 到自己的 `TypeStore`，从而按 struct/tuple
        /// /enum 统一重建 aggregate，而不是只靠 struct FQN 特判。
        /// 使用 `OnceCell` 允许 typecheck 以共享引用写回。
        resolved_copy_update_tys: OnceCell<std::collections::HashMap<String, TypeId>>,
        /// typecheck 写回的 enum prefix -> concrete variant/field 形状。
        ///
        /// 约定：
        /// - `""` = base 表达式自身就是 enum；
        /// - `"Ok.point"` / `"payload.Result"` = 路径前缀落到某个 enum 值时的具体 enum 信息。
        ///
        /// lowering 读取后可直接把 enum copy-update 收口为 `when` + variant ctor 重建，
        /// 而不必在 AST/HIR 之间重复 enum 泛型实参 substitution。
        resolved_copy_update_enums:
            OnceCell<std::collections::HashMap<String, WithUpdateResolvedEnum>>,
    },
}

impl Expr {
    pub fn missing(span: Span) -> Self {
        Self {
            span,
            kind: ExprKind::Missing,
        }
    }
}

/// `when` 的一个分支（arm）：`pat -> body`。
#[derive(Debug, Clone)]
pub struct WhenArm {
    pub span: Span,
    pub pat: WhenPat,
    /// 可选 guard：`pat if <expr> -> body`。
    pub guard: Option<Expr>,
    pub arrow_span: Span,
    pub body: Expr,
}

/// `handle` 的一个 handler arm：`Effect.op(args...) -> body`。
#[derive(Clone)]
pub struct HandleArm {
    pub span: Span,
    pub op: HandleOp,
    pub arrow_span: Span,
    pub kind: HandleArmKind,
    pub body: Expr,
}

/// `handle` 的 handler arm 形式（spec §5.4）。
///
/// 说明：
/// - non-resuming：`Effect.op(...) -> expr`（try/catch lowering 产物属于该类）。
/// - escape-continuation：`Effect.op(...), k -> expr`。
#[derive(Debug, Clone, Copy)]
pub enum HandleArmKind {
    /// `->`：非恢复 arm；handled computation 被放弃。
    NonResuming,
    /// `, k ->`：逃逸 continuation arm；`k` 是显式 continuation binder。
    ///
    /// 说明：当前阶段仅做语法建模；continuation 的运行期语义由 lowering/codegen（T0617+）落地。
    EscapeContinuation {
        /// continuation binder（例如 `k`）在源码中的 span（用于 resolver/typecheck 注入 `k` 符号）。
        k_span: Span,
    },
}

impl std::fmt::Debug for HandleArm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 为了保持 parse fixtures 的 AST snapshot 尽量稳定：
        // - non-resuming arm 不额外打印 kind（与旧版输出保持一致）
        let mut s = f.debug_struct("HandleArm");
        s.field("span", &self.span);
        s.field("op", &self.op);
        s.field("arrow_span", &self.arrow_span);
        if let HandleArmKind::EscapeContinuation { k_span } = self.kind {
            s.field("k_span", &k_span);
        }
        s.field("body", &self.body);
        s.finish()
    }
}

/// handler arm head 中的 effect operation：`Effect<T>.op<U>(binders...)`。
///
/// 注意：这里的 `binders` 是 **参数绑定**（类似模式参数），不是普通调用表达式的实参表达式。
#[derive(Clone)]
pub struct HandleOp {
    pub span: Span,
    pub effect: TypePath,
    pub dot_span: Span,
    pub op: Ident,
    pub op_type_args: Vec<TypeRef>,
    pub binders: Vec<HandleBinder>,
}

impl std::fmt::Debug for HandleOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("HandleOp");
        s.field("span", &self.span);
        s.field("effect", &self.effect);
        s.field("dot_span", &self.dot_span);
        s.field("op", &self.op);
        if !self.op_type_args.is_empty() {
            s.field("op_type_args", &self.op_type_args);
        }
        s.field("binders", &self.binders);
        s.finish()
    }
}

/// handler arm 的一个参数绑定：`name` 或 `name: Type`。
#[derive(Debug, Clone)]
pub struct HandleBinder {
    pub span: Span,
    pub name: Ident,
    pub colon_span: Option<Span>,
    pub ty: Option<TypeRef>,
}

/// `when` 分支的模式（早期最小子集）。
#[derive(Debug, Clone)]
pub enum WhenPat {
    Else {
        span: Span,
    },
    /// or-pattern：`A | B | C`
    ///
    /// 说明：
    /// - 该语法仅用于 `when` 分支头；
    /// - 当前阶段语义与更完整的 pattern 系统仍在逐步补齐（see TODO）。
    Or {
        span: Span,
        pats: Vec<WhenPat>,
    },
    Is {
        is_span: Span,
        ty: TypeRef,
    },
    /// `_`：通配符模式（匹配任意值）。
    Wildcard {
        span: Span,
    },
    /// rest：`..`（忽略剩余字段/元素；仅允许出现在 tuple/variant pattern 内）。
    Rest {
        span: Span,
    },
    /// 绑定变量模式：`x`（把匹配到的值绑定到变量 `x`）。
    ///
    /// 说明：该绑定仅在当前 when arm 的 body 作用域内可见（由 resolver 建立作用域）。
    Bind {
        ident: Ident,
    },
    /// tuple 模式：`(p1, p2, ...)`。
    Tuple {
        span: Span,
        elements: Vec<WhenPat>,
    },
    /// enum variant 模式：`Some(x)` / `None`（0 参数 variant）。
    ///
    /// 说明：
    /// - 早期阶段仅支持“位置参数”的 variant pattern；
    /// - `name` 目前不做解析写回，由 typecheck 基于 subject 的 enum 类型做约束与匹配。
    Variant {
        span: Span,
        name: Ident,
        args: Vec<WhenPat>,
    },
    IntLit {
        span: Span,
    },
    CharLit {
        span: Span,
    },
    StringLit {
        span: Span,
    },
    /// `true` / `false`（当前阶段 lexer 仍以 ident token 承载）。
    BoolLit {
        span: Span,
    },
}

impl WhenPat {
    pub fn span(&self) -> Span {
        match self {
            WhenPat::Else { span } => *span,
            WhenPat::Or { span, .. } => *span,
            WhenPat::Is { is_span, ty } => Span::new(is_span.start, ty.span().end),
            WhenPat::Wildcard { span } => *span,
            WhenPat::Rest { span } => *span,
            WhenPat::Bind { ident } => ident.span,
            WhenPat::Tuple { span, .. } => *span,
            WhenPat::Variant { span, .. } => *span,
            WhenPat::IntLit { span } => *span,
            WhenPat::CharLit { span } => *span,
            WhenPat::StringLit { span } => *span,
            WhenPat::BoolLit { span } => *span,
        }
    }
}

/// 模式（pattern）——用于 `val` 解构绑定等语法位置。
///
/// 注意：当前阶段只实现解构绑定所需的最小子集（T0244、T0460）：
/// - `_` wildcard
/// - `..` rest（忽略剩余字段/元素/参数；仅允许出现在 tuple/struct/variant pattern 内）
/// - 绑定标识符（bind）
/// - enum variant pattern：`Some(x)` / `Result.Ok(v)`
/// - tuple pattern：`(p1, p2, ...)`
/// - struct pattern：`TypeName { field, field: pat, .. }`
#[derive(Debug, Clone)]
pub struct Pattern {
    pub span: Span,
    pub kind: PatternKind,
}

#[derive(Debug, Clone)]
pub enum PatternKind {
    Wildcard,
    /// rest：`..`（仅用于 tuple/struct/variant pattern 内的占位，表示忽略剩余元素/字段/参数）。
    Rest,
    Bind(Ident),
    /// enum variant pattern：`Some(x)` / `Result.Ok(v)`。
    ///
    /// 说明：
    /// - 该 pattern 复用 `when` 的 variant destructuring 语义；
    /// - `path` 允许写 `Enum.Variant` 形式以消歧；
    /// - 当前阶段仅支持“位置参数”的 payload 解构（不支持命名字段）。
    Variant {
        path: TypePath,
        args: Vec<Pattern>,
    },
    Tuple(Vec<Pattern>),
    Struct {
        path: TypePath,
        fields: Vec<StructPatternField>,
        /// `Some(..)` 表示出现了 `..` rest（并记录其 span，便于诊断）。
        rest: Option<Span>,
    },
    Missing,
}

impl Pattern {
    pub fn missing(span: Span) -> Self {
        Self {
            span,
            kind: PatternKind::Missing,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StructPatternField {
    pub span: Span,
    pub name: Ident,
    /// `None` 表示 shorthand：`Point { x }` 等价于 `Point { x: x }`（语义留给后续阶段）。
    pub value: Option<Box<Pattern>>,
}

/// `comptime if` 语句（spec §6.3）。
///
/// 说明：分支裁剪与“未选中分支不做类型检查”等语义由后续阶段实现；当前阶段仅做语法建模。
#[derive(Debug, Clone)]
pub struct ComptimeIf {
    pub span: Span,
    pub comptime_span: Span,
    pub if_span: Span,
    pub cond: Expr,
    pub then_branch: Block,
    pub else_branch: Option<Box<ComptimeIfElse>>,
}

#[derive(Debug, Clone)]
pub enum ComptimeIfElse {
    Block(Block),
    If(Box<ComptimeIf>),
}

/// `comptime for (x in xs) { ... }` 语句（spec §6.3）。
#[derive(Debug, Clone)]
pub struct ComptimeFor {
    pub span: Span,
    pub comptime_span: Span,
    pub for_span: Span,
    pub binder: Ident,
    pub in_span: Span,
    pub iter: Expr,
    pub body: Block,
}

/// `for (x in xs) { ... }` 语句（Kotlin-like，Appendix B.12）。
///
/// 说明：
/// - 语义上会被 lowering 为迭代协议（`iterator`/`next(): Option<T>`）；
/// - typecheck 写回 `resolved_for_info`，HIR lowering 读取以做类型特化降糖。
#[derive(Debug, Clone)]
pub struct ForStmt {
    pub span: Span,
    pub for_span: Span,
    pub binder: Ident,
    pub in_span: Span,
    pub iter: Expr,
    pub body: Block,
    /// typecheck 写回的 for-loop 降糖信息（T0110）。
    pub resolved_for_info: OnceCell<ForLoopResolvedInfo>,
}

/// typecheck → HIR lowering 间传递的 for-loop 降糖信息（T0110）。
#[derive(Debug, Clone)]
pub struct ForLoopResolvedInfo {
    pub kind: ForLoopIterableKind,
    /// 仅 `Custom` 路径使用：记录 lowering 所需的稳定调用目标与元素类型。
    pub custom: Option<ForLoopCustomResolvedInfo>,
}

/// for-in 迭代器的底层类型类别（决定 HIR lowering 的降糖策略）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForLoopIterableKind {
    /// `Array<Int>` — 降糖为基于索引的 while 循环（`size`/`get`）。
    ArrayInt,
    /// `IntProgression` — 降糖为 progression while 循环（字段访问）。
    IntProgression,
    /// 其它实现了 iterator()/next() 协议的类型。
    Custom,
}

/// 自定义 iterable 在 typecheck 后写回给 HIR lowering 的额外信息。
#[derive(Debug, Clone)]
pub struct ForLoopCustomResolvedInfo {
    /// `iterator()` 的静态调用目标（例如 `pkg.Iterable.iterator`）。
    pub iterator_method_fqn: String,
    /// `iterator()` 的返回类型（typecheck `TypeStore` 中的 TypeId）。
    pub iterator_ty: TypeId,
    /// `next()` 的静态调用目标（例如 `pkg.Iterator.next`）。
    pub next_method_fqn: String,
    /// `next(): Option<T>` 中的 `T`（typecheck `TypeStore` 中的 TypeId）。
    pub elem_ty: TypeId,
}

/// 语句（最小骨架）。
///
/// 目前阶段仅为后续 block 解析预留结构；T0207/T0208 会逐步扩展其子集。
#[derive(Debug, Clone)]
pub struct Stmt {
    pub span: Span,
    pub kind: StmtKind,
    /// 该语句是否以 `;` 结尾。
    ///
    /// 用于 block tail value 语义判定（T3102）：当 block 最后一条语句是
    /// `StmtKind::Expr` 且 `has_trailing_semi == true` 时，该表达式视为
    /// expression statement，block 值为 `Unit`；反之为 tail expression，
    /// block 值为该表达式的类型。
    pub has_trailing_semi: bool,
}

#[derive(Debug, Clone)]
pub enum StmtKind {
    Empty,
    Expr(Expr),
    Val(ValDecl),
    /// `return` / `return expr`（spec §7.1/§7.3）。
    ///
    /// 说明：
    /// - 当前阶段仅支持 block 内的 return 语句；
    /// - label/non-local return 的语义留到后续阶段处理。
    Return {
        return_span: Span,
        value: Option<Expr>,
    },
    /// `while (cond) { ... }`（PLAN §4.6）。
    ///
    /// 说明：
    /// - 当前阶段只做语法解析；不在 parser 中检查 `break/continue` 的位置合法性。
    While {
        while_span: Span,
        cond: Expr,
        body: Block,
    },
    /// `break`（PLAN §4.6）。
    Break {
        break_span: Span,
    },
    /// `continue`（PLAN §4.6）。
    Continue {
        continue_span: Span,
    },
    /// `for (x in xs) { ... }`（Appendix B.12）。
    For(ForStmt),
    /// `comptime { ... }` 执行块（spec §6）。
    ///
    /// 说明：当前阶段仅做语法建模；真正的编译期执行入口在后续阶段实现（见 TODO T12xx）。
    ComptimeBlock {
        comptime_span: Span,
        body: Block,
    },
    /// `comptime if (...) { ... } else ...`（spec §6.3）。
    ComptimeIf(ComptimeIf),
    /// `comptime for (x in xs) { ... }`（spec §6.3）。
    ComptimeFor(ComptimeFor),
    Missing,
}

#[derive(Clone)]
pub struct Param {
    /// 参数上的注解列表（spec §15.3）。
    ///
    /// 说明：
    /// - 同一个参数在不同语境下可能映射到不同“可注解元素”：
    ///   - 普通函数参数：主要目标是 Param；
    ///   - 主构造 `val/var` 参数：同时是 Param + Property + Field（可用 use-site target 前缀区分）。
    /// - 当前阶段仅做语法解析与结构化存储；具体附着语义由后续任务逐步补齐（T1016/T1208...）。
    pub annotations: Vec<AnnotationUse>,
    /// 主构造参数中的 `val/var` 前缀（Kotlin-like）。
    ///
    /// 说明：
    /// - 普通函数参数与 lambda 参数不支持 `val/var`，因此为 `None`；
    /// - 对于 `class C(val x: Int)`，`kind = Some(Val)` 表示该参数同时声明一个同名字段/属性；
    /// - 对于 `class C(x: Int)`，`kind = None` 表示它只是构造参数（仅在初始化语境与成员体内可见）。
    pub kind: Option<ValKind>,
    /// `vararg` 参数标记（Appendix B.5.5）。
    ///
    /// 说明：
    /// - `vararg x: T` 在调用点可接受 0..N 个实参；
    /// - 更完整的“spread / collection 转换”语义由 typecheck 逐步补齐（见 TODO T1308）。
    pub is_vararg: bool,
    pub name: Ident,
    pub ty: Option<TypeRef>,
    pub default_value: Option<Expr>,
}

impl std::fmt::Debug for Param {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("Param");
        if !self.annotations.is_empty() {
            s.field("annotations", &self.annotations);
        }
        s.field("name", &self.name);
        if self.kind.is_some() {
            s.field("kind", &self.kind);
        }
        if self.is_vararg {
            s.field("is_vararg", &self.is_vararg);
        }
        s.field("ty", &self.ty);
        if self.default_value.is_some() {
            s.field("default_value", &self.default_value);
        }
        s.finish()
    }
}

#[derive(Clone)]
pub struct ValDecl {
    pub span: Span,
    pub annotations: Vec<AnnotationUse>,
    pub modifiers: Vec<Modifier>,
    pub kind: ValKind,
    pub binding: ValBinding,
    pub ty: Option<TypeRef>,
    /// 初始化表达式（当前阶段可能为 `ExprKind::Missing`，后续任务会逐步补齐解析）。
    pub init: Option<Expr>,
}

#[derive(Clone)]
pub enum ValBinding {
    Name(Ident),
    Pattern(Pattern),
}

impl ValBinding {
    pub fn bound_idents(&self) -> Vec<Ident> {
        let mut out = Vec::new();
        match self {
            ValBinding::Name(name) => out.push(*name),
            ValBinding::Pattern(pattern) => collect_pattern_bound_idents(pattern, &mut out),
        }
        out
    }
}

impl ValDecl {
    pub fn name(&self) -> Option<Ident> {
        match self.binding {
            ValBinding::Name(name) => Some(name),
            ValBinding::Pattern(_) => None,
        }
    }
}

impl std::fmt::Debug for ValDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("ValDecl");
        s.field("span", &self.span);
        if !self.annotations.is_empty() {
            s.field("annotations", &self.annotations);
        }
        if !self.modifiers.is_empty() {
            s.field("modifiers", &self.modifiers);
        }
        s.field("kind", &self.kind);
        match &self.binding {
            ValBinding::Name(name) => s.field("name", name),
            ValBinding::Pattern(pat) => s.field("pattern", pat),
        };
        s.field("ty", &self.ty);
        s.field("init", &self.init);
        s.finish()
    }
}

fn collect_pattern_bound_idents(pattern: &Pattern, out: &mut Vec<Ident>) {
    match &pattern.kind {
        PatternKind::Bind(ident) => out.push(*ident),
        PatternKind::Tuple(elements) => {
            for element in elements {
                collect_pattern_bound_idents(element, out);
            }
        }
        PatternKind::Struct { fields, .. } => {
            for field in fields {
                match field.value.as_deref() {
                    Some(nested) => collect_pattern_bound_idents(nested, out),
                    None => out.push(field.name),
                }
            }
        }
        PatternKind::Variant { args, .. } => {
            for arg in args {
                collect_pattern_bound_idents(arg, out);
            }
        }
        PatternKind::Wildcard | PatternKind::Rest | PatternKind::Missing => {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValKind {
    Val,
    Var,
}

#[derive(Debug, Clone)]
pub enum TypeRef {
    Path(TypePath),
    Tuple(TypeTuple),
    /// 星投影（star projection）：仅允许出现在类型实参位置，例如 `List<*>`。
    ///
    /// 说明：当前阶段（T0249）仅做语法层解析与存储；语义由后续 typecheck 实现。
    Star {
        span: Span,
    },
    /// use-site effect row 实参：仅允许出现在类型实参列表末尾，例如 `Disposable<eff Pure>`。
    ///
    /// 说明：
    /// - `eff` 是上下文关键字：仅在类型实参列表 `<...>` 内被当作关键字处理；
    /// - 当前阶段（T0253）仅做语法层解析与存储；与声明处 `eff` 参数的匹配与合法性检查留给 typecheck。
    EffectRowArg {
        span: Span,
        row: EffectRowExpr,
    },
    /// 函数类型（spec §7.5）：`(A, B) -> C / R` 或 `T.(A, B) -> C / R`
    Function(TypeFunction),
    Nullable {
        span: Span,
        inner: Box<TypeRef>,
    },
}

/// 函数类型（type position）。
///
/// 说明：
/// - `receiver` 对应 receiver function type：`T.(...) -> ...`
/// - `effects` 对应可选的 effect row：`/ Pure` 或 `/ (E1 + E2)`
#[derive(Debug, Clone)]
pub struct TypeFunction {
    pub span: Span,
    pub receiver: Option<Box<TypeRef>>,
    pub params_span: Span,
    pub params: Vec<TypeRef>,
    pub return_ty: Box<TypeRef>,
    pub effects: Option<EffectRowExpr>,
}

/// effect row 表达式（spec §5.8）。
///
/// 当前阶段只需要语法结构：
/// - `Pure`（空 effect row）
/// - `E1 + E2 + ...`（并集；项为 effect 名/row 变量的路径）
/// - `E!`（闭合 effect row，spec §5.8.4；语义见 TODO T0627）
#[derive(Clone)]
pub struct EffectRowExpr {
    pub span: Span,
    /// `terms.is_empty()` 表示 `Pure`。
    pub terms: Vec<TypePath>,
    /// 是否为闭合 effect row（`E!`）。
    ///
    /// 说明：`!` 的优先级低于 `+`，它作用于整个 row 表达式，而不是最后一个项。
    pub closed: bool,
}

impl std::fmt::Debug for EffectRowExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 为了保持现有 parse fixtures 的 AST snapshot 稳定：
        // - open row（`closed=false`）不打印 `closed` 字段，格式与旧版完全一致；
        // - 仅当闭合 row 时才显式打印 `closed: true`。
        let mut s = f.debug_struct("EffectRowExpr");
        s.field("span", &self.span);
        s.field("terms", &self.terms);
        if self.closed {
            s.field("closed", &true);
        }
        s.finish()
    }
}

#[derive(Debug, Clone)]
pub struct TypePath {
    pub span: Span,
    pub segments: Vec<Ident>,
    pub args: Vec<TypeRef>,
}

#[derive(Debug, Clone)]
pub struct TypeTuple {
    pub span: Span,
    pub elements: Vec<TypeRef>,
}

impl TypeRef {
    pub fn span(&self) -> Span {
        match self {
            TypeRef::Path(p) => p.span,
            TypeRef::Tuple(t) => t.span,
            TypeRef::Star { span } => *span,
            TypeRef::EffectRowArg { span, .. } => *span,
            TypeRef::Function(f) => f.span,
            TypeRef::Nullable { span, .. } => *span,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Ident {
    pub span: Span,
    /// 对于 parser 产生的“语法糖”节点（例如 try/catch lowering），某些标识符并不直接来自源文本。
    ///
    /// 当该字段为 `Some(...)` 时，后续 resolve/typecheck 应优先使用这里的字面文本，
    /// 而不是通过 `span` 回切当前源文件。
    pub text: Option<&'static str>,
}

impl Ident {
    pub fn new(span: Span) -> Self {
        Self { span, text: None }
    }

    pub fn synthetic(span: Span, text: &'static str) -> Self {
        Self {
            span,
            text: Some(text),
        }
    }

    pub fn text<'a>(&self, source: &'a crate::source::SourceFile) -> &'a str {
        match self.text {
            Some(t) => t,
            None => source.slice(self.span),
        }
    }
}

impl std::fmt::Debug for Ident {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 为了保持 parse fixtures 的 AST snapshot 稳定回归：
        // - 常规 Ident 只打印 span（与旧版完全一致）；
        // - 仅当 parser 生成了合成标识符时才额外打印 text，用于调试/回归 try-catch lowering。
        let mut s = f.debug_struct("Ident");
        s.field("span", &self.span);
        if let Some(text) = self.text {
            s.field("text", &text);
        }
        s.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lambda_ast_node_is_constructible() {
        let arrow_span = Span::new(3, 5);
        let body = Expr {
            span: Span::new(5, 6),
            kind: ExprKind::IntLit,
        };
        let lambda = Expr {
            span: Span::new(0, 6),
            kind: ExprKind::Lambda(LambdaExpr {
                at_safe_span: None,
                params: vec![Param {
                    annotations: Vec::new(),
                    kind: None,
                    is_vararg: false,
                    name: Ident::new(Span::new(1, 2)),
                    ty: None,
                    default_value: None,
                }],
                arrow_span: Some(arrow_span),
                body: Box::new(body),
            }),
        };

        match &lambda.kind {
            ExprKind::Lambda(l) => {
                assert_eq!(l.params.len(), 1);
                assert_eq!(l.arrow_span, Some(arrow_span));
            }
            other => panic!("expected Lambda, got {other:?}"),
        }
    }

    #[test]
    fn struct_lit_ast_node_is_constructible() {
        let ty = TypePath {
            span: Span::new(0, 5),
            segments: vec![Ident::new(Span::new(0, 5))],
            args: vec![],
        };

        let field_name = Ident::new(Span::new(8, 9));
        let value = Expr {
            span: Span::new(11, 12),
            kind: ExprKind::IntLit,
        };
        let field = StructLitField {
            span: Span::new(8, 12),
            name: field_name,
            colon_span: Span::new(9, 10),
            value,
        };

        let lit = Expr {
            span: Span::new(0, 13),
            kind: ExprKind::StructLit {
                ty,
                fields: vec![field],
            },
        };

        match &lit.kind {
            ExprKind::StructLit { ty, fields } => {
                assert_eq!(ty.segments.len(), 1);
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name.span, field_name.span);
            }
            other => panic!("expected StructLit, got {other:?}"),
        }
    }
}
