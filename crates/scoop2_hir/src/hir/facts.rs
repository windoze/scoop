//! typed HIR 语义事实侧表（MIR lowering 消费用）。
//!
//! 本模块定义 typecheck 决议点产出的「语义事实」数据类型，以及 per-file 的
//! [`SemanticFacts`] 容器。这些事实由 `typecheck::expr` 在**不改算法**的前提下
//! 于决议成功点写入（路径 A：只补「写表」）。
//!
//! 设计要点：
//!
//! - 所有表以 [`NodeId`](scoop2_base::NodeId) 为键（与 `expr_types` 同），存放在
//!   [`crate::hir::TypedFile`] 中，使其与表达式类型表同生命周期。
//! - 事实是**完整精确**的决议快照：每个 Call/MemberAccess/Operator/Index 节点
//!   都有对应事实。决议失败时，typecheck 必须报诊断（而非静默跳过）。
//!   合法程序的 HIR 中不存在缺失事实的 call/member site。
//! - 携带的类型句柄（`TypeId`）来自 typecheck 的 `TypeStore`，move 进 `TypedHir`
//!   后保持有效（store 是 TypeId 的唯一来源）。

use scoop2_base::{NodeId, Span, Symbol};

use crate::resolve::output::{NodeIdTable, ResolvedValue};
use crate::ty::{EffectRow, TypeId};

/// 一个调用表达式的决议结果。
///
/// 覆盖 `ExprKind::Call` / `InfixCall` / `Binary` / `Unary` / `Index` / 构造器调用
/// 以及方法调用（member-access callee）。key 为**调用表达式自身的 NodeId**
///（`Call`/`InfixCall`/`Binary`/`Unary`/`Index` 节点；方法调用为承载
/// `MemberAccess` 的 `Call` 节点）。
#[derive(Clone, Debug)]
pub enum ResolvedCall {
    /// 顶层函数直接调用（已选定重载）。
    TopLevelFun {
        fqn: Symbol,
        /// 选定重载的声明 span（定位源声明）。
        decl_span: Span,
        /// 选定重载的声明文件。
        decl_file: scoop2_base::FileId,
        /// 显式给出的类型实参（`callee<T, eff E>`）；空表示未显式指定。
        explicit_type_args: Vec<TypeId>,
        /// 调用点推断的类型实参（按 callee type_params 声明顺序）；供 MIR 单态化使用。
        /// 当 explicit_type_args 非空时，与之一致；否则为从实参类型推断的结果。
        inferred_type_args: Vec<TypeId>,
        /// 调用点推断的返回类型。
        return_ty: TypeId,
    },
    /// 方法调用（静态/虚分发）。
    Method {
        /// 接收者静态类型。
        receiver_ty: TypeId,
        /// 声明该方法的 owner 类型 FQN（沿超类型链上溯找到的声明点）。
        owner_fqn: Symbol,
        /// 方法名（simple name）。
        method_name: Symbol,
        /// 声明 span。
        decl_span: Span,
        /// 声明文件。
        decl_file: scoop2_base::FileId,
        /// 是否 `open`/`abstract`/`override`（虚分发候选）。
        is_virtual: bool,
        /// owner 是否为 interface（interface 分发走 itable，class 虚方法走 vtable）。
        is_interface: bool,
        /// 显式类型实参。
        explicit_type_args: Vec<TypeId>,
        /// 调用点推断的类型实参（按方法 type_params 声明顺序）。
        inferred_type_args: Vec<TypeId>,
        /// 返回类型。
        return_ty: TypeId,
    },
    /// 构造器调用（class/struct 主构造或次构造）。
    Constructor {
        /// 被构造类型 FQN。
        type_fqn: Symbol,
        /// 选定构造器声明 span。
        decl_span: Span,
        decl_file: scoop2_base::FileId,
        /// 构造产物类型。
        return_ty: TypeId,
    },
    /// enum variant 构造（`Some(x)` / `Color.Red(x)`）。
    EnumVariant {
        /// enum 类型 FQN。
        enum_fqn: Symbol,
        /// variant 名（simple name）。
        variant_name: Symbol,
        /// 产物类型。
        return_ty: TypeId,
    },
    /// 局部函数值调用（callee 是局部绑定的函数类型值）。
    LocalValue {
        /// 局部绑定名。
        local_name: Symbol,
        /// 函数类型。
        fn_ty: TypeId,
        return_ty: TypeId,
    },
    /// 函数值调用（callee 是计算结果为函数类型的表达式，如 `f()(x)` / `fns[0](1)` / lambda 调用）。
    /// callee 不是命名局部变量，而是任意表达式。MIR 按 FunValue indirect call 处理。
    FunValue {
        /// callee 表达式的函数类型。
        fn_ty: TypeId,
        return_ty: TypeId,
    },
    /// effect 操作调用（`Effect.op(...)` perform 站点）。
    EffectOp {
        /// effect 类型名（simple 或 FQN 文本对应的 Symbol）。
        effect_name: Symbol,
        /// 操作名。
        op_name: Symbol,
        /// 返回类型。
        return_ty: TypeId,
    },
}

/// 一个成员访问（`receiver.member`）的决议结果。
///
/// key 为 `ExprKind::MemberAccess` / `SafeMemberAccess` 节点的 NodeId。
#[derive(Clone, Debug)]
pub enum ResolvedMember {
    /// 字段 / 属性（数据成员）。
    Field {
        receiver_ty: TypeId,
        owner_fqn: Symbol,
        member_name: Symbol,
        member_ty: TypeId,
        /// 是否不可变（`val`）。
        is_immutable: bool,
    },
    /// 方法（无括号引用，作为函数值）。
    Method {
        receiver_ty: TypeId,
        owner_fqn: Symbol,
        method_name: Symbol,
    },
    /// tuple 索引（`t.0`）。
    TupleIndex {
        receiver_ty: TypeId,
        index: u128,
        element_ty: TypeId,
    },
}

/// 一个赋值目标（`StmtKind::Assign` 的 LHS）的 place 分类。
///
/// key 为 [`crate::syntax::ast::AssignTarget`] 的 NodeId（`AssignTarget.id`）。
#[derive(Clone, Debug)]
pub enum ResolvedPlace {
    /// 局部变量写。
    Local { name: Symbol, local_ty: TypeId },
    /// 顶层 `var` 写。
    TopLevelVar { fqn: Symbol, ty: TypeId },
    /// `this`/接收者成员字段写。
    MemberField {
        receiver_ty: TypeId,
        owner_fqn: Symbol,
        member_name: Symbol,
        member_ty: TypeId,
    },
    /// 索引写（`a[i] = v` → `operator set`）。
    Index {
        receiver_ty: TypeId,
        /// `set` 方法 owner FQN。
        owner_fqn: Symbol,
    },
}

/// 一个 effect 站点的元数据。
///
/// key 为承载 effect 操作的表达式 NodeId（perform 调用表达式）。
#[derive(Clone, Debug)]
pub struct EffectSite {
    /// effect 类型名 Symbol。
    pub effect_name: Symbol,
    /// 操作名 Symbol（perform 站点）；调用站点 effect 为 None。
    pub op_name: Option<Symbol>,
    /// effect 行（该站点「执行」的 effect 集合）。
    pub effect_row: EffectRow,
    /// 站点 span。
    pub span: Span,
}

/// 模式绑定（`when` arm / 解构 `val`）引入的一个局部绑定。
///
/// key 为合成键（见 [`PatternBindingKey`]）。
#[derive(Clone, Debug)]
pub struct PatternBinding {
    /// 绑定名。
    pub name: Symbol,
    /// 绑定类型。
    pub ty: TypeId,
    /// 绑定来源。
    pub source: PatternBindingSource,
    /// 绑定在源中的 span（Ident span；合成时为模式 span）。
    pub span: Span,
}

/// 模式绑定来源。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternBindingSource {
    /// `when` arm 模式绑定。
    WhenArm,
    /// `val (a, b) = ...` 解构绑定。
    Destructure,
    /// enum variant payload 字段绑定（`is Color.Red(r)`）。
    VariantField,
}

/// per-file 的语义事实侧表集合。
///
/// 与 [`crate::hir::TypedFile::expr_types`] 同生命周期，由 typecheck 在决议点填充。
#[derive(Debug, Default)]
pub struct SemanticFacts {
    /// 值引用（从 resolve `Resolution::value_refs` 搬入；`Ident`/callee 的值解析）。
    pub value_refs: NodeIdTable<ResolvedValue>,
    /// 调用决议（`Call`/`InfixCall`/`Binary`/`Unary`/`Index` 等节点）。
    pub call_resolutions: NodeIdTable<ResolvedCall>,
    /// 成员访问决议（`MemberAccess`/`SafeMemberAccess` 节点）。
    pub member_refs: NodeIdTable<ResolvedMember>,
    /// 赋值目标 place 分类（`AssignTarget` 节点）。
    pub assign_places: NodeIdTable<ResolvedPlace>,
    /// effect 站点元数据。
    pub effect_sites: NodeIdTable<EffectSite>,
    /// 模式绑定（key = 父模式 `Pattern` 节点 NodeId；value = 该模式引入的全部绑定）。
    /// 每个绑定按其在模式中出现的位置（序号）排列，MIR lowering 据此生成 binder local。
    pub pattern_bindings: NodeIdTable<Vec<PatternBinding>>,
    /// 每个表达式的 actual effect row（执行该表达式引入的 effect 集合）。
    /// Pure = 空行。Handle 表达式的 row 已减去被 arm 截获的 effect。
    pub expr_effect_rows: NodeIdTable<crate::ty::EffectRow>,
    /// 构造器调用点（Call 节点）选中的 ctor 声明 span。
    /// primary ctor 的 span = 类名 span；secondary ctor 的 span = `constructor` 关键字 span。
    /// 区分 primary/secondary，供 MIR/codegen 选择正确的 ctor callable。
    /// 缺省（无 secondary ctor 时）不写入——调用点按 primary 处理。
    pub ctor_selections: NodeIdTable<scoop2_base::Span>,
    /// 调用点（Call 节点）的解析后实参列表。
    ///
    /// HIR 层完整解析每个 call site 后，把实参按参数位置排序 + 填充默认值，
    /// 写入此表。MIR lower 直接消费（不再做任何 resolution / default filling）。
    /// 每个元素：Provided(original_idx) = 用调用点第 original_idx 个实参；
    /// Default(expr_node_id) = 用默认值表达式（需 MIR lower 该表达式）。
    /// 不写入 = 无默认值填充（全部位置实参，MIR 按原样使用）。
    pub resolved_call_args: NodeIdTable<Vec<ResolvedCallArg>>,
}

/// 解析后的调用实参（按 callee 参数位置排序 + 默认值填充）。
#[derive(Debug, Clone)]
pub enum ResolvedCallArg {
    /// 使用调用点提供的第 `original_index` 个实参。
    Provided { original_index: usize },
    /// 使用默认值表达式（已克隆的 Expr，供 MIR 直接 lower）。
    Default { expr: crate::syntax::ast::Expr },
}

impl SemanticFacts {
    pub fn new() -> Self {
        Self::default()
    }
}

/// 模式绑定表专用键（pattern sub-binding 无独立 NodeId）。
///
/// 由「父节点 NodeId + 绑定在该父节点中的序号」复合而成，保证稳定且无冲突。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PatternBindingKey {
    pub parent: NodeId,
    pub index: u32,
}
