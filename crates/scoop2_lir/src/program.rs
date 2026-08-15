//! LIR 数据结构：LirProgram 容器 + 所有产出类型。
//!
//! 这些类型是 LIR pass 的产出，也是 codegen 的输入。
//! codegen 遍历这些结构时不需要做任何推断——所有布局/ABI/分发决策已在 LIR 完成。

use std::collections::HashMap;

use scoop2_mir::ty::TypeId;

// =========================================================================
// 操作数（LirOperand）
// =========================================================================

/// LIR 操作数：local 引用或编译期常量。
/// 替代 MIR 的 Operand，在所有需要值操作数的地方使用。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum LirOperand {
    /// 局部变量引用。
    Local(u32),
    /// 编译期常量。
    Const(LirConstValue),
}

/// LIR 编译期常量（从 MIR ConstValue 1:1 映射）。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum LirConstValue {
    Bool(bool),
    Char(char),
    Unit,
    Int(u128, Option<LirIntSuffix>),
    Float(f64, Option<LirFloatSuffix>),
    String(String),
    Null,
}

/// 整数后缀。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum LirIntSuffix {
    U,
    L,
    UL,
}

/// 浮点数后缀。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum LirFloatSuffix {
    F32,
}

// =========================================================================
// 顶层容器
// =========================================================================

/// LIR 顶层产物：自包含的编译单元，codegen 无需回查 HIR/MIR。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LirProgram {
    /// 函数体列表（含 Plain 和 EffectStep）。
    pub callables: Vec<LirCallable>,
    /// 无函数体的声明（extern / abstract / intrinsic）。
    pub declarations: Vec<LirDeclaration>,
    /// 类型布局表：TypeId → TypeLayout。
    pub type_layouts: TypeLayoutTable,
    /// class vtable 布局列表。
    pub vtables: Vec<VtableLayout>,
    /// interface itable 定义列表。
    pub itables: Vec<ItableLayout>,
    /// class × interface 的 itable 实现映射列表。
    pub class_itables: Vec<ClassItableLayout>,
    /// GC 类型描述符列表。
    pub type_descriptors: Vec<TypeDescriptor>,
    /// 顶层 val/var 全局初始化计划。
    pub global_init: GlobalInitPlan,
    /// 类初始化计划列表。
    pub class_inits: Vec<ClassInitPlan>,
    /// 合成类型声明（Step enum / Continuation struct / Frame tuple）。
    pub synthetic_types: Vec<SyntheticTypeDecl>,
    /// 闭包对象布局列表（每个 invoke_fqn 一个）。
    pub closure_layouts: Vec<ClosureLayout>,
}

/// 闭包对象布局。
/// 闭包对象 = { invoke_fn_ptr: ptr, env_ptr: ptr }
/// env 布局由 captures 列表决定。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ClosureLayout {
    /// invoke 函数的 FQN。
    pub invoke_fqn: String,
    /// env 中每个捕获变量的字段布局。
    pub captures: Vec<ClosureCaptureLayout>,
    /// env 的总大小（字节）。
    pub env_size: u64,
    /// env 的对齐。
    pub env_align: u64,
}

/// 闭包捕获变量布局。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ClosureCaptureLayout {
    /// 捕获变量名。
    pub name: String,
    /// 在 env 中的偏移。
    pub offset: u64,
    /// 捕获变量类型。
    pub ty: TypeId,
    /// 是否 GC-managed。
    pub gc_traceable: bool,
}

impl LirProgram {
    pub fn new() -> Self {
        Self {
            callables: Vec::new(),
            declarations: Vec::new(),
            type_layouts: TypeLayoutTable::new(),
            vtables: Vec::new(),
            itables: Vec::new(),
            class_itables: Vec::new(),
            type_descriptors: Vec::new(),
            global_init: GlobalInitPlan {
                entries: Vec::new(),
            },
            class_inits: Vec::new(),
            synthetic_types: Vec::new(),
            closure_layouts: Vec::new(),
        }
    }
}

impl Default for LirProgram {
    fn default() -> Self {
        Self::new()
    }
}

/// 合成类型声明（由 effect lowering 产生的合成类型的布局信息）。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct SyntheticTypeDecl {
    /// 合成类型的 FQN（如 "pkg.f$step"、"pkg.f$continuation"）。
    pub fqn: String,
    /// 合成类型种类。
    pub kind: SyntheticTypeKind,
    /// 布局信息。
    pub layout: TypeLayout,
}

/// 合成类型种类。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum SyntheticTypeKind {
    /// Step tagged union（EffectStep 函数的返回类型）。
    StepEnum,
    /// Continuation 对象（resuming arm 的 continuation binder 类型）。
    ContinuationStruct,
    /// Frame tuple（EffectStep 函数的状态保存结构）。
    FrameTuple,
}

// =========================================================================
// 类型布局
// =========================================================================

/// 类型布局表：TypeId → TypeLayout 映射。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TypeLayoutTable {
    pub entries: HashMap<TypeId, TypeLayout>,
}

impl TypeLayoutTable {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn get(&self, ty: TypeId) -> Option<&TypeLayout> {
        self.entries.get(&ty)
    }

    pub fn insert(&mut self, ty: TypeId, layout: TypeLayout) {
        self.entries.insert(ty, layout);
    }
}

impl Default for TypeLayoutTable {
    fn default() -> Self {
        Self::new()
    }
}

/// 一个类型的完整布局信息。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TypeLayout {
    /// 类型大小（字节）。
    pub size: u64,
    /// 类型对齐（字节）。
    pub align: u64,
    /// 布局种类（决定 codegen 如何翻译此类型）。
    pub kind: TypeLayoutKind,
}

/// 布局种类。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum TypeLayoutKind {
    /// 标量值类型（Int/Bool/Char/Float/Unit）。
    Scalar {
        /// 标量种类。
        scalar_kind: ScalarKind,
    },
    /// Struct 值类型。
    Struct {
        /// 字段列表（按声明顺序，含偏移）。
        fields: Vec<FieldLayout>,
    },
    /// Tuple 值类型。
    Tuple {
        /// 元素列表（按顺序，含偏移）。
        elements: Vec<FieldLayout>,
    },
    /// Option<T> 值类型。
    Option {
        /// niche 存储方式。
        storage: NicheStorage,
        /// payload 类型布局。
        payload_size: u64,
        /// payload 的 HIR TypeId（codegen 需要它降级 payload 字段的 LLVM 类型）。
        payload_ty: TypeId,
    },
    /// Enum 值类型（tagged union）。
    Enum {
        /// tag 大小（字节）。
        tag_size: u64,
        /// tag 在对象中的偏移。
        tag_offset: u64,
        /// payload 区域在对象中的字节偏移（`align_to(tag_size, max_payload_align)`）。
        payload_offset: u64,
        /// 变体列表。
        variants: Vec<EnumVariantLayout>,
    },
    /// 引用类型（GC-managed 或非 GC）。
    Reference {
        /// 是否 GC-managed。
        gc_traceable: bool,
        /// 引用种类。
        ref_kind: RefKind,
    },
    /// 函数值类型（引用，GC-managed）。
    Function,
    /// Nothing（bottom type，大小 0）。
    Nothing,
}

/// 标量种类。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ScalarKind {
    Unit,
    Bool,
    Char,
    Int { bits: u16, unsigned: bool },
    Float { bits: u16 },
}

/// 引用种类。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum RefKind {
    /// class 引用（GC-managed）。
    Class,
    /// interface 引用（GC-managed，itable 分发）。
    Interface,
    /// String 引用（GC-managed）。
    String,
    /// Any 引用（GC-managed，所有引用的根）。
    Any,
    /// Object 单例引用（GC-managed）。
    Object,
}

/// Niche 存储方式（Option<T> 的空值编码）。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum NicheStorage {
    /// 指针 null niche（Option<引用> 用 null 表示 None）。
    Pointer,
    /// u8 sentinel（Option<SmallInt> 用特殊值表示 None）。
    U8 { none_value: u8 },
    /// 无 niche 可用，需要额外 tag 字段。
    Tagged,
}

/// 字段布局（struct/tuple 的字段）。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct FieldLayout {
    /// 字节偏移。
    pub offset: u64,
    /// 字段大小（字节）。
    pub size: u64,
    /// 字段的 HIR TypeId。
    pub ty: TypeId,
}

/// Enum 变体布局。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct EnumVariantLayout {
    /// 变体名。
    pub name: String,
    /// tag 值（判别值）。
    pub tag_value: u64,
    /// 本 variant payload 区域在 enum 值内的绝对字节偏移。
    /// 不含 ref 的 variant 共用 scalar union slot（= Enum.payload_offset）；
    /// 含 ref 的 variant 各占独立 slot（GC trace 位按偏移静态枚举，
    /// 不同 variant 的 ref 叶子偏移不能落在同一字节上歧义）。
    pub slot_offset: u64,
    /// payload 类型（None = 无 payload 或多字段 variant——后者无单一 TypeId 可表达，
    /// 字段布局见 `payload_fields`）。
    pub payload_ty: Option<TypeId>,
    /// 多字段 variant 的 payload 字段布局（偏移相对本 variant 的 slot 起点；
    /// 单字段 variant 为空——读写走 `payload_ty` 主线）。
    pub payload_fields: Vec<FieldLayout>,
}

// =========================================================================
// 函数体（LirCallable）
// =========================================================================

/// 一个函数的 LIR 表示。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LirCallable {
    /// 全限定名。
    pub fqn: String,
    /// Mangled 符号名（codegen 使用）。
    pub symbol_name: String,
    /// 函数 ABI。
    pub abi: LirCallableAbi,
    /// 参数列表。
    pub params: Vec<LirParam>,
    /// 返回值类型。
    pub return_ty: TypeId,
    /// 返回值 ABI。
    pub return_abi: ParamAbi,
    /// 函数体（None = 声明体）。
    pub body: Option<LirBody>,
    /// GC 信息（safepoint + root map）。
    pub gc_info: Option<GcInfo>,
    /// EffectStep 专用：frame schema。
    pub frame_schema: Option<FrameSchema>,
    /// EffectStep 专用：step layout。
    pub step_layout: Option<StepLayout>,
    /// EffectStep 专用：state dispatch（resume-state → block-id 映射）。
    pub state_dispatch: Option<StateDispatch>,
    /// EffectStep 专用：continuation layout。
    pub continuation_layout: Option<ContinuationLayout>,
    /// EffectStep 专用：codegen 元数据（frame local、参数槽、resume 续点）。
    pub effect_info: Option<LirEffectInfo>,
}

/// EffectStep 函数的 codegen 元数据。
///
/// EffectStep 函数 codegen 为两个 LLVM 函数：`sym`（原始签名 wrapper：
/// 堆分配 frame + 清零 + 写参数槽 + 调 `sym$step(frame, 0)` 并返回其 Step）
/// 和 `sym$step(ptr frame, i64 word) -> Step`（LIR body 的编译目标）。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LirEffectInfo {
    /// body 内持有 frame 堆指针的 local id。
    pub frame_local: u32,
    /// frame tuple 类型（codegen 用其布局计算参数槽/续点槽字节偏移）。
    pub frame_ty: TypeId,
    /// 参数槽表：`(参数 local id, frame slot 下标)`（slot 0 = state，参数从 1 起）。
    pub param_slots: Vec<(u32, u64)>,
    /// resume 续点表（仅 escape 捕获站点）。
    pub resume_points: Vec<LirResumePoint>,
    /// Step enum 类型（`sym$step` 的返回类型）。
    pub step_ty: TypeId,
}

/// resume 续点：state 值 → 目标块 + resume 值投递 local。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LirResumePoint {
    /// frame.state 中的 state 值。
    pub state: u64,
    /// 目标基本块 id（LIR body 内的块 id）。
    pub block_id: u32,
    /// resume 值投递目标 local。
    pub resume_local: u32,
    /// resume 值类型（word → 该类型的转换在块首完成）。
    pub resume_ty: TypeId,
}

/// 无函数体的声明。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LirDeclaration {
    /// 全限定名。
    pub fqn: String,
    /// Mangled 符号名。
    pub symbol_name: String,
    /// 参数列表。
    pub params: Vec<LirParam>,
    /// 返回值类型。
    pub return_ty: TypeId,
    /// 返回值 ABI。
    pub return_abi: ParamAbi,
    /// 是否 extern（@Extern）。
    pub is_extern: bool,
    /// extern 符号名（@Extern(name = ...)）。
    pub extern_symbol: Option<String>,
    /// `@Intrinsic("name")` 内联 intrinsic 名。
    /// Some 时该声明是 `@Intrinsic` 方法（无 body），codegen 必须按此名内联
    /// lowering，**不得**当作 extern 运行时符号声明（否则会把 `scoop.core.Int.ushr`
    /// 错误映射成不存在的 `@scoop_ushr` 运行时符号）。
    pub intrinsic_name: Option<String>,
}

/// 函数 ABI 种类。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum LirCallableAbi {
    /// 普通函数 ABI：(args) -> R。
    Plain,
    /// EffectStep ABI：step(frame_ptr, resume_payload?) -> Step。
    EffectStep,
}

/// 参数。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LirParam {
    /// 参数名。
    pub name: String,
    /// 参数类型。
    pub ty: TypeId,
    /// 参数 ABI。
    pub abi: ParamAbi,
    /// 参数对应的 body local id（codegen 把第 i 个 LLVM 参数存入该 local，
    /// 不能假设参数总是占据 locals 0..n——闭包的捕获变量可能插在参数之间）。
    pub local_id: u32,
}

/// 参数 ABI（每个参数的传递方式）。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ParamAbi {
    /// 直接传递（标量/引用指针按值传）。
    Direct,
    /// 间接传递（大 aggregate 通过 hidden pointer 传）。
    Indirect,
}

/// 函数体。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LirBody {
    /// 局部变量声明。
    pub locals: Vec<LirLocalDecl>,
    /// 基本块列表。
    pub blocks: Vec<LirBlock>,
    /// 入口块 ID。
    pub start_block: u32,
}

/// 局部变量声明。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LirLocalDecl {
    /// local ID。
    pub id: u32,
    /// local 名称（None = 编译器临时）。
    pub name: Option<String>,
    /// local 类型。
    pub ty: TypeId,
    /// 是否可变。
    pub mutable: bool,
    /// 是否 GC-managed 引用。
    pub gc_traceable: bool,
}

/// 基本块。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LirBlock {
    /// 块 ID。
    pub id: u32,
    /// 语句列表。
    pub stmts: Vec<LirStmt>,
    /// 终结符。
    pub terminator: LirTerminator,
}

/// LIR 语句（从 MIR StatementKind 1:1 映射，附加布局信息）。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LirStmt {
    /// 源 span。
    pub span: scoop2_base::Span,
    /// 语句种类。
    pub kind: LirStmtKind,
}

/// LIR 语句种类。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum LirStmtKind {
    /// 空操作。
    Nop,
    /// `target = rvalue`。
    Assign { target: u32, value: LirRvalue },
    /// 成员字段写：`receiver.member = value`。
    StoreMember {
        receiver_local: LirOperand,
        receiver_ty: TypeId,
        member_name: String,
        field_offset: u64,
        value_local: LirOperand,
        value_ty: TypeId,
    },
    /// tuple 元素写：`receiver.<index> = value`。
    StoreTupleIndex {
        receiver_local: LirOperand,
        index: u128,
        value_local: LirOperand,
        value_ty: TypeId,
    },
    /// 顶层 `var` 写。
    StoreGlobal {
        global_fqn: String,
        value_local: LirOperand,
        value_ty: TypeId,
    },
    /// 运行期 panic。
    Panic { message: String },
}

/// LIR Rvalue（从 MIR Rvalue 映射，简化为 local-only 操作数 + 布局信息）。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum LirRvalue {
    /// 直接使用一个 local。
    Use(u32),
    /// 编译期常量值（codegen 将其物化到 local 或直接内联）。
    Const(LirConstValue),
    /// 函数调用。
    Call(LirCall),
    /// enum variant 构造。
    EnumVariant {
        enum_ty: TypeId,
        variant_name: String,
        tag_value: u64,
        args: Vec<LirOperand>,
        payload_ty: Option<TypeId>,
    },
    /// class/struct 构造。
    ClassCtor {
        class_fqn: String,
        args: Vec<LirOperand>,
        /// 选中的 ctor 声明 span.start（secondary ctor 时指向 `constructor` 关键字；
        /// None = primary ctor）。供 codegen 选择 `<Class>.$init` vs `<Class>.$ctor.s<start>`。
        selected_ctor_span_start: Option<usize>,
    },
    /// tuple 构造。
    MakeTuple {
        elements: Vec<LirOperand>,
        ty: TypeId,
    },
    /// 数组构造。
    MakeArray {
        elements: Vec<LirOperand>,
        ty: TypeId,
        /// 结果是否为 `MutableArray<T>`（true：不 freeze，返回可变数组本体）。
        mutable: bool,
    },
    /// struct 字面量。
    StructLit {
        type_fqn: String,
        fields: Vec<(String, LirOperand)>,
        ty: TypeId,
    },
    /// 闭包构造。
    MakeClosure {
        env_local: LirOperand,
        invoke_fqn: String,
    },
    /// 成员访问。
    MemberAccess {
        receiver_local: LirOperand,
        receiver_ty: TypeId,
        member_name: String,
        field_offset: u64,
        result_ty: TypeId,
    },
    /// tuple 索引。
    TupleIndex {
        receiver_local: LirOperand,
        index: u128,
        element_ty: TypeId,
    },
    /// 索引访问 `receiver[indices]`（operator get）。
    IndexAccess {
        receiver_local: LirOperand,
        index_locals: Vec<LirOperand>,
        element_ty: TypeId,
        /// receiver 是否为 `MutableArray<T>`（外置 data 指针布局；
        /// false = `Array<T>` 内联 data 布局或其他按 Array 布局处理的类型）。
        receiver_mutable: bool,
    },
    /// 类型测试 `is T`。
    TypeTest {
        value_local: LirOperand,
        target_ty: TypeId,
        /// 静态折叠结果（编译期已知 true/false 时直接折叠）。
        static_fold: scoop2_mir::mir::transport::RuntimeTypeStaticFold,
        /// 目标类型的运行时描述符键（含 FQN，供 codegen 计算 type_id）。
        descriptor: scoop2_mir::mir::transport::RuntimeTypeDescriptorKey,
    },
    /// 类型转换 `as T`。
    Cast {
        value_local: LirOperand,
        target_ty: TypeId,
        /// 目标类型的运行时描述符键（含 FQN，供 codegen 计算 type_id）。
        descriptor: scoop2_mir::mir::transport::RuntimeTypeDescriptorKey,
        /// 转换失败行为。
        failure: scoop2_mir::mir::transport::RuntimeCastFailure,
    },
    /// 模式匹配测试。
    PatternMatch {
        subject_local: LirOperand,
        pattern: LirPattern,
    },
    /// 模式提取。
    PatternExtract {
        subject_local: LirOperand,
        /// 提取路径（MIR 透传）：variant 字段提取携带 `VariantField { variant,
        /// field_index }`，codegen 据此计算多字段 variant payload 内的字段偏移。
        path: Vec<scoop2_mir::mir::transport::PatternBindingStep>,
        result_ty: TypeId,
    },
    /// 整数相等比较。
    IntEq {
        lhs_local: LirOperand,
        rhs_local: LirOperand,
    },
    /// 顶层值引用。
    TopLevelRef { fqn: String, ty: TypeId },
    /// f-string 拼接。
    InterpolatedString { parts: Vec<LirInterpolatedPart> },
    /// with 更新。
    WithUpdate {
        base_local: LirOperand,
        updates: Vec<LirWithUpdateField>,
        result_ty: TypeId,
    },
    /// class 元数据字面量 `T::class`。
    ClassLit { type_fqn: String },
    /// escape 捕获点构造 continuation 对象（canonical 布局，见 effect/mod.rs）。
    ///
    /// codegen 负责：堆分配 72B 对象（descriptor bitmap = 0b100，只 trace
    /// frame 指针）、写 resumed=0 / state / frame 指针 / step_fn 地址 /
    /// resume_value=0，返回 GC 指针。frame 指针与 step_fn 从所在函数的
    /// `LirEffectInfo` 推导（MakeContinuation 只可能出现在 EffectStep 函数体内）。
    MakeContinuation { state: u64 },
    /// 构造 chain link 对象（48B：frame@32 + step_fn@40）并写入 TLS
    /// `__scoop_effect_chain`。用于 EffectStep 函数把 callee 挂起继续向外
    /// 传播；`state` 是本函数传播路径的续点编号。产出 Unit。
    /// frame 指针与 step_fn 同样从 `LirEffectInfo` 推导。
    MakeChainLink { state: u64 },
    /// 读取 TLS `__scoop_effect_chain` 并清零（消费语义），产出 GC 指针
    /// （result_ty，Any ref）。callee 挂起传播到本函数时其 chain link 留在
    /// TLS；act 块首部取走存入 frame link 槽，或丢弃（abandon 语义）。
    TakeChainLink { result_ty: TypeId },
    /// 从本函数 frame 的 `link_slot` 槽取出 chain link，调用其
    /// `step_fn(link.frame, resume_word)`，产出 callee 的 Step 值。
    /// 仅出现在 EffectStep 函数的 call-chain resume 续点块。
    ResumeChainLink { link_slot: u64, result_ty: TypeId },
}

/// LIR 调用信息。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LirCall {
    /// 调用种类。
    pub kind: LirCallKind,
    /// 实参 local 列表。
    pub args: Vec<LirOperand>,
    /// 返回值类型。
    pub result_ty: TypeId,
}

/// LIR 调用种类。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum LirCallKind {
    /// 直接调用（已知 callee 符号名）。
    ///
    /// `callee_symbol` 在 `backfill_call_sites` 中被回填为 mangled 符号名
    /// （按 callee_fqn + stable_instance_key 解析），以支持同 FQN 多重载（如
    /// `println<Int>` / `println<String>`）。
    Direct {
        callee_symbol: String,
        /// 原始 callee FQN（回填前保留；回填后 callee_symbol 为 mangled 名）。
        callee_fqn: String,
        /// 实例键（携带泛型实参/eff 实参的稳定哈希），用于在同 FQN 重载中
        /// 精确定位目标符号；非泛型时为 None。
        stable_instance_key: Option<scoop2_mir::mir::StableInstanceKey>,
        /// `@Intrinsic("name")` 注解透传的 intrinsic 名。
        /// Some 时 codegen 直接按此名做 intrinsic 内联（不再按 FQN 启发式匹配）。
        /// None = 非 intrinsic 方法（普通函数调用）。
        intrinsic_name: Option<String>,
    },
    /// class 虚方法分发（vtable）。
    Virtual {
        receiver_local: LirOperand,
        /// 接收者的静态 owner FQN（声明该虚方法的类）。用于在 `backfill_call_sites`
        /// 时按 `(class_fqn, method_name)` 查找正确的 vtable slot。
        owner_fqn: String,
        method_name: String,
        vtable_slot: u32,
    },
    /// interface 分发（itable）。
    Interface {
        receiver_local: LirOperand,
        interface_fqn: String,
        method_name: String,
        interface_id: u64,
        itable_slot: u32,
    },
    /// 闭包调用。
    Closure { callee_local: LirOperand },
    /// 函数值调用。
    FunValue { callee_local: LirOperand },
    /// Continuation resume（`k.resume(value)`）。
    /// continuation 是 Continuation 对象引用；resume_value 是 resume 的实参。
    /// codegen 从 continuation 读取 step_fn 函数指针并间接调用。
    Resume {
        continuation: LirOperand,
        resume_value: LirOperand,
    },
}

/// LIR 模式。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum LirPattern {
    Wildcard,
    Bind {
        ty: TypeId,
    },
    IntLit(i128),
    CharLit(char),
    StringLit(String),
    BoolLit(bool),
    Is {
        ty: TypeId,
        negated: bool,
        target_fqn: Option<String>,
    },
    Tuple {
        elements: Vec<LirPattern>,
    },
    Struct {
        type_fqn: String,
        fields: Vec<(String, LirPattern)>,
    },
    Variant {
        variant_name: String,
        /// 变体判别值（enum_variants 声明序下标；None = 未知 / 外部 enum）。
        tag_value: Option<u64>,
        args: Vec<LirPattern>,
    },
    Or {
        patterns: Vec<LirPattern>,
    },
}

/// with 更新字段。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LirWithUpdateField {
    /// enum variant 目标（`err with { Ok.point.x: 7 }` 的首段 `Ok`）：
    /// 运行时 tag 必须等于 `tag_value`，否则 panic（exit 3）。
    /// Some 时 `path` 各段的 offset 是 enum 值内的**绝对**字节偏移
    /// （variant slot 起点累计），不再逐层相对。
    pub variant: Option<LirWithUpdateVariantTarget>,
    /// 更新路径（已逐段解析：字段名 + 在该层布局中的字节偏移 + 字段类型）。
    /// 单段 = 扁平更新；多段 = 嵌套更新（`line with { start.x: 99 }`）。
    pub path: Vec<LirWithUpdateSegment>,
    /// 新值。
    pub value: LirOperand,
    /// 值类型。
    pub value_ty: TypeId,
}

/// with 更新的 enum variant 目标（tag 运行时检查）。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LirWithUpdateVariantTarget {
    /// 变体名（诊断用）。
    pub name: String,
    /// 判别值（与 `EnumVariantLayout::tag_value` 同源）。
    pub tag_value: u64,
}

/// with 更新路径的一段。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LirWithUpdateSegment {
    /// 字段名（tuple 段为 `_N`；仅用于诊断）。
    pub name: String,
    /// 字段在该层 struct/tuple 布局中的字节偏移（与 TypeLayoutTable 同源）。
    pub offset: u64,
    /// 字段类型（下一层的 receiver 类型）。
    pub ty: TypeId,
}

/// f-string 片段。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum LirInterpolatedPart {
    Lit(String),
    Expr(LirOperand),
}

/// LIR 终结符。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum LirTerminator {
    /// 函数返回。
    Return { value: Option<LirOperand> },
    /// 无条件跳转。
    Goto { target: u32 },
    /// 条件分支。
    CondBr {
        cond: LirOperand,
        then_target: u32,
        else_target: u32,
    },
    /// 不可达。
    Unreachable,
}

// =========================================================================
// 分发表
// =========================================================================

/// class vtable 布局。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct VtableLayout {
    /// class FQN。
    pub class_fqn: String,
    /// slot 列表（含超类继承的 slot）。
    pub slots: Vec<VtableSlot>,
}

/// vtable slot。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct VtableSlot {
    /// slot 索引（在 vtable 中的位置）。
    pub slot_index: u32,
    /// 方法名。
    pub method_name: String,
    /// 声明此方法的 owner FQN。
    pub owner_fqn: String,
    /// overload 签名（canonical 文本）。
    pub overload_sig: String,
    /// 目标函数符号名（mangled）。
    pub target_symbol: String,
}

/// interface itable 定义。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ItableLayout {
    /// interface FQN。
    pub interface_fqn: String,
    /// 全局唯一 interface ID。
    pub interface_id: u64,
    /// slot 列表。
    pub slots: Vec<ItableSlot>,
}

/// itable slot。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ItableSlot {
    /// slot 索引。
    pub slot_index: u32,
    /// 方法名。
    pub method_name: String,
    /// overload 签名。
    pub overload_sig: String,
}

/// class × interface 的 itable 实现映射。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ClassItableLayout {
    /// class FQN。
    pub class_fqn: String,
    /// interface FQN。
    pub interface_fqn: String,
    /// interface ID。
    pub interface_id: u64,
    /// slot_index → 实现函数符号名。
    pub method_impls: Vec<Option<String>>,
}

// =========================================================================
// GC 信息
// =========================================================================

/// 函数的 GC 语义信息（LIR 产出，codegen 消费）。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct GcInfo {
    /// 此函数中所有 GC-managed local 的列表。
    pub gc_locals: Vec<GcLocal>,
    /// 此函数中的所有 safepoint。
    pub safepoints: Vec<GcSafepoint>,
}

/// 一个 GC-managed local。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct GcLocal {
    /// local ID。
    pub local_id: u32,
    /// 引用类型。
    pub ty: TypeId,
    /// 基指针来源（当前总是 None）。
    pub base_local: Option<u32>,
}

/// 一个 safepoint。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct GcSafepoint {
    /// 块 ID。
    pub block_id: u32,
    /// 语句索引。
    pub stmt_index: u32,
    /// safepoint 类型。
    pub kind: SafepointKind,
    /// 存活的 GC-managed local ID 列表。
    pub live_gc_locals: Vec<u32>,
}

/// safepoint 类型。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum SafepointKind {
    /// 函数调用。
    Call { callee_symbol: String },
    /// 纯 GC poll。
    Poll,
    /// effect 挂起。
    EffectSuspend,
}

/// 类型描述符（GC 运行时用）。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TypeDescriptor {
    /// 类型 FQN。
    pub type_fqn: String,
    /// 大小。
    pub size: u64,
    /// 对齐。
    pub align: u64,
    /// GC 指针偏移列表。
    pub trace_offsets: Vec<u64>,
    /// @ReleaseHook 函数符号。
    pub release_fn: Option<String>,
    /// RTTI 类型 ID。
    pub type_id: u64,
    /// 超类类型 ID。
    pub parent_type_id: Option<u64>,
}

// =========================================================================
// Effect Step 信息
// =========================================================================

/// EffectStep 函数的 frame schema。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct FrameSchema {
    /// frame tuple 类型。
    pub frame_ty: TypeId,
    /// slot 列表。
    pub slots: Vec<FrameSlot>,
}

/// EffectStep 函数的 state dispatch 信息（resume-state → block-id 映射）。
/// 供 codegen 生成分发代码（jump table 或 CondBr 链）。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct StateDispatch {
    /// state 值 → 重入块 ID 的映射。
    /// state 0 = 初始入口；state N = 第 N 个 Perform 的 resume 续点。
    pub entries: Vec<StateDispatchEntry>,
}

/// 单个 state dispatch 条目。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct StateDispatchEntry {
    /// state 值。
    pub state_value: u32,
    /// 对应的重入块 ID。
    pub block_id: u32,
}

/// frame slot。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct FrameSlot {
    /// slot 索引。
    pub slot_index: u32,
    /// slot 种类。
    pub kind: FrameSlotKind,
    /// slot 类型。
    pub ty: TypeId,
    /// 是否 GC-managed。
    pub gc_traceable: bool,
}

/// frame slot 种类。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum FrameSlotKind {
    /// state 字段（Int）。
    State,
    /// 源程序 local。
    SourceLocal { local_id: u32 },
}

/// Step enum 布局。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct StepLayout {
    /// Step 类型。
    pub step_ty: TypeId,
    /// Complete 变体。
    pub complete_variant: StepVariantLayout,
    /// effect 操作变体。
    pub effect_variants: Vec<StepVariantLayout>,
}

/// Step 变体布局。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct StepVariantLayout {
    /// 变体名。
    pub name: String,
    /// tag 值。
    pub tag_value: u64,
    /// payload 类型（None = 无 payload）。
    pub payload: Option<TypeId>,
}

/// Continuation 对象布局。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ContinuationLayout {
    /// continuation 类型 FQN。
    pub cont_fqn: String,
    /// 字段列表。
    pub fields: Vec<ContinuationField>,
}

/// continuation 字段。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ContinuationField {
    /// 字段名。
    pub name: String,
    /// 偏移。
    pub offset: u64,
    /// 类型。
    pub ty: TypeId,
    /// 字段种类。
    pub kind: ContinuationFieldKind,
}

/// continuation 字段种类。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ContinuationFieldKind {
    /// GC 对象头。
    Header,
    /// resumed 标志（Bool）。
    ResumedFlag,
    /// resume state tag（Int）。
    ResumeStateTag,
    /// frame 指针。
    FramePtr,
    /// step 函数指针。
    StepFnPtr,
    /// resume value。
    ResumeValue,
}

// =========================================================================
// 全局初始化
// =========================================================================

/// 全局初始化计划。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct GlobalInitPlan {
    /// 初始化条目（按执行顺序）。
    pub entries: Vec<GlobalInitEntry>,
}

/// 全局初始化条目。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct GlobalInitEntry {
    /// 全局变量 FQN。
    pub fqn: String,
    /// 类型。
    pub ty: TypeId,
    /// 是否可变（var vs val）。
    pub is_var: bool,
    /// 初始化函数符号名。
    pub init_callable: String,
}

/// 类初始化计划。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ClassInitPlan {
    /// class FQN。
    pub class_fqn: String,
    /// 字段初始化列表。
    pub field_inits: Vec<FieldInit>,
    /// 超类初始化函数。
    pub super_init: Option<String>,
    /// 构造器初始化块列表（每个块对应一个可调用的初始化函数符号）。
    /// 当前 Scoop 的主构造器逻辑在 LIR 中尚未展开为独立 callable，故留空；
    /// 待构造器 lowering 暴露 per-block callable 后填充。
    pub init_blocks: Vec<InitBlock>,
}

/// 类构造器的初始化块。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct InitBlock {
    /// 该初始化块对应的可调用函数符号（mangled）。
    pub body_callable: String,
}

/// 字段初始化。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct FieldInit {
    /// 字段名。
    pub field_name: String,
    /// 字段类型。
    pub ty: TypeId,
    /// 初始化种类。
    pub init_kind: InitKind,
}

/// 字段初始化种类。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum InitKind {
    /// 默认值（零初始化）。
    DefaultValue,
    /// 属性参数（主构造器参数赋值）。
    PropertyParam,
    /// 属性初始化器（属性声明处的初始值）。
    PropertyInitializer,
}
