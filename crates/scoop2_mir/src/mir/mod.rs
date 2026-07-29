//! MIR 数据结构定义。
//!
//! 顶层容器是 [`Module`]，含若干 [`Item`]。每个可调用 [`Item::Fun`] 拥有一个
//! [`Body`]：若干 [`LocalDecl`]（参数 + 局部 + 临时，ANF 风格）+ 若干
//! [`BasicBlock`]（每块一串 [`Statement`] + 一个 [`Terminator`]）。
//!
//! - 表达式 lowering 产物是「向一个临时 local 赋一个 [`Rvalue`]」；
//! - 控制流（if/when/while/for/break/continue/try/handle/perform）lowering 为
//!   基本块图，终结符见 [`TerminatorKind`]；
//! - effect 用 direct-style：`Perform` / `Handle` 是终结符。
//!
//! 所有类型句柄复用 `scoop2_hir::ty::{TypeId, TypeStore, EffectRow}`。

pub mod devirtualize;
pub mod dump;
pub mod effect_lower;
pub mod inline;
pub mod lower;
pub mod materialize;
pub mod stable_id;
pub mod transport;
pub mod verify;

pub use transport::*;

use scoop2_base::{FileId, NodeId, Span, Symbol};
use scoop2_hir::ty::{EffectRow, TypeId};

// ---------------------------------------------------------------------------
// 标识符（newtype，下标语义）
// ---------------------------------------------------------------------------

/// 基本块 id（下标进 [`Body::blocks`]）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BasicBlockId(pub u32);

impl std::fmt::Display for BasicBlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "bb{}", self.0)
    }
}

/// local id（下标进 [`Body::locals`]）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LocalId(pub u32);

impl std::fmt::Display for LocalId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "_{}", self.0)
    }
}

/// 源程序调用点稳定身份（per-call-site；devirtualization / 诊断用）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SiteId(pub u32);

// ---------------------------------------------------------------------------
// Module / Item / FunDecl
// ---------------------------------------------------------------------------

/// 一个编译单元的 MIR 模块（generic 模板阶段；单态化产物也是 Module）。
#[derive(Clone, Debug)]
pub struct Module {
    pub items: Vec<Item>,
    /// 类型存储（与 HIR 共享句柄；lowering 时从 HIR move 或克隆）。
    pub types: scoop2_hir::ty::TypeStore,
}

impl Module {
    /// 取类型存储的不可变引用（供 verify 使用）。
    pub fn types_ref(&self) -> &scoop2_hir::ty::TypeStore {
        &self.types
    }
}

/// 顶层 MIR item。
#[derive(Clone, Debug)]
pub enum Item {
    /// 可调用函数（顶层 / 成员 / 闭包 / 次构造器）。
    Fun(FunDecl),
    /// 顶层 val/var 初始化根（程序启动时求值）。
    Initializer(InitializerRoot),
    /// extern 全局符号（`@Extern` 顶层 var）。
    ExternGlobal(ExternGlobal),
    /// 元数据根（类型声明信息，供后端 layout / vtable 生成参考）。
    Metadata(MetadataRoot),
}

/// 函数声明 + 可选 body。
#[derive(Clone, Debug)]
pub struct FunDecl {
    pub span: Span,
    /// 全限定名（点分；闭包用合成名 `<owner>$closure<N>`）。
    pub fqn: String,
    pub name: String,
    /// 函数类型（`(P0,..) -> R / Row`）。
    pub ty: TypeId,
    pub params: Vec<Param>,
    pub return_ty: TypeId,
    /// effect 行。
    pub effect_row: EffectRow,
    /// 类型参数名序列（按声明顺序；>0 表示泛型模板）。
    pub type_params: Vec<scoop2_base::Symbol>,
    /// `None` = 声明体（abstract / interface 成员 / @Extern / @Intrinsic）。
    pub body: Option<Body>,
    /// 源文件（跨文件诊断用）。
    pub file: FileId,
    /// 该函数的 stable template key（供分离编译使用）。
    /// 由 `compute_public_stable_keys` pass 填充；含 FQN + type params + overload sig。
    /// None = 尚未计算（lowering 产出时为 None）。
    pub stable_template_key: Option<crate::mir::transport::StableTemplateKey>,
    /// 单态化实例的唯一符号名（含具体类型/eff 实参哈希）。
    ///
    /// - 非单态化产物（源模板 / 非泛型顶层函数）：None → codegen 用
    ///   `mangle_symbol(fqn, stable_template_key)` 计算。
    /// - 单态化产物（generic 实例化结果）：Some(unique_sym)，由 materialize
    ///   按 `InstanceKey` 计算并写入，确保同 FQN 不同实参的实例（如
    ///   `println<Int>` / `println<String>`）符号不冲突。
    pub instance_symbol: Option<String>,
    /// Effect step ABI 信息。None = Plain 函数（普通 ABI）。
    /// Some = EffectStep 函数（经过 effect lowering，含未捕获 Perform，
    /// 函数体已变换为状态机，返回 Step tagged union）。
    pub effect_abi: Option<EffectStepAbi>,
}

/// EffectStep 函数的 ABI 信息。
#[derive(Clone, Debug)]
pub struct EffectStepAbi {
    /// Frame tuple 类型的 TypeId。
    pub frame_ty: TypeId,
    /// Step enum 合成类型的 TypeId。
    pub step_ty: TypeId,
    /// Step enum 的所有变体信息（Complete + 各 effect 操作变体）。
    pub step_variants: Vec<StepVariant>,
    /// frame local 的 LocalId（在函数体内）。
    pub frame_local: LocalId,
    /// state local 的 LocalId（用于 state dispatch）。
    pub state_local: LocalId,
}

/// Step enum 的一个变体。
#[derive(Clone, Debug)]
pub struct StepVariant {
    /// 变体名称（如 "Complete"、"scoop_core_Raise_raise"）。
    pub name: String,
    /// 变体的 FQN Symbol。
    pub name_sym: scoop2_base::Symbol,
    /// 变体的 payload 类型（Complete = 原始返回类型；effect 变体 = payload tuple 类型）。
    pub payload_ty: TypeId,
    /// 是否为 Complete 变体（正常完成）。
    pub is_complete: bool,
}

/// 函数参数（同时也是一个 local）。
#[derive(Clone, Debug)]
pub struct Param {
    pub span: Span,
    pub name: String,
    pub ty: TypeId,
    pub local: LocalId,
}

/// 顶层 val/var 初始化器根。
#[derive(Clone, Debug)]
pub struct InitializerRoot {
    pub span: Span,
    pub fqn: String,
    pub ty: TypeId,
    pub is_var: bool,
    pub body: Body,
    pub file: FileId,
}

/// extern 全局符号。
#[derive(Clone, Debug)]
pub struct ExternGlobal {
    pub span: Span,
    pub fqn: String,
    pub ty: TypeId,
    pub file: FileId,
}

/// 类型声明元数据根（class/interface/struct/enum/object/extension）。
#[derive(Clone, Debug)]
pub struct MetadataRoot {
    pub span: Span,
    pub fqn: String,
    pub kind: MetadataKind,
    pub file: FileId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataKind {
    Class,
    Interface,
    Struct,
    Enum,
    Object,
    Effect,
    TypeAlias,
    Extension,
}

// ---------------------------------------------------------------------------
// Body / BasicBlock / LocalDecl
// ---------------------------------------------------------------------------

/// 函数体：locals + 基本块图 + 入口块。
#[derive(Clone, Debug)]
pub struct Body {
    pub locals: Vec<LocalDecl>,
    pub blocks: Vec<BasicBlock>,
    pub start: BasicBlockId,
}

impl Body {
    pub fn new() -> Self {
        Self {
            locals: Vec::new(),
            blocks: Vec::new(),
            start: BasicBlockId(0),
        }
    }

    /// 追加一个 local，返回其 id。
    pub fn push_local(&mut self, decl: LocalDecl) -> LocalId {
        let id = LocalId(self.locals.len() as u32);
        self.locals.push(decl);
        id
    }

    /// 追加一个基本块，返回其 id。
    pub fn push_block(&mut self, block: BasicBlock) -> BasicBlockId {
        let id = BasicBlockId(self.blocks.len() as u32);
        self.blocks.push(block);
        id
    }

    /// 入口块的可变引用。
    pub fn entry_mut(&mut self) -> Option<&mut BasicBlock> {
        self.blocks.get_mut(self.start.0 as usize)
    }

    /// 从 `start` BFS 可达的块 id 集合。
    pub fn reachable(&self) -> std::collections::BTreeSet<BasicBlockId> {
        use std::collections::{BTreeSet, VecDeque};
        let mut seen: BTreeSet<BasicBlockId> = BTreeSet::new();
        let mut queue: VecDeque<BasicBlockId> = VecDeque::new();
        queue.push_back(self.start);
        seen.insert(self.start);
        while let Some(b) = queue.pop_front() {
            let Some(block) = self.blocks.get(b.0 as usize) else {
                continue;
            };
            for succ in block.successors() {
                if seen.insert(succ) {
                    queue.push_back(succ);
                }
            }
        }
        seen
    }

    /// 分配一个新的 SiteId（per-call-site 稳定身份计数器）。
    pub fn next_site_id(&mut self) -> u32 {
        // SiteId 从 0 开始递增；与 locals/blocks 独立的计数器。
        // 用 blocks.len() + locals.len() + 1 作起点避免与它们冲突。
        (self.blocks.len() as u32) + (self.locals.len() as u32) + 1
    }
}

impl Default for Body {
    fn default() -> Self {
        Self::new()
    }
}

/// local 声明。
#[derive(Clone, Debug)]
pub struct LocalDecl {
    pub span: Span,
    /// `Some(name)` = 源程序命名 local（参数 / val/var）；`None` = 编译器临时。
    pub name: Option<String>,
    pub ty: TypeId,
    pub source: LocalSource,
    /// 是否可变（`var` 声明）；`val` / 参数 / 临时为 `false`。
    pub mutable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalSource {
    /// 源程序声明的 local（参数 / val / var / pattern binder）。
    Source,
    /// 编译器生成的临时（表达式 lowering 中间值）。
    Temp,
}

/// 基本块。
#[derive(Clone, Debug)]
pub struct BasicBlock {
    pub stmts: Vec<Statement>,
    pub terminator: Terminator,
}

impl BasicBlock {
    pub fn new(terminator: Terminator) -> Self {
        Self {
            stmts: Vec::new(),
            terminator,
        }
    }

    /// 该块的所有后继（CFG 边）。
    pub fn successors(&self) -> SmallSuccessors {
        self.terminator.successors()
    }
}

/// 后继迭代器（最多 4 个，栈分配）。
pub struct SmallSuccessors {
    items: [Option<BasicBlockId>; 4],
    len: usize,
    pos: usize,
}

impl SmallSuccessors {
    fn new() -> Self {
        Self {
            items: [None, None, None, None],
            len: 0,
            pos: 0,
        }
    }
    fn push(&mut self, b: BasicBlockId) {
        if self.len < 4 {
            self.items[self.len] = Some(b);
            self.len += 1;
        }
    }
}

impl Iterator for SmallSuccessors {
    type Item = BasicBlockId;
    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.len {
            let v = self.items[self.pos];
            self.pos += 1;
            v
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Statement
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Statement {
    pub span: Span,
    pub kind: StatementKind,
}

#[derive(Clone, Debug)]
pub enum StatementKind {
    /// 空操作（占位 / trailing）。
    Nop,
    /// `target = rvalue`（核心赋值）。
    Assign {
        target: LocalId,
        value: Rvalue,
    },
    /// 成员字段写：`receiver.member = value`。
    StoreMember {
        receiver: Operand,
        member: MemberAccessMetadata,
        value: Operand,
        value_ty: TypeId,
        continuation_route: StoredContinuationRoutePublication,
    },
    /// tuple 元素写：`receiver.<index> = value`。
    StoreTupleIndex {
        receiver: Operand,
        index: u128,
        value: Operand,
        value_ty: TypeId,
    },
    /// 顶层 `var` 写：`fqn = value`。
    StoreTopLevelVar {
        fqn: Symbol,
        value: Operand,
        value_ty: TypeId,
    },
    /// 运行期断言失败（`!!` / 不可达分支）：发出 panic（无值）。
    Panic {
        message: String,
    },
}

// ---------------------------------------------------------------------------
// Operand / ConstValue
// ---------------------------------------------------------------------------

/// 一个值操作数：local 引用或编译期常量。
#[derive(Clone, Debug)]
pub enum Operand {
    Local(LocalId),
    Const(ConstValue),
}

/// 编译期常量（保留载荷；不靠 span 重建）。
#[derive(Clone, Debug)]
pub enum ConstValue {
    Bool(bool),
    Char(char),
    Unit,
    Int(u128, Option<IntSuffix>),
    Float(f64, Option<FloatSuffix>),
    String(String),
    Null, // `null` 字面量（可空类型的 null 值）。
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntSuffix {
    U,
    L,
    UL,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloatSuffix {
    F32,
}

// ---------------------------------------------------------------------------
// Rvalue（值生产者；覆盖全部 ExprKind）
// ---------------------------------------------------------------------------

/// 一切「产出一个值」的 MIR 节点。每个表达式 lowering 为
/// `StatementKind::Assign { target: tmp, value: Rvalue }`。
#[derive(Clone, Debug)]
pub enum Rvalue {
    /// 直接使用一个操作数（identity / 变量读取）。
    Use(Operand),
    /// 顶层值引用（`value_ref` 命中 TopLevelValue）。
    TopLevelRef(TopLevelRef),
    /// 未解析的名字（resolve/typecheck 失败时的兜底；verify 会拒绝）。
    UnresolvedName {
        name: String,
    },
    /// `expr is T` / `expr !is T` 运行期类型测试。
    TypeTest {
        site_id: Option<SiteId>,
        value: Operand,
        metadata: RuntimeTypeTestMetadata,
    },
    /// `expr as T` / `expr as? T` 类型转换。
    Cast {
        site_id: Option<SiteId>,
        value: Operand,
        op: CastOp,
        metadata: RuntimeCastMetadata,
    },
    /// 成员访问（字段 / 属性读取）。
    MemberAccess {
        site_id: Option<SiteId>,
        receiver: Operand,
        member: MemberAccessMetadata,
    },
    /// tuple 索引读取 `receiver.<index>`。
    TupleIndex {
        receiver: Operand,
        index: u128,
        element_ty: TypeId,
    },
    /// 索引读取 `receiver[args]` → `operator get`。
    IndexAccess {
        receiver: Operand,
        indices: Vec<Operand>,
        element_ty: TypeId,
    },
    /// enum variant 构造 `Variant(args...)`。
    EnumVariant {
        enum_ty: TypeId,
        enum_fqn: Symbol,
        variant_name: Symbol,
        args: Vec<CallArg>,
        payload: AggregateTransportMetadata,
        /// variant 的 stable template key（含 enum FQN + variant 名 + payload 类型）。
        /// 供分离编译使用。None = 尚未计算。
        stable_key: Option<crate::mir::transport::StableTemplateKey>,
    },
    /// class/struct 构造器调用 `Type(args...)`。
    ClassCtor {
        site_id: Option<SiteId>,
        type_fqn: Symbol,
        ctor: ClassCtorCallMetadata,
        args: Vec<CallArg>,
        hidden_effects: EffectRow,
    },
    /// 统一调用（direct / virtual / closure / fun-value / perform-resume）。
    Call {
        site_id: Option<SiteId>,
        kind: CallKind,
        args: Vec<CallArg>,
        transport: CallTransportMetadata,
    },
    /// 元组字面量 `(a, b, ..)`。
    MakeTuple {
        elements: Vec<Operand>,
        transport: AggregateTransportMetadata,
    },
    /// 数组字面量 `[a, b, ..]`（lowering 为 `Array.of(...)` 调用）。
    MakeArray {
        elements: Vec<Operand>,
        result_ty: TypeId,
    },
    /// struct 字面量 `T { f1: v1, .. }`。
    StructLit {
        type_fqn: Symbol,
        fields: Vec<StructLitField>,
        transport: AggregateTransportMetadata,
    },
    /// f-string 拼接（lowering 为 `String.concat(...)` 调用链的结果）。
    InterpolatedString {
        parts: Vec<InterpolatedPart>,
    },
    /// `expr with { path: v, .. }`（不可变更新；构造副本）。
    WithUpdate {
        base: Operand,
        updates: Vec<WithUpdateField>,
        result_ty: TypeId,
    },
    /// 闭包构造：env tuple + 指向嵌套函数的 invoke 指针。
    MakeClosure {
        env: Operand,
        invoke_fqn: String,
        env_contract: ClosureEnvTransportMetadata,
    },
    /// `T::class`（类型元数据字面量）。
    ClassLit {
        type_fqn: Symbol,
    },
    /// effect 操作的结果占位（perform 终结符在 resume_target 续点产出此值）。
    /// 该 Rvalue 由 `Perform` 终结符写入对应的 resume local；本身不执行 effect。
    PerformResult {
        op_fqn: String,
        result_ty: TypeId,
    },
    /// 模式匹配测试：`subject` 是否匹配 `pattern`，产出 Bool。
    /// 用于 `when` arm 的模式测试（variant tag / literal / type test）。
    PatternMatch {
        subject: Operand,
        pattern: Pattern,
    },
    /// 模式提取：从已匹配的 `subject` 中按 `path` 提取 binder 值。
    /// path 是投影序列（tuple index / variant field index）。
    PatternExtract {
        subject: Operand,
        path: Vec<PatternBindingStep>,
        result_ty: TypeId,
    },
    /// 整数相等比较：`lhs == rhs`，产出 Bool。
    /// 用于 effect lowering 的 state dispatch（检查 frame.state 值）。
    IntEq {
        lhs: Operand,
        rhs: Operand,
    },
}

/// 调用实参（可命名 / 可展开 `*expr`）。
#[derive(Clone, Debug)]
pub struct CallArg {
    /// 命名实参名（`name = ...`）；None = 位置实参。
    pub name: Option<Symbol>,
    /// `*expr` spread。
    pub is_spread: bool,
    pub value: Operand,
    pub value_ty: TypeId,
}

/// struct 字面量字段。
#[derive(Clone, Debug)]
pub struct StructLitField {
    pub name: Symbol,
    pub value: Operand,
    pub value_ty: TypeId,
}

/// f-string 拼接片段。
#[derive(Clone, Debug)]
pub enum InterpolatedPart {
    Lit(String),
    Expr(Operand),
}

/// `with` 更新字段（路径 segments + 值）。
#[derive(Clone, Debug)]
pub struct WithUpdateField {
    pub path: Vec<WithUpdateSegment>,
    pub value: Operand,
    pub value_ty: TypeId,
}

#[derive(Clone, Debug)]
pub enum WithUpdateSegment {
    Named(Symbol),
    TupleIndex(u128),
}

// ---------------------------------------------------------------------------
// CallKind（分发类别）
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum CallKind {
    /// 静态已知 callee（顶层函数 / final 方法 / 构造器已选重载）。
    Direct {
        callee_fqn: String,
        type_args: Vec<TypeId>,
        is_intrinsic: bool,
        stable_template_key: Option<StableTemplateKey>,
        stable_instance_key: Option<StableInstanceKey>,
        generic_type_args: Vec<TypeId>,
        generic_eff_args: Vec<EffectRow>,
    },
    /// class 虚方法分发（open/override 方法，走 vtable）。
    Virtual {
        receiver: Operand,
        dispatch: DispatchMetadata,
    },
    /// interface 分发（走 itable，与 class vtable 分开）。
    Interface {
        receiver: Operand,
        dispatch: DispatchMetadata,
    },
    /// 闭包调用（callable 值 + 已知 invoke 目标）。
    Closure {
        callee: Operand,
        invoke_fqn: String,
    },
    /// 函数值调用（未退化为 direct/closure 的函数类型值）。
    FunValue {
        callee: Operand,
    },
    /// continuation resume 调用（`k.resume(value)`）。
    /// continuation 是 Continuation<Resume, Answer, eff E> 接口的实例。
    Resume {
        continuation: Operand,
        resume_value: Operand,
    },
}

// ---------------------------------------------------------------------------
// Terminator
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Terminator {
    pub span: Span,
    pub kind: TerminatorKind,
}

impl Terminator {
    pub fn successors(&self) -> SmallSuccessors {
        let mut s = SmallSuccessors::new();
        match &self.kind {
            TerminatorKind::Return { .. } | TerminatorKind::Unreachable => {}
            TerminatorKind::Goto { target } => s.push(*target),
            TerminatorKind::CondBr {
                then_target,
                else_target,
                ..
            } => {
                s.push(*then_target);
                s.push(*else_target);
            }
            TerminatorKind::Perform { resume_target, .. } => s.push(*resume_target),
            TerminatorKind::Handle {
                body_target,
                arm_targets,
                finally_target,
                exit_target,
                ..
            } => {
                s.push(*body_target);
                for a in arm_targets {
                    s.push(*a);
                }
                if let Some(f) = finally_target {
                    s.push(*f);
                }
                s.push(*exit_target);
            }
        }
        s
    }
}

#[derive(Clone, Debug)]
pub enum TerminatorKind {
    /// 函数返回（`None` = Unit/隐式 return）。
    Return {
        value: Option<Operand>,
    },
    /// 无条件跳转。
    Goto {
        target: BasicBlockId,
    },
    /// 条件分支。
    CondBr {
        cond: Operand,
        then_target: BasicBlockId,
        else_target: BasicBlockId,
    },
    /// 不可达（穷尽 when / 不可达 else 等）。
    Unreachable,
    /// effect 操作调用（direct-style：携带 resume_target）。
    Perform {
        site_id: Option<SiteId>,
        op_fqn: String,
        metadata: PerformMetadata,
        args: Vec<CallArg>,
        resume_local: LocalId,
        resume_target: BasicBlockId,
    },
    /// effect handler 区（结构化终结符；携带 body/arm/finally/exit 目标）。
    Handle {
        site_id: Option<SiteId>,
        metadata: HandleMetadata,
        arms: Vec<HandlerArm>,
        body_target: BasicBlockId,
        arm_targets: Vec<BasicBlockId>,
        finally_target: Option<BasicBlockId>,
        exit_target: BasicBlockId,
    },
}

// ---------------------------------------------------------------------------
// Pattern（when arm 模式）
// ---------------------------------------------------------------------------

/// `when` arm 模式（lowering 用）。
#[derive(Clone, Debug)]
pub enum Pattern {
    /// `_` / 命名 binder（无约束）。
    Wildcard,
    /// 绑定 `name: ty`。
    Bind {
        name: Symbol,
        ty: TypeId,
    },
    /// 字面量匹配。
    IntLit(i128),
    CharLit(char),
    StringLit(String),
    BoolLit(bool),
    /// `is T` 类型测试模式。
    Is {
        ty: TypeId,
        negated: bool,
    },
    /// tuple 模式 `(p0, p1, ..)`。
    Tuple {
        elements: Vec<Pattern>,
    },
    /// struct 模式 `T { f0: p0, .. }`。
    Struct {
        type_fqn: Symbol,
        fields: Vec<StructPatternField>,
    },
    /// enum variant 模式 `Variant(p0, ..)`。
    Variant {
        enum_fqn: Symbol,
        variant_name: Symbol,
        args: Vec<Pattern>,
    },
    /// `p0 | p1` 或模式。
    Or {
        patterns: Vec<Pattern>,
    },
}

#[derive(Clone, Debug)]
pub struct StructPatternField {
    pub name: Symbol,
    pub pattern: Pattern,
}

// ---------------------------------------------------------------------------
// Metadata（源 AST 节点身份回指，诊断 / 调试用）
// ---------------------------------------------------------------------------

/// 附着在 lowering 产物上的源 AST 身份（可选）。
#[derive(Clone, Copy, Debug)]
pub struct Metadata {
    pub source_node: Option<NodeId>,
    pub source_span: Span,
    pub source_file: FileId,
}

// 复用 AST 的 op 枚举（不重复定义）。
pub use scoop2_syntax::ast::CastOp;
