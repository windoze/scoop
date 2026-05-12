//! LLVM backend lowering root.
//!
//! 当前 `codegen/mod.rs` 的职责上界已经从“巨型主题混合文件”收口为：
//! - `CompilationUnitCodegenCx` / `MainCodegen` 等共享上下文与构造入口；
//! - 顶层 `const` / `immutable` / `var` 初始化与访问；
//! - 字面量、聚合值、成员访问、运算符、类型转换、通用 coercion 等 generic lowering；
//! - GC-sensitive spill/root/sret/return helper、通用 lvalue bridge、具体类型恢复等跨主题 helper。
//!
//! 其余主题 lowering 已拆到独立模块：
//! - `call/`：调用分派、调用点 ABI / 实参绑定、ordinary callee resume 与 effect-call wrapper；
//! - `intrinsics/`：builtin 与 sysroot intrinsics；
//! - `closure/` / `class_ctor.rs`：closure lowering 与 class constructor lowering；
//! - `enum_lowering.rs` / `object_init.rs`：enum constructor/object singleton 相关 lowering；
//! - `effect/`、`gc.rs`、`runtime_abi.rs` 等继续保持独立主题边界。
//!
//! `T5000c` 已将 `ProgramFacts` / `EffectAnalysisCtx` / `ExprFactResolver` 这类 shared facts
//! 抽离到 backend 外的共享层；这里当前只消费这些 backend-agnostic 输入，并继续朝
//! “只做 backend lowering”的边界收口。后续 `T5000d+` 将让 early MIR / summary 直接复用
//! 同一层共享事实，而不是回到 LLVM 现场拼装分析输入。

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::path::Path;
use std::rc::Rc;

use inkwell::AddressSpace;
use inkwell::FloatPredicate;
use inkwell::IntPredicate;
use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Linkage;
use inkwell::module::Module;
use inkwell::targets::TargetData;
use inkwell::types::AnyType;
use inkwell::types::BasicMetadataTypeEnum;
use inkwell::types::BasicType;
use inkwell::types::BasicTypeEnum;
use inkwell::types::FunctionType;
use inkwell::types::IntType;
use inkwell::types::PointerType;
use inkwell::types::StructType;
use inkwell::values::AggregateValueEnum;
use inkwell::values::AsValueRef;
use inkwell::values::BasicValue;
use inkwell::values::BasicValueEnum;
use inkwell::values::CallSiteValue;
use inkwell::values::FloatValue;
use inkwell::values::FunctionValue;
use inkwell::values::GlobalValue;
use inkwell::values::IntValue;
use inkwell::values::PointerValue;

use crate::ast;
use crate::effect::state_machine::CalleeSuspendPlan;
use crate::expr_facts::ExprFactResolver;
use crate::hir;
use crate::llvm::target::HostTargetInfo;
use crate::program_facts::ProgramFacts;
use crate::source::{SourceFile, SourceId, SourceMap};
use crate::stable_id::{StableHashScope, stable_hash64};
use crate::syntax::int_literal::{parse_int_literal, parse_int_literal_checked};
use crate::syntax::string_literal::{
    StringLiteralParseError, parse_normal_string_bytes, parse_string_literal_bytes,
};
use crate::ty::layout::{NicheStorage, TypeLayout};
use crate::ty::{
    BuiltinTypes, NominalType, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind,
};

use super::LlvmEmitError;

mod call;
mod class_ctor;
mod closure;
mod composite_transport;
mod control_flow;
mod effect_ctx;
mod effect_lowered;
mod effect_outcome;
mod enum_lowering;
mod expr;
mod gc;
mod intrinsics;
mod layout;
mod mir_body;
mod object_init;
mod ordinary_callee;
mod runtime_abi;
mod runtime_symbols;
mod stmt;
mod ty;
mod types;

use types::{
    CgEnumLayout, CgEnumPayload, CgEnumRepr, CgEnumVariant, CgTy, CgValue, GC_ADDRSPACE, IntTy,
};

/// 一次调用点中某个"已按形参顺序归位"的 LLVM 实参结果。
///
/// `pointer_value` 只在值最终表现为指针时填充，供 vtable/itable 等需要复用 receiver
/// 原始指针的路径读取；普通 direct call / function-value call 仅消费 `value`。
#[derive(Clone)]
struct EvaluatedCallArg<'ctx> {
    value: inkwell::values::BasicMetadataValueEnum<'ctx>,
    pointer_value: Option<PointerValue<'ctx>>,
    cleanup_spills: Vec<DeferredGcSensitiveSpill<'ctx>>,
}

/// 一个“已求值，但不能继续依赖 SSA 跨后续子表达式存活”的中间值。
///
/// extern/native 调用期间，moving GC 会直接更新 managed locals / temp spill slots；
/// 若外层表达式把带 GC refs 的中间值继续保留在 SSA 里，extern 返回后这些 SSA 会变 stale。
/// 因此，凡是需要跨“后续子表达式求值”保存的 GC-sensitive 值，都先落到临时 slot，
/// 最终消费时再 reload。
#[derive(Clone)]
struct DeferredGcSensitiveSpill<'ctx> {
    slot: PointerValue<'ctx>,
    value_ty: BasicTypeEnum<'ctx>,
}

#[derive(Clone)]
struct DeferredCgValue<'ctx> {
    ty: CgTy,
    immediate: Option<BasicValueEnum<'ctx>>,
    spill: Option<DeferredGcSensitiveSpill<'ctx>>,
}

#[derive(Clone, Copy)]
enum OrdinaryParamAbi<'ctx> {
    Direct {
        cg_ty: CgTy,
        llvm_param_ty: BasicMetadataTypeEnum<'ctx>,
    },
    IndirectGcAggregate {
        cg_ty: CgTy,
        llvm_param_ty: BasicMetadataTypeEnum<'ctx>,
        pointee_ty: BasicTypeEnum<'ctx>,
    },
}

impl<'ctx> OrdinaryParamAbi<'ctx> {
    fn cg_ty(self) -> CgTy {
        match self {
            Self::Direct { cg_ty, .. } | Self::IndirectGcAggregate { cg_ty, .. } => cg_ty,
        }
    }

    fn llvm_param_ty(self) -> BasicMetadataTypeEnum<'ctx> {
        match self {
            Self::Direct { llvm_param_ty, .. }
            | Self::IndirectGcAggregate { llvm_param_ty, .. } => llvm_param_ty,
        }
    }

    fn pointee_ty(self) -> Option<BasicTypeEnum<'ctx>> {
        match self {
            Self::Direct { .. } => None,
            Self::IndirectGcAggregate { pointee_ty, .. } => Some(pointee_ty),
        }
    }
}

#[derive(Clone, Copy)]
enum CallArgAbiMode {
    Native,
    Ordinary,
}

#[derive(Clone, Copy)]
struct BoundCallArgsSpec {
    span: crate::span::Span,
    callee_span: crate::span::Span,
    kind: &'static str,
    abi_mode: CallArgAbiMode,
}

#[derive(Clone, Copy)]
struct CallableValueCallSpec<'a> {
    span: crate::span::Span,
    callee_span: crate::span::Span,
    call_may_suspend: bool,
    fun_ty: &'a crate::ty::FunctionType,
    args: &'a [hir::CallArg],
}

/// 一个局部变量（`val`/`var`）在 LLVM 里的存储形态。
///
/// 当前阶段（T0809）统一用栈分配（`alloca`）承载 locals，并用 `load/store` 实现读写。
#[derive(Debug, Clone, Copy)]
struct CgLocal<'ctx> {
    /// 该局部绑定在 HIR/type 层面的原始 `TypeId`（用于需要"精确类型结构"的场景，例如函数值调用）。
    ///
    /// 说明：
    /// - 早期 codegen 的 `CgTy::Ref` 统一覆盖所有引用类型（Any/class/function/union...），
    ///   但某些操作（例如调用函数值）仍需要区分具体的 `RefTypeKind::Function` 并读取其签名。
    /// - 对于无法在 codegen 阶段轻易恢复 `TypeId` 的合成 locals，可为 `None`。
    hir_ty: Option<TypeId>,
    /// 对函数值局部记录“调用时是否可能触发 suspend boundary”。
    ///
    /// 该标记不仅覆盖显式 effect row，也覆盖 hidden suspend 来源
    /// （例如 object/class init、runtime raise）经由局部函数值传播的情况。
    call_may_suspend: bool,
    ty: CgTy,
    ptr: PointerValue<'ctx>,
    /// 对于 state-machine 中“持久化在 heap frame 字段里”的 locals：
    /// `ptr` 指向稳定的执行期 local home（entry alloca），而该字段指向对应的 heap frame slot。
    ///
    /// 目的：避免把 heap-frame GEP 长期暴露成 env local home（moving GC 后可能 stale），
    /// 同时仍能在每次写入执行期 local home 时把最新值写回持久化 frame。
    frame_backing_ptr: Option<PointerValue<'ctx>>,
    mutable: bool,
}

#[derive(Clone, Copy)]
struct TrackedGcRootSlot<'ctx> {
    slot: PointerValue<'ctx>,
    value_ptr_ty: PointerType<'ctx>,
    frame_slot: PointerValue<'ctx>,
}

#[derive(Default)]
struct ExplicitFrameLayoutPlan<'ctx> {
    function_symbol: Option<String>,
    frame_storage: Option<PointerValue<'ctx>>,
    slot_tys: Vec<PointerType<'ctx>>,
}

#[derive(Clone, Copy)]
struct OrdinaryParamLocalBinding<'ctx, 'a> {
    at: crate::span::Span,
    llvm_fun: FunctionValue<'ctx>,
    param_index: u32,
    name: &'a str,
    id: hir::SymbolId,
    ty_id: TypeId,
    call_may_suspend: bool,
    missing_kind: &'static str,
}

#[derive(Clone, Copy)]
struct AddressablePlace<'ctx> {
    ptr: PointerValue<'ctx>,
    ty: CgTy,
    writable: bool,
}

#[derive(Clone)]
struct DeferredClassFieldPlace<'ctx> {
    class: hir::ClassInit,
    field_idx: u32,
    field_cg: CgTy,
    writable: bool,
    receiver: DeferredCgValue<'ctx>,
}

#[derive(Debug, Default, Clone)]
struct Env<'ctx> {
    scopes: Vec<HashMap<hir::SymbolId, CgLocal<'ctx>>>,
}

impl<'ctx> Env<'ctx> {
    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        let _ = self.scopes.pop();
    }

    fn insert(&mut self, id: hir::SymbolId, local: CgLocal<'ctx>) {
        if let Some(frame) = self.scopes.last_mut() {
            frame.insert(id, local);
        }
    }

    fn get(&self, id: hir::SymbolId) -> Option<CgLocal<'ctx>> {
        for frame in self.scopes.iter().rev() {
            if let Some(local) = frame.get(&id).copied() {
                return Some(local);
            }
        }
        None
    }

    fn get_mut(&mut self, id: hir::SymbolId) -> Option<&mut CgLocal<'ctx>> {
        for frame in self.scopes.iter_mut().rev() {
            if let Some(local) = frame.get_mut(&id) {
                return Some(local);
            }
        }
        None
    }
}

/// 普通 indirect callee suspend-state prefix 的固定字段布局。
pub(super) const CALLEE_SUSPEND_STATE_RESUME_ENTRY_FN_INDEX: u32 = 4;
pub(super) const CALLEE_SUSPEND_STATE_USER_FIELD_BASE_INDEX: u32 =
    CALLEE_SUSPEND_STATE_RESUME_ENTRY_FN_INDEX + 1;

/// 单个编译单元内可跨多个 `MainCodegen` 复用的共享 cache。
///
/// 当前先收口两类内容：
/// - suspend / outward-effect 相关分析缓存；
/// - layout / enum / class init / packed-field 索引缓存。
///
/// 这些缓存都不属于“单个函数体 lowering 的临时状态”，因此不应继续挂在
/// `MainCodegen` 上随着 `fresh_main_codegen()` / `fresh_child_codegen()` 重建。
#[derive(Default)]
struct SharedCodegenCaches {
    known_fun_call_suspend_cache: RefCell<Option<HashMap<String, bool>>>,
    type_layout_cache: RefCell<HashMap<TypeId, TypeLayout>>,
    option_niche_cache: RefCell<HashMap<TypeId, Option<(NicheStorage, u64)>>>,
    enum_cg_layout_cache: RefCell<HashMap<TypeId, CgEnumLayout>>,
    class_init_layout_cache: RefCell<HashMap<String, hir::ClassInit>>,
    pack_field_indices: RefCell<HashMap<String, Vec<u32>>>,
    callable_carrier_contract_enabled: Cell<bool>,
    callable_carrier_entry_symbols: RefCell<HashMap<(CallableCarrierKind, String), String>>,
    plain_callable_carrier_fallback_targets: RefCell<HashSet<(CallableCarrierKind, String)>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum CallableCarrierKind {
    ClosureObject,
    ClassVtable,
    InterfaceItable,
}

impl CallableCarrierKind {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::ClosureObject => "closure callable object",
            Self::ClassVtable => "class vtable slot",
            Self::InterfaceItable => "interface itable slot",
        }
    }
}

/// 单个编译单元内可跨多个 `MainCodegen` 复用的稳定输入与共享状态。
///
/// 这一层先只承接：
/// - module 级只读输入；
/// - 编译单元级共享事实；
/// - child-codegen 之间必须一致的共享状态与共享 cache。
///
/// 函数 / body 生命周期状态现已收口到 `FunctionBodyCodegenCx`，并由 `MainCodegen`
/// 作为独立子上下文持有。
pub(crate) struct CompilationUnitCodegenCx<'a, 'ctx> {
    context: &'ctx Context,
    module: &'a Module<'ctx>,
    builder: &'a Builder<'ctx>,
    target_data: &'a TargetData,
    host: &'a HostTargetInfo,
    source_map: &'a SourceMap,
    entry_source_id: SourceId,
    types: &'a TypeStore,
    struct_layouts: &'a hir::StructLayoutIndex,
    enum_layouts: &'a hir::EnumLayoutIndex,
    top_level_vars: &'a hir::TopLevelVarIndex,
    top_level_consts: &'a hir::TopLevelConstIndex,
    top_level_immutable_values: &'a hir::TopLevelImmutableValueIndex,
    top_level_fun_call_sites: &'a hir::TopLevelFunCallSiteIndex,
    extern_globals: &'a hir::ExternGlobalIndex,
    extern_funs: &'a hir::ExternFunIndex,
    object_inits: &'a hir::ObjectInitIndex,
    class_inits: &'a hir::ClassInitIndex,
    class_vtables: &'a crate::vtable::ClassVtableIndex,
    interfaces: &'a crate::itable::InterfaceIndex,
    class_itables: &'a crate::itable::ClassItableIndex,
    ctor_call_sites: &'a hir::CtorCallSiteIndex,
    dispatch_call_sites: &'a hir::DispatchCallSiteIndex,
    #[allow(dead_code)]
    effect_op_call_sites: &'a hir::EffectOpCallSiteIndex,
    continuation_resume_call_sites: &'a hir::ContinuationResumeCallSiteIndex,
    when_pat_binding_tys: &'a hir::WhenPatBindingTypeIndex,
    nominal_kinds: &'a hir::NominalKindIndex,
    direct_supertypes: &'a hir::DirectSupertypesIndex,
    builtins: BuiltinTypes,
    fun_index: &'a HashMap<String, &'a hir::FunDecl>,
    /// production/codegen 主路径显式接入的 canonical materialized MIR/pass 视图。
    ///
    /// reachability、callable body-presence、known fun suspendability 查询与显式
    /// pass-rewritten callable body lowering 会优先观察该 pass 产物层。
    materialized_pass_view: Option<crate::mir::MaterializedMirPassView<'a>>,
    /// ABI 可见性阶段发布的 callable contract。
    ///
    /// 这里先只承接“某个 callable root 是否需要 effect hidden ABI / resume shell”这类
    /// 声明层判断，避免继续从 HIR 的 effectful 布尔值回推 ABI 形状。
    published_late_lowered_program: Option<&'a crate::effect_lowered::LateLoweredProgram>,
    /// backend-agnostic 的共享程序事实。
    program_facts: Rc<ProgramFacts>,
    /// 编译单元级共享 analysis/layout cache。
    shared_caches: SharedCodegenCaches,
    /// Effect op_tag 分配状态（T1608）：整个编译单元共享的 FQN → tag 表。
    ///
    /// 说明：
    /// - 每个 effect operation 的 FQN 在单次编译中对应唯一的 `op_tag`；
    /// - `scoop.core.Raise.raise` 固定为 1（与 runtime 约定兼容）；
    /// - 其余 effect op 从 2 开始递增分配；
    /// - 所有顶层函数 / nested helper / step trampoline 都必须共享同一份状态，
    ///   否则跨函数 perform 的 op_tag 会错位。
    effect_op_tags: Rc<RefCell<EffectOpTagState>>,
    /// 编译单元内“effect FQN -> 已知 effect 实例 TypeId 列表”。
    ///
    /// 用途：
    /// - same-op multi-arm dispatch 需要把 runtime perform-slot 中的 effect-instance key
    ///   与“当前程序里可能出现的 effect 实例集合”对齐；
    /// - 这里采用闭包世界（当前编译单元）收集，避免在 emitter 内重复扫描 `TypeStore`。
    known_effect_instances_by_effect_fqn: HashMap<String, Vec<TypeId>>,
}

/// 单个函数 / body lowering 生命周期内的局部状态。
///
/// 这类状态不应混入编译单元级共享输入，因为：
/// - `fresh_main_codegen()` / `fresh_child_codegen()` 进入新函数体时必须整体重置；
/// - effect/state-machine emitter 进入 runtime function 时，也需要成组保存/恢复它。
#[derive(Default)]
struct FunctionBodyCodegenCx<'ctx> {
    env: Env<'ctx>,
    tracked_gc_root_slots: Vec<TrackedGcRootSlot<'ctx>>,
    explicit_frame_layout: ExplicitFrameLayoutPlan<'ctx>,
    explicit_frame_slot_mirrors: HashMap<usize, Vec<PointerValue<'ctx>>>,
    current_fun_return_ty: Option<CgTy>,
    current_callable_fqn: Option<String>,
    loop_context_stack: Vec<LoopContext<'ctx>>,
    return_context: Option<ReturnContext<'ctx>>,
    current_sret_return_ptr: Option<PointerValue<'ctx>>,
    current_effect_ctx_ref: Option<PointerValue<'ctx>>,
    current_incoming_resume_token_ref: Option<PointerValue<'ctx>>,
    current_effect_outcome_ptr: Option<PointerValue<'ctx>>,
    local_effect_escape_targets: Vec<inkwell::basic_block::BasicBlock<'ctx>>,
    top_level_const_eval_stack: Vec<String>,
}

/// ordinary callee suspend/resume lowering 的专属运行态。
///
/// 这组状态只在“普通函数体需要把 perform 外传到外层 state-machine，再由 resume thunk
/// 回放原 call-site”这一 effect lowering 路径中有意义；它不属于 generic lowering
/// 或普通函数 / body 生命周期上下文。
#[allow(dead_code)]
#[derive(Default)]
struct CalleeSuspendLoweringCodegenCx<'ctx> {
    current_suspend_plan: Option<CalleeSuspendPlan>,
    current_resume_entry_fn: Option<FunctionValue<'ctx>>,
}

/// state-machine step/dispatch 中 suspend-site outcome 捕获的专属运行态。
#[derive(Default)]
struct SuspendSiteEffectOutcomeCodegenCx<'ctx> {
    active_capture: Option<ActiveSuspendSiteEffectOutcomeCapture>,
    explicit_outcomes: HashMap<u32, PointerValue<'ctx>>,
}

/// effect lowering / state-machine emitter 的专属上下文。
///
/// 它集中承接所有不应继续平铺在 `MainCodegen` 上的 effect 专属运行态，使：
/// - 独立 runtime function（step/dispatch、callee resume entry）可以整组保存/恢复；
/// - state-machine emitter 不再手工保存/恢复多串 `MainCodegen` 字段；
/// - `MainCodegen` 的剩余字段更接近 generic lowering / function-body 边界。
#[derive(Default)]
struct EffectLoweringCodegenCx<'ctx> {
    callee_suspend: CalleeSuspendLoweringCodegenCx<'ctx>,
    suspend_site_effect_outcomes: SuspendSiteEffectOutcomeCodegenCx<'ctx>,
}

pub(crate) struct MainCodegen<'a, 'ctx> {
    shared: &'a CompilationUnitCodegenCx<'a, 'ctx>,
    current_source_id: SourceId,
    function_cx: FunctionBodyCodegenCx<'ctx>,
    effect_cx: EffectLoweringCodegenCx<'ctx>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LlvmFunctionDeclarationSurface {
    ExportedAbi,
    RuntimeOrNativeImport,
    CompilerPrivateHelper,
}

impl LlvmFunctionDeclarationSurface {
    const fn label(self) -> &'static str {
        match self {
            Self::ExportedAbi => "exported ABI",
            Self::RuntimeOrNativeImport => "runtime/native import",
            Self::CompilerPrivateHelper => "compiler-private helper",
        }
    }
}

/// 所有 LLVM function declaration 都必须先声明外部 surface，再显式给出 linkage。
///
/// 当前允许保留 external 的显式例外包括：
/// - source/user exported callable 与宿主固定入口 `main`
/// - `malloc` / `exit`
/// - runtime ABI entry（如 `scoop_runtime_init` / `scoop_entry_argv_array`）
/// - `@Extern` 指定的 native symbol
///
/// 其余 compiler-private helper 也必须先走 `CompilerPrivateHelper` 分类，并显式
/// 选择 `Internal` / `Private` linkage。
fn declare_classified_llvm_function<'ctx>(
    module: &Module<'ctx>,
    name: &str,
    fn_ty: FunctionType<'ctx>,
    surface: LlvmFunctionDeclarationSurface,
    linkage: Linkage,
) -> FunctionValue<'ctx> {
    match surface {
        LlvmFunctionDeclarationSurface::ExportedAbi
        | LlvmFunctionDeclarationSurface::RuntimeOrNativeImport => {
            assert_eq!(
                linkage,
                Linkage::External,
                "{name} declared as {} must stay external",
                surface.label()
            );
        }
        LlvmFunctionDeclarationSurface::CompilerPrivateHelper => {
            assert!(
                matches!(
                    linkage,
                    Linkage::External | Linkage::Internal | Linkage::Private
                ),
                "{name} declared as compiler-private helper must use explicit external/internal/private linkage"
            );
        }
    }
    if let Some(existing) = module.get_function(name) {
        return existing;
    }
    module.add_function(name, fn_ty, Some(linkage))
}

pub(crate) fn declare_exported_abi_function<'ctx>(
    module: &Module<'ctx>,
    name: &str,
    fn_ty: FunctionType<'ctx>,
) -> FunctionValue<'ctx> {
    declare_classified_llvm_function(
        module,
        name,
        fn_ty,
        LlvmFunctionDeclarationSurface::ExportedAbi,
        Linkage::External,
    )
}

pub(crate) fn declare_runtime_or_native_import_function<'ctx>(
    module: &Module<'ctx>,
    name: &str,
    fn_ty: FunctionType<'ctx>,
) -> FunctionValue<'ctx> {
    declare_classified_llvm_function(
        module,
        name,
        fn_ty,
        LlvmFunctionDeclarationSurface::RuntimeOrNativeImport,
        Linkage::External,
    )
}

pub(crate) fn declare_compiler_private_helper_function<'ctx>(
    module: &Module<'ctx>,
    name: &str,
    fn_ty: FunctionType<'ctx>,
    linkage: Linkage,
) -> FunctionValue<'ctx> {
    declare_classified_llvm_function(
        module,
        name,
        fn_ty,
        LlvmFunctionDeclarationSurface::CompilerPrivateHelper,
        linkage,
    )
}

/// T0141: Loop context for break/continue targets.
#[derive(Debug, Clone, Copy)]
struct LoopContext<'ctx> {
    break_bb: inkwell::basic_block::BasicBlock<'ctx>,
    continue_bb: inkwell::basic_block::BasicBlock<'ctx>,
}

/// T0141: Function-level return context for early return from nested blocks.
#[derive(Debug, Clone, Copy)]
struct ReturnContext<'ctx> {
    return_bb: inkwell::basic_block::BasicBlock<'ctx>,
    /// Alloca for storing the return value (None for Unit return type).
    return_alloca: Option<inkwell::values::PointerValue<'ctx>>,
}

#[derive(Debug, Clone, Copy)]
struct ActiveSuspendSiteEffectOutcomeCapture {
    site_id: u32,
    call_span: crate::span::Span,
    capture_any: bool,
}

/// 稳定 effect op-tag 分配器：为每个 effect op FQN 分配唯一 u32 tag，
/// 供 state machine emitter、codegen_perform_expr 与 emit_raise 消费。
#[derive(Debug)]
pub(super) struct EffectOpTagState {
    map: HashMap<String, u32>,
    next: u32,
}

impl EffectOpTagState {
    pub(super) fn new() -> Self {
        let mut map = HashMap::new();
        // Raise.raise 固定为 1。
        map.insert("scoop.core.Raise.raise".to_string(), 1u32);
        Self { map, next: 2 }
    }
}

const EFFECT_INSTANCE_KEY_RAISE_RUNTIME_ERROR: u32 = u32::MAX;

fn collect_known_effect_instance_types_by_effect_fqn(
    types: &TypeStore,
    nominal_kinds: &hir::NominalKindIndex,
) -> HashMap<String, Vec<TypeId>> {
    let mut by_effect_fqn: HashMap<String, Vec<TypeId>> = HashMap::new();

    for type_id in types.iter_ids() {
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = types.kind(type_id) else {
            continue;
        };
        if !matches!(nominal_kinds.get(&nominal.fqn), Some(ast::TypeKind::Effect)) {
            continue;
        }
        by_effect_fqn
            .entry(nominal.fqn.clone())
            .or_default()
            .push(type_id);
    }

    for ids in by_effect_fqn.values_mut() {
        ids.sort_by(|lhs, rhs| {
            let lhs_display = types.display(*lhs).to_string();
            let rhs_display = types.display(*rhs).to_string();
            lhs_display.cmp(&rhs_display).then_with(|| lhs.cmp(rhs))
        });
        ids.dedup();
    }

    by_effect_fqn
}

pub(super) struct CompilationUnitCodegenInputs<'a, 'ctx> {
    pub(super) context: &'ctx Context,
    pub(super) module: &'a Module<'ctx>,
    pub(super) builder: &'a Builder<'ctx>,
    pub(super) target_data: &'a TargetData,
    pub(super) host: &'a HostTargetInfo,
    pub(super) source_map: &'a SourceMap,
    pub(super) entry_source_id: SourceId,
    pub(super) types: &'a TypeStore,
    pub(super) struct_layouts: &'a hir::StructLayoutIndex,
    pub(super) enum_layouts: &'a hir::EnumLayoutIndex,
    pub(super) top_level_vars: &'a hir::TopLevelVarIndex,
    pub(super) top_level_consts: &'a hir::TopLevelConstIndex,
    pub(super) top_level_immutable_values: &'a hir::TopLevelImmutableValueIndex,
    pub(super) top_level_fun_call_sites: &'a hir::TopLevelFunCallSiteIndex,
    pub(super) extern_globals: &'a hir::ExternGlobalIndex,
    pub(super) object_inits: &'a hir::ObjectInitIndex,
    pub(super) class_inits: &'a hir::ClassInitIndex,
    pub(super) class_vtables: &'a crate::vtable::ClassVtableIndex,
    pub(super) interfaces: &'a crate::itable::InterfaceIndex,
    pub(super) class_itables: &'a crate::itable::ClassItableIndex,
    pub(super) ctor_call_sites: &'a hir::CtorCallSiteIndex,
    pub(super) dispatch_call_sites: &'a hir::DispatchCallSiteIndex,
    pub(super) effect_op_call_sites: &'a hir::EffectOpCallSiteIndex,
    pub(super) continuation_resume_call_sites: &'a hir::ContinuationResumeCallSiteIndex,
    pub(super) when_pat_binding_tys: &'a hir::WhenPatBindingTypeIndex,
    pub(super) nominal_kinds: &'a hir::NominalKindIndex,
    pub(super) direct_supertypes: &'a hir::DirectSupertypesIndex,
    pub(super) builtins: BuiltinTypes,
    pub(super) extern_funs: &'a hir::ExternFunIndex,
    pub(super) fun_index: &'a HashMap<String, &'a hir::FunDecl>,
    pub(super) materialized_pass_view: Option<crate::mir::MaterializedMirPassView<'a>>,
    pub(super) published_late_lowered_program:
        Option<&'a crate::effect_lowered::LateLoweredProgram>,
    pub(super) program_facts: Rc<ProgramFacts>,
    pub(super) effect_op_tags: Rc<RefCell<EffectOpTagState>>,
}

pub(super) struct TypeDescriptorSpec<'ctx, 'a> {
    pub(super) at: crate::span::Span,
    pub(super) global_name: &'a str,
    pub(super) canonical_name: &'a str,
    pub(super) obj_ty: StructType<'ctx>,
    pub(super) trace_start_offset_bytes: u64,
    pub(super) parent: Option<GlobalValue<'ctx>>,
    pub(super) itable: Option<PointerValue<'ctx>>,
    pub(super) vtable: Option<PointerValue<'ctx>>,
}

impl<'a, 'ctx> CompilationUnitCodegenCx<'a, 'ctx> {
    pub(super) fn new(inputs: CompilationUnitCodegenInputs<'a, 'ctx>) -> Self {
        let CompilationUnitCodegenInputs {
            context,
            module,
            builder,
            target_data,
            host,
            source_map,
            entry_source_id,
            types,
            struct_layouts,
            enum_layouts,
            top_level_vars,
            top_level_consts,
            top_level_immutable_values,
            top_level_fun_call_sites,
            extern_globals,
            object_inits,
            class_inits,
            class_vtables,
            interfaces,
            class_itables,
            ctor_call_sites,
            dispatch_call_sites,
            effect_op_call_sites,
            continuation_resume_call_sites,
            when_pat_binding_tys,
            nominal_kinds,
            direct_supertypes,
            builtins,
            extern_funs,
            fun_index,
            materialized_pass_view,
            published_late_lowered_program,
            program_facts,
            effect_op_tags,
        } = inputs;
        let known_effect_instances_by_effect_fqn =
            collect_known_effect_instance_types_by_effect_fqn(types, nominal_kinds);
        Self {
            context,
            module,
            builder,
            target_data,
            host,
            source_map,
            entry_source_id,
            types,
            struct_layouts,
            enum_layouts,
            top_level_vars,
            top_level_consts,
            top_level_immutable_values,
            top_level_fun_call_sites,
            extern_globals,
            extern_funs,
            object_inits,
            class_inits,
            class_vtables,
            interfaces,
            class_itables,
            ctor_call_sites,
            dispatch_call_sites,
            effect_op_call_sites,
            continuation_resume_call_sites,
            when_pat_binding_tys,
            nominal_kinds,
            direct_supertypes,
            builtins,
            fun_index,
            materialized_pass_view,
            published_late_lowered_program,
            program_facts,
            shared_caches: SharedCodegenCaches::default(),
            effect_op_tags,
            known_effect_instances_by_effect_fqn,
        }
    }

    /// 为单个顶层函数、closure body、wrapper 或 init body 构造新的函数级 codegen。
    ///
    /// 新实例会重置函数级局部状态，但继续复用编译单元级共享输入与共享事实。
    pub(super) fn fresh_main_codegen(&'a self) -> MainCodegen<'a, 'ctx> {
        MainCodegen::new(self)
    }

    pub(super) fn materialized_pass_view(
        &self,
    ) -> Option<&crate::mir::MaterializedMirPassView<'a>> {
        self.materialized_pass_view.as_ref()
    }

    pub(super) fn published_late_lowered_program(
        &self,
    ) -> Option<&crate::effect_lowered::LateLoweredProgram> {
        self.published_late_lowered_program
    }
}

impl<'a, 'ctx> Deref for MainCodegen<'a, 'ctx> {
    type Target = CompilationUnitCodegenCx<'a, 'ctx>;

    fn deref(&self) -> &Self::Target {
        self.shared
    }
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(crate) fn declare_exported_abi_function(
        &self,
        name: &str,
        fn_ty: FunctionType<'ctx>,
    ) -> FunctionValue<'ctx> {
        declare_exported_abi_function(self.module, name, fn_ty)
    }

    pub(crate) fn declare_runtime_or_native_import_function(
        &self,
        name: &str,
        fn_ty: FunctionType<'ctx>,
    ) -> FunctionValue<'ctx> {
        declare_runtime_or_native_import_function(self.module, name, fn_ty)
    }

    pub(crate) fn declare_compiler_private_helper_function(
        &self,
        name: &str,
        fn_ty: FunctionType<'ctx>,
        linkage: Linkage,
    ) -> FunctionValue<'ctx> {
        declare_compiler_private_helper_function(self.module, name, fn_ty, linkage)
    }

    pub(super) fn enable_callable_carrier_contract(&self) {
        self.shared_caches
            .callable_carrier_contract_enabled
            .set(true);
    }

    fn callable_carrier_contract_enabled(&self) -> bool {
        self.shared_caches.callable_carrier_contract_enabled.get()
    }

    pub(super) fn register_callable_carrier_entry_symbol(
        &self,
        kind: CallableCarrierKind,
        callable_fqn: &str,
        symbol_name: &str,
    ) -> Result<(), LlvmEmitError> {
        let mut symbols = self
            .shared_caches
            .callable_carrier_entry_symbols
            .borrow_mut();
        let key = (kind, callable_fqn.to_string());
        if self
            .shared_caches
            .plain_callable_carrier_fallback_targets
            .borrow()
            .contains(&key)
        {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "callable carrier contract 同时把 {} `{}` 发布为 plain fallback 和 effect-step target",
                    kind.label(),
                    callable_fqn,
                ),
            });
        }
        if let Some(existing) = symbols.get(&key) {
            if existing == symbol_name {
                return Ok(());
            }
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "callable carrier contract 为 {} `{}` 重复发布了不同 target：已有 `{}`，新值 `{}`",
                    kind.label(),
                    callable_fqn,
                    existing,
                    symbol_name,
                ),
            });
        }
        symbols.insert(key, symbol_name.to_string());
        Ok(())
    }

    pub(super) fn register_plain_callable_carrier_fallback(
        &self,
        kind: CallableCarrierKind,
        callable_fqn: &str,
    ) -> Result<(), LlvmEmitError> {
        let key = (kind, callable_fqn.to_string());
        if self
            .shared_caches
            .callable_carrier_entry_symbols
            .borrow()
            .contains_key(&key)
        {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "callable carrier contract 同时把 {} `{}` 发布为 effect-step target 和 plain fallback",
                    kind.label(),
                    callable_fqn,
                ),
            });
        }
        self.shared_caches
            .plain_callable_carrier_fallback_targets
            .borrow_mut()
            .insert(key);
        Ok(())
    }

    fn callable_carrier_entry_symbol(
        &self,
        kind: CallableCarrierKind,
        callable_fqn: &str,
    ) -> Result<Option<String>, LlvmEmitError> {
        if let Some(symbol) = self
            .shared_caches
            .callable_carrier_entry_symbols
            .borrow()
            .get(&(kind, callable_fqn.to_string()))
            .cloned()
        {
            return Ok(Some(symbol));
        }
        if matches!(kind, CallableCarrierKind::ClosureObject)
            && is_direct_hir_closure_carrier_alias(callable_fqn)
        {
            return Ok(None);
        }
        if self.callable_carrier_contract_enabled() {
            if self
                .shared_caches
                .plain_callable_carrier_fallback_targets
                .borrow()
                .contains(&(kind, callable_fqn.to_string()))
            {
                return Ok(None);
            }
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "callable carrier contract 缺少 {} `{}` 的 published target entry",
                    kind.label(),
                    callable_fqn,
                ),
            });
        }
        Ok(None)
    }

    fn plain_callable_carrier_fallback_allowed(
        &self,
        kind: CallableCarrierKind,
        callable_fqn: &str,
    ) -> bool {
        self.shared_caches
            .plain_callable_carrier_fallback_targets
            .borrow()
            .contains(&(kind, callable_fqn.to_string()))
    }

    pub(super) fn callable_carrier_target_fn_ptr(
        &self,
        kind: CallableCarrierKind,
        callable_fqn: &str,
        fallback_target: PointerValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let Some(symbol_name) = self.callable_carrier_entry_symbol(kind, callable_fqn)? else {
            return Ok(fallback_target);
        };
        let function = self
            .module
            .get_function(&symbol_name)
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "refactor callable carrier contract 为 {} `{}` 发布了 target `{symbol_name}`，但 LLVM module 中缺少对应 function shell",
                    kind.label(),
                    callable_fqn,
                ),
            })?;
        Ok(function.as_global_value().as_pointer_value())
    }

    pub(crate) fn begin_function_explicit_frame_layout(
        &mut self,
        llvm_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let entry = llvm_fun
            .get_first_basic_block()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "function has no entry block",
                at: crate::span::Span::new(0, 0).into(),
            })?;
        let entry_builder = self.context.create_builder();
        match entry.get_first_instruction() {
            Some(inst) => entry_builder.position_before(&inst),
            None => entry_builder.position_at_end(entry),
        }

        let storage = entry_builder.build_array_alloca(
            self.llvm_ptr_type(AddressSpace::default()),
            self.context.i32_type().const_int(2, false),
            "explicit_root_frame_storage",
        )?;
        self.function_cx.explicit_frame_layout = ExplicitFrameLayoutPlan {
            function_symbol: Some(
                llvm_fun
                    .get_name()
                    .to_str()
                    .unwrap_or("anonymous")
                    .to_string(),
            ),
            frame_storage: Some(storage),
            slot_tys: Vec::new(),
        };
        Ok(())
    }

    pub(crate) fn finish_function_explicit_frame_layout(
        &mut self,
        at: crate::span::Span,
    ) -> Result<(), LlvmEmitError> {
        let plan = std::mem::take(&mut self.function_cx.explicit_frame_layout);
        let Some(ref function_symbol) = plan.function_symbol else {
            return Ok(());
        };

        let slot_count = plan.slot_tys.len();
        let frame_ty_name = explicit_root_frame_type_name(function_symbol);
        let frame_ty = self
            .context
            .get_struct_type(&frame_ty_name)
            .unwrap_or_else(|| self.context.opaque_struct_type(&frame_ty_name));
        let header_ty = self.llvm_explicit_root_frame_header_type();

        let gc_slot_ty = self.llvm_gc_i8_ptr_type();
        let mut field_tys: Vec<BasicTypeEnum<'ctx>> = Vec::with_capacity(1 + slot_count);
        field_tys.push(header_ty.into());
        field_tys.extend((0..slot_count).map(|_| BasicTypeEnum::PointerType(gc_slot_ty)));
        frame_ty.set_body(&field_tys, false);

        let ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let i32_ty = self.context.i32_type();
        let offset_ptr = if slot_count == 0 {
            ptr_ty.const_null()
        } else {
            let offset_global_name = explicit_root_frame_offsets_global_name(function_symbol);
            let offsets_gv = if let Some(existing) = self.module.get_global(&offset_global_name) {
                existing
            } else {
                let mut offsets = Vec::with_capacity(slot_count);
                for field_index in 0..slot_count {
                    let offset = self.explicit_root_frame_slot_offset_bytes(field_index)?;
                    offsets.push(i32_ty.const_int(offset, false));
                }

                let arr_ty = i32_ty.array_type(slot_count as u32);
                let gv = self.module.add_global(arr_ty, None, &offset_global_name);
                gv.set_initializer(&i32_ty.const_array(&offsets));
                gv.set_constant(true);
                gv.set_linkage(Linkage::Internal);
                gv
            };
            offsets_gv.as_pointer_value().const_cast(ptr_ty)
        };

        let desc_global_name = explicit_root_frame_desc_global_name(function_symbol);
        if self.module.get_global(&desc_global_name).is_none() {
            let desc_ty = self.llvm_explicit_root_frame_desc_type();
            let init = desc_ty.const_named_struct(&[
                i32_ty.const_int(slot_count as u64, false).into(),
                offset_ptr.into(),
            ]);
            let gv = self.module.add_global(desc_ty, None, &desc_global_name);
            gv.set_initializer(&init);
            gv.set_constant(true);
            gv.set_linkage(Linkage::Internal);
        }

        // 即使当前函数没有显式 GC leaf slots，也必须把 zero-slot frame 挂到 TLS：
        // verify-roots / moving GC 需要一个统一的 managed root source，不能退回到
        // 已不再作为普通托管函数真源的 stackmap 路径。
        self.finalize_function_explicit_frame_lifecycle(at, &plan, &desc_global_name)?;
        Ok(())
    }

    fn reserve_explicit_frame_leaf_slots_for_storage_type(
        &mut self,
        at: crate::span::Span,
        storage_ty: BasicTypeEnum<'ctx>,
    ) -> Result<Vec<PointerValue<'ctx>>, LlvmEmitError> {
        if self
            .function_cx
            .explicit_frame_layout
            .function_symbol
            .is_none()
        {
            return Ok(Vec::new());
        }

        let mut leaf_tys = Vec::new();
        self.collect_gc_ptr_leaf_pointer_types_in_basic_type(at, storage_ty, &mut leaf_tys)?;

        let Some(frame_storage) = self.function_cx.explicit_frame_layout.frame_storage else {
            return Ok(Vec::new());
        };

        let mut frame_slots = Vec::with_capacity(leaf_tys.len());
        for leaf_ty in leaf_tys {
            let slot_index = self.function_cx.explicit_frame_layout.slot_tys.len();
            self.function_cx
                .explicit_frame_layout
                .slot_tys
                .push(leaf_ty);
            frame_slots.push(self.explicit_root_frame_slot_pointer(
                at,
                frame_storage,
                slot_index,
                leaf_ty,
                &format!("explicit_root_frame_slot_{slot_index}"),
            )?);
        }
        Ok(frame_slots)
    }

    fn explicit_root_frame_header_size_bytes(&self) -> Result<u64, LlvmEmitError> {
        let header_ty = self.llvm_explicit_root_frame_header_type();
        let size = self.target_data.get_store_size(&header_ty);
        if size == 0 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "explicit root frame header size",
                at: crate::span::Span::new(0, 0).into(),
            });
        }
        Ok(size)
    }

    fn explicit_root_frame_slot_offset_bytes(
        &self,
        slot_index: usize,
    ) -> Result<u64, LlvmEmitError> {
        Ok(self.explicit_root_frame_header_size_bytes()?
            + (slot_index as u64 * self.target_layout().pointer_size.max(1)))
    }

    fn explicit_root_frame_slot_pointer(
        &self,
        at: crate::span::Span,
        frame_storage: PointerValue<'ctx>,
        slot_index: usize,
        _slot_ty: PointerType<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let entry = frame_storage
            .as_instruction_value()
            .and_then(|inst| inst.get_parent())
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "explicit root frame entry block",
                at: at.into(),
            })?;
        let builder = self.context.create_builder();
        let mut cursor = entry.get_first_instruction();
        while let Some(inst) = cursor {
            if inst.get_opcode() != inkwell::values::InstructionOpcode::Alloca {
                builder.position_before(&inst);
                break;
            }
            cursor = inst.get_next_instruction();
        }
        if cursor.is_none() {
            builder.position_at_end(entry);
        }

        let frame_i8 = builder.build_pointer_cast(
            frame_storage,
            self.llvm_i8_ptr_type(),
            &format!("{name}_base"),
        )?;
        let i64_ty = self.context.i64_type();
        let offset = self.explicit_root_frame_slot_offset_bytes(slot_index)?;
        let slot_addr = unsafe {
            builder.build_in_bounds_gep(
                self.context.i8_type(),
                frame_i8,
                &[i64_ty.const_int(offset, false)],
                name,
            )?
        };
        Ok(builder.build_pointer_cast(
            slot_addr,
            self.llvm_ptr_type(AddressSpace::default()),
            &format!("{name}_slot"),
        )?)
    }

    fn record_explicit_frame_slot_mirrors(
        &mut self,
        slot: PointerValue<'ctx>,
        frame_slots: Vec<PointerValue<'ctx>>,
    ) {
        if frame_slots.is_empty() {
            return;
        }
        self.function_cx
            .explicit_frame_slot_mirrors
            .insert(pointer_value_key(slot), frame_slots);
    }

    fn explicit_frame_slot_mirrors_for(
        &self,
        slot: PointerValue<'ctx>,
    ) -> Option<&[PointerValue<'ctx>]> {
        self.function_cx
            .explicit_frame_slot_mirrors
            .get(&pointer_value_key(slot))
            .map(Vec::as_slice)
    }

    fn explicit_frame_leaf_slot_pairs_for_storage_slot(
        &mut self,
        at: crate::span::Span,
        slot: PointerValue<'ctx>,
        value_ty: BasicTypeEnum<'ctx>,
        name_prefix: &str,
    ) -> Result<Vec<(PointerValue<'ctx>, PointerType<'ctx>, PointerValue<'ctx>)>, LlvmEmitError>
    {
        if self
            .function_cx
            .explicit_frame_layout
            .frame_storage
            .is_none()
        {
            return Ok(Vec::new());
        }

        let slot =
            self.rematerialize_ptr_in_current_block(at, slot, &format!("{name_prefix}_slot"))?;
        let Some(frame_slots) = self
            .explicit_frame_slot_mirrors_for(slot)
            .map(|slots| slots.to_vec())
        else {
            return Ok(Vec::new());
        };

        let mut gc_leaf_slots = Vec::new();
        self.collect_gc_ptr_leaf_slots_in_spill(slot, value_ty, name_prefix, &mut gc_leaf_slots)?;
        if frame_slots.len() != gc_leaf_slots.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "spill slot/frame slot count mismatch",
                at: at.into(),
            });
        }

        Ok(gc_leaf_slots
            .into_iter()
            .zip(frame_slots)
            .map(|((leaf_slot, value_ptr_ty), frame_slot)| (leaf_slot, value_ptr_ty, frame_slot))
            .collect())
    }

    /// 对单个 pointer-shaped GC 值，返回 post-safepoint 应优先 reload 的 explicit-frame home slot。
    ///
    /// aggregate / multi-leaf 值仍交给后续 refresh/rebuild contract 处理；这里先收紧 direct
    /// ref / string / niche-pointer 这类“单槽 GC 值”的 reload source-of-truth。
    fn explicit_frame_single_gc_ptr_reload_slot_for_storage_slot(
        &mut self,
        at: crate::span::Span,
        slot: PointerValue<'ctx>,
        value_ty: BasicTypeEnum<'ctx>,
        name_prefix: &str,
    ) -> Result<Option<PointerValue<'ctx>>, LlvmEmitError> {
        let BasicTypeEnum::PointerType(ptr_ty) = value_ty else {
            return Ok(None);
        };
        if ptr_ty.get_address_space() != self.gc_address_space() {
            return Ok(None);
        }

        let mut pairs =
            self.explicit_frame_leaf_slot_pairs_for_storage_slot(at, slot, value_ty, name_prefix)?;
        if pairs.is_empty() {
            return Ok(None);
        }
        if pairs.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "single gc ptr explicit frame reload slot",
                at: at.into(),
            });
        }

        let (_, _, frame_slot) = pairs.remove(0);
        Ok(Some(frame_slot))
    }

    fn rebuild_value_from_storage_slot_with_explicit_frame(
        &mut self,
        at: crate::span::Span,
        slot: PointerValue<'ctx>,
        value_ty: BasicTypeEnum<'ctx>,
        frame_slots: &[PointerValue<'ctx>],
        frame_index: &mut usize,
        name_prefix: &str,
    ) -> Result<BasicValueEnum<'ctx>, LlvmEmitError> {
        Ok(match value_ty {
            BasicTypeEnum::PointerType(ptr_ty) => {
                if ptr_ty.get_address_space() == self.gc_address_space() {
                    let frame_slot = frame_slots.get(*frame_index).copied().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "aggregate explicit frame rebuild slot",
                            at: at.into(),
                        },
                    )?;
                    *frame_index += 1;
                    self.builder.build_load(
                        ptr_ty,
                        frame_slot,
                        &format!("{name_prefix}_frame_reload"),
                    )?
                } else {
                    self.builder.build_load(
                        ptr_ty,
                        slot,
                        &format!("{name_prefix}_scalar_reload"),
                    )?
                }
            }
            BasicTypeEnum::StructType(struct_ty) => {
                if struct_ty.is_opaque() {
                    self.builder.build_load(
                        struct_ty,
                        slot,
                        &format!("{name_prefix}_opaque_reload"),
                    )?
                } else {
                    let mut agg = struct_ty.get_undef();
                    for (idx, field_ty) in struct_ty.get_field_types().into_iter().enumerate() {
                        let field_slot = self.builder.build_struct_gep(
                            struct_ty,
                            slot,
                            idx as u32,
                            &format!("{name_prefix}_field_gep_{idx}"),
                        )?;
                        let field = self.rebuild_value_from_storage_slot_with_explicit_frame(
                            at,
                            field_slot,
                            field_ty,
                            frame_slots,
                            frame_index,
                            name_prefix,
                        )?;
                        agg = self
                            .builder
                            .build_insert_value(
                                agg,
                                field,
                                idx as u32,
                                &format!("{name_prefix}_field_insert_{idx}"),
                            )?
                            .into_struct_value();
                    }
                    agg.into()
                }
            }
            BasicTypeEnum::ArrayType(array_ty) => {
                let mut agg = array_ty.get_undef();
                let i32_ty = self.context.i32_type();
                let zero = i32_ty.const_zero();
                for idx in 0..array_ty.len() {
                    let elem_slot = unsafe {
                        self.builder.build_in_bounds_gep(
                            array_ty,
                            slot,
                            &[zero, i32_ty.const_int(idx as u64, false)],
                            &format!("{name_prefix}_elem_gep_{idx}"),
                        )?
                    };
                    let elem = self.rebuild_value_from_storage_slot_with_explicit_frame(
                        at,
                        elem_slot,
                        array_ty.get_element_type(),
                        frame_slots,
                        frame_index,
                        name_prefix,
                    )?;
                    agg = self
                        .builder
                        .build_insert_value(
                            agg,
                            elem,
                            idx,
                            &format!("{name_prefix}_elem_insert_{idx}"),
                        )?
                        .into_array_value();
                }
                agg.into()
            }
            BasicTypeEnum::IntType(int_ty) => {
                self.builder
                    .build_load(int_ty, slot, &format!("{name_prefix}_int_reload"))?
            }
            BasicTypeEnum::FloatType(float_ty) => {
                self.builder
                    .build_load(float_ty, slot, &format!("{name_prefix}_float_reload"))?
            }
            BasicTypeEnum::VectorType(vector_ty) => {
                self.builder
                    .build_load(vector_ty, slot, &format!("{name_prefix}_vector_reload"))?
            }
            BasicTypeEnum::ScalableVectorType(vector_ty) => self.builder.build_load(
                vector_ty,
                slot,
                &format!("{name_prefix}_scalable_vector_reload"),
            )?,
        })
    }

    fn storage_slot_for_use(
        &mut self,
        at: crate::span::Span,
        slot: PointerValue<'ctx>,
        cg_ty: CgTy,
        name_prefix: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let slot = self.rematerialize_ptr_in_current_block(at, slot, name_prefix)?;
        let llvm_ty = self.llvm_basic_type_of(at, cg_ty)?;
        if let Some(frame_slot) = self.explicit_frame_single_gc_ptr_reload_slot_for_storage_slot(
            at,
            slot,
            llvm_ty,
            name_prefix,
        )? {
            return Ok(frame_slot);
        }
        if !self.basic_type_contains_gc_ptrs(at, llvm_ty)? {
            return Ok(slot);
        }
        let Some(frame_slots) = self
            .explicit_frame_slot_mirrors_for(slot)
            .map(|slots| slots.to_vec())
        else {
            return Ok(slot);
        };
        if frame_slots.is_empty() {
            return Ok(slot);
        }

        let scratch =
            self.create_entry_scratch_alloca_raw(at, &format!("{name_prefix}_rebuild"), llvm_ty)?;
        let mut frame_index = 0;
        let rebuilt = self.rebuild_value_from_storage_slot_with_explicit_frame(
            at,
            slot,
            llvm_ty,
            frame_slots.as_slice(),
            &mut frame_index,
            name_prefix,
        )?;
        if frame_index != frame_slots.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "aggregate explicit frame rebuild arity",
                at: at.into(),
            });
        }
        let _ = self.builder.build_store(scratch, rebuilt)?;
        self.apply_alloca_alignment_for_ty(at, scratch, cg_ty)?;
        Ok(scratch)
    }

    fn sync_storage_slot_into_explicit_frame(
        &mut self,
        at: crate::span::Span,
        slot: PointerValue<'ctx>,
        value_ty: BasicTypeEnum<'ctx>,
        name_prefix: &str,
    ) -> Result<(), LlvmEmitError> {
        for (leaf_slot, value_ptr_ty, frame_slot) in
            self.explicit_frame_leaf_slot_pairs_for_storage_slot(at, slot, value_ty, name_prefix)?
        {
            let loaded = self
                .builder
                .build_load(value_ptr_ty, leaf_slot, &format!("{name_prefix}_reload"))?
                .into_pointer_value();
            let _ = self.builder.build_store(frame_slot, loaded)?;
        }
        Ok(())
    }

    fn sync_basic_value_into_explicit_frame(
        &mut self,
        at: crate::span::Span,
        slot: PointerValue<'ctx>,
        raw: BasicValueEnum<'ctx>,
        value_ty: BasicTypeEnum<'ctx>,
        name_prefix: &str,
    ) -> Result<(), LlvmEmitError> {
        let slot =
            self.rematerialize_ptr_in_current_block(at, slot, &format!("{name_prefix}_slot"))?;
        let Some(frame_slots) = self
            .explicit_frame_slot_mirrors_for(slot)
            .map(|slots| slots.to_vec())
        else {
            return Ok(());
        };
        if frame_slots.is_empty() {
            return Ok(());
        }

        let mut leaves = Vec::new();
        if !self.collect_gc_ptr_leaf_values_in_basic_value(
            raw,
            value_ty,
            name_prefix,
            &mut leaves,
        )? {
            return self.sync_storage_slot_into_explicit_frame(at, slot, value_ty, name_prefix);
        }
        if leaves.len() != frame_slots.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "value/frame slot count mismatch",
                at: at.into(),
            });
        }
        for ((leaf, _leaf_ty), frame_slot) in leaves.into_iter().zip(frame_slots) {
            let ptr = leaf.into_pointer_value();
            let _ = self.builder.build_store(frame_slot, ptr)?;
        }
        Ok(())
    }

    fn collect_gc_ptr_leaf_values_in_basic_value(
        &mut self,
        raw: BasicValueEnum<'ctx>,
        value_ty: BasicTypeEnum<'ctx>,
        name_prefix: &str,
        out: &mut Vec<(BasicValueEnum<'ctx>, PointerType<'ctx>)>,
    ) -> Result<bool, LlvmEmitError> {
        match value_ty {
            BasicTypeEnum::PointerType(ptr_ty) => {
                if !matches!(raw, BasicValueEnum::PointerValue(_)) {
                    return Ok(false);
                }
                if ptr_ty.get_address_space() == self.gc_address_space() {
                    out.push((raw, ptr_ty));
                }
            }
            BasicTypeEnum::StructType(struct_ty) => {
                if struct_ty.is_opaque() {
                    return Ok(true);
                }
                let BasicValueEnum::StructValue(raw) = raw else {
                    return Ok(false);
                };
                for (idx, field_ty) in struct_ty.get_field_types().into_iter().enumerate() {
                    let field = self.builder.build_extract_value(
                        raw,
                        idx as u32,
                        &format!("{name_prefix}_leaf_value_{idx}"),
                    )?;
                    if !self.collect_gc_ptr_leaf_values_in_basic_value(
                        field,
                        field_ty,
                        name_prefix,
                        out,
                    )? {
                        return Ok(false);
                    }
                }
            }
            BasicTypeEnum::ArrayType(array_ty) => {
                let BasicValueEnum::ArrayValue(raw) = raw else {
                    return Ok(false);
                };
                for idx in 0..array_ty.len() {
                    let field = self.builder.build_extract_value(
                        raw,
                        idx,
                        &format!("{name_prefix}_leaf_array_value_{idx}"),
                    )?;
                    if !self.collect_gc_ptr_leaf_values_in_basic_value(
                        field,
                        array_ty.get_element_type(),
                        name_prefix,
                        out,
                    )? {
                        return Ok(false);
                    }
                }
            }
            BasicTypeEnum::IntType(_)
            | BasicTypeEnum::FloatType(_)
            | BasicTypeEnum::VectorType(_)
            | BasicTypeEnum::ScalableVectorType(_) => {}
        }
        Ok(true)
    }

    fn finalize_function_explicit_frame_lifecycle(
        &mut self,
        at: crate::span::Span,
        plan: &ExplicitFrameLayoutPlan<'ctx>,
        desc_global_name: &str,
    ) -> Result<(), LlvmEmitError> {
        let Some(frame_storage) = plan.frame_storage else {
            return Ok(());
        };
        let frame_storage_inst =
            frame_storage
                .as_instruction_value()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "explicit root frame storage alloca",
                    at: at.into(),
                })?;
        let total_words = self
            .context
            .i32_type()
            .const_int((2 + plan.slot_tys.len()) as u64, false);
        unsafe {
            llvm_sys::core::LLVMSetOperand(
                frame_storage_inst.as_value_ref(),
                0,
                total_words.as_value_ref(),
            );
        }

        let desc_global =
            self.module
                .get_global(desc_global_name)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "explicit root frame descriptor global",
                    at: at.into(),
                })?;
        self.emit_explicit_root_frame_entry_setup(
            at,
            frame_storage,
            plan.slot_tys.len(),
            desc_global,
        )?;
        self.emit_explicit_root_frame_return_pops(at, frame_storage, plan.slot_tys.as_slice())?;
        Ok(())
    }

    fn emit_explicit_root_frame_entry_setup(
        &self,
        at: crate::span::Span,
        frame_storage: PointerValue<'ctx>,
        slot_count: usize,
        desc_global: GlobalValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let frame_header_ty = self.llvm_explicit_root_frame_header_type();
        let insert_block = frame_storage
            .as_instruction_value()
            .and_then(|inst| inst.get_parent())
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "explicit root frame entry block",
                at: at.into(),
            })?;
        let builder = self.context.create_builder();
        let mut cursor = insert_block.get_first_instruction();
        while let Some(inst) = cursor {
            if inst.get_opcode() != inkwell::values::InstructionOpcode::Alloca {
                builder.position_before(&inst);
                break;
            }
            cursor = inst.get_next_instruction();
        }
        if cursor.is_none() {
            builder.position_at_end(insert_block);
        }

        let top_tls = self.declare_runtime_explicit_root_frame_top_tls();
        let frame_header = builder.build_pointer_cast(
            frame_storage,
            self.llvm_ptr_type(AddressSpace::default()),
            "explicit_root_frame_header",
        )?;
        let prev_ptr = builder.build_struct_gep(
            frame_header_ty,
            frame_header,
            0,
            "explicit_root_frame_prev_ptr",
        )?;
        let desc_ptr = builder.build_struct_gep(
            frame_header_ty,
            frame_header,
            1,
            "explicit_root_frame_desc_ptr",
        )?;
        let prev = builder.build_load(
            self.llvm_ptr_type(AddressSpace::default()),
            top_tls.as_pointer_value(),
            "explicit_root_frame_prev",
        )?;
        builder.build_store(prev_ptr, prev)?;
        builder.build_store(desc_ptr, desc_global.as_pointer_value())?;

        let null_gc = self.llvm_gc_i8_ptr_type().const_null();
        let frame_i8 = builder.build_pointer_cast(
            frame_storage,
            self.llvm_i8_ptr_type(),
            "explicit_root_frame_i8",
        )?;
        let i64_ty = self.context.i64_type();
        for slot_index in 0..slot_count {
            let offset = self.explicit_root_frame_slot_offset_bytes(slot_index)?;
            let slot_addr = unsafe {
                builder.build_in_bounds_gep(
                    self.context.i8_type(),
                    frame_i8,
                    &[i64_ty.const_int(offset, false)],
                    &format!("explicit_root_frame_init_slot_{slot_index}"),
                )?
            };
            let slot_ptr = builder.build_pointer_cast(
                slot_addr,
                self.llvm_ptr_type(AddressSpace::default()),
                &format!("explicit_root_frame_init_slot_ptr_{slot_index}"),
            )?;
            builder.build_store(slot_ptr, null_gc)?;
        }
        builder.build_store(top_tls.as_pointer_value(), frame_header)?;
        Ok(())
    }

    fn emit_explicit_root_frame_return_pops(
        &self,
        at: crate::span::Span,
        frame_storage: PointerValue<'ctx>,
        slot_tys: &[PointerType<'ctx>],
    ) -> Result<(), LlvmEmitError> {
        let frame_header_ty = self.llvm_explicit_root_frame_header_type();
        let function = frame_storage
            .as_instruction_value()
            .and_then(|inst| inst.get_parent())
            .and_then(|bb| bb.get_parent())
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "explicit root frame parent function",
                at: at.into(),
            })?;
        let top_tls = self.declare_runtime_explicit_root_frame_top_tls();
        let null_gc = self.llvm_gc_i8_ptr_type().const_null();

        for bb in function.get_basic_blocks() {
            let Some(term) = bb.get_terminator() else {
                continue;
            };
            let opcode = term.get_opcode();
            if opcode != inkwell::values::InstructionOpcode::Return
                && opcode != inkwell::values::InstructionOpcode::Unreachable
            {
                continue;
            }
            let builder = self.context.create_builder();
            builder.position_before(&term);
            let frame_header = builder.build_pointer_cast(
                frame_storage,
                self.llvm_ptr_type(AddressSpace::default()),
                "explicit_root_frame_pop_header",
            )?;
            let prev_ptr = builder.build_struct_gep(
                frame_header_ty,
                frame_header,
                0,
                "explicit_root_frame_pop_prev_ptr",
            )?;
            let prev = builder.build_load(
                self.llvm_ptr_type(AddressSpace::default()),
                prev_ptr,
                "explicit_root_frame_pop_prev",
            )?;
            let frame_i8 = builder.build_pointer_cast(
                frame_storage,
                self.llvm_i8_ptr_type(),
                "explicit_root_frame_pop_i8",
            )?;
            let i64_ty = self.context.i64_type();
            for (slot_index, _slot_ty) in slot_tys.iter().enumerate() {
                let offset = self.explicit_root_frame_slot_offset_bytes(slot_index)?;
                let slot_addr = unsafe {
                    builder.build_in_bounds_gep(
                        self.context.i8_type(),
                        frame_i8,
                        &[i64_ty.const_int(offset, false)],
                        &format!("explicit_root_frame_pop_slot_{slot_index}"),
                    )?
                };
                let slot_ptr = builder.build_pointer_cast(
                    slot_addr,
                    self.llvm_ptr_type(AddressSpace::default()),
                    &format!("explicit_root_frame_pop_slot_ptr_{slot_index}"),
                )?;
                builder.build_store(slot_ptr, null_gc)?;
            }
            builder.build_store(top_tls.as_pointer_value(), prev)?;
        }
        Ok(())
    }

    fn collect_gc_ptr_leaf_pointer_types_in_basic_type(
        &self,
        _at: crate::span::Span,
        ty: BasicTypeEnum<'ctx>,
        out: &mut Vec<PointerType<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        match ty {
            BasicTypeEnum::PointerType(ptr_ty) => {
                if ptr_ty.get_address_space() == self.gc_address_space() {
                    out.push(ptr_ty);
                }
            }
            BasicTypeEnum::StructType(st) => {
                if st.is_opaque() {
                    return Ok(());
                }
                for field_ty in st.get_field_types() {
                    self.collect_gc_ptr_leaf_pointer_types_in_basic_type(_at, field_ty, out)?;
                }
            }
            BasicTypeEnum::ArrayType(arr) => {
                for _ in 0..arr.len() {
                    self.collect_gc_ptr_leaf_pointer_types_in_basic_type(
                        _at,
                        arr.get_element_type(),
                        out,
                    )?;
                }
            }
            BasicTypeEnum::IntType(_)
            | BasicTypeEnum::FloatType(_)
            | BasicTypeEnum::VectorType(_)
            | BasicTypeEnum::ScalableVectorType(_) => {}
        }
        Ok(())
    }

    fn new(shared: &'a CompilationUnitCodegenCx<'a, 'ctx>) -> Self {
        Self {
            shared,
            current_source_id: shared.entry_source_id,
            function_cx: FunctionBodyCodegenCx::default(),
            effect_cx: EffectLoweringCodegenCx::default(),
        }
    }

    /// 统一 nested/wrapper codegen 的构造路径，避免再次手写整套编译单元输入拼装。
    fn fresh_child_codegen(&self) -> Self {
        Self::new(self.shared)
    }

    fn take_function_body_cx(&mut self) -> FunctionBodyCodegenCx<'ctx> {
        std::mem::take(&mut self.function_cx)
    }

    fn restore_function_body_cx(&mut self, function_cx: FunctionBodyCodegenCx<'ctx>) {
        self.function_cx = function_cx;
    }

    fn take_suspend_site_explicit_effect_outcome(
        &mut self,
        site_id: u32,
    ) -> Option<PointerValue<'ctx>> {
        self.effect_cx
            .suspend_site_effect_outcomes
            .explicit_outcomes
            .remove(&site_id)
    }

    /// 在某段 lowering 内临时安装 ordinary callee suspend/replay 状态。
    fn with_callee_suspend_lowering<T, F>(
        &mut self,
        current_suspend_plan: Option<CalleeSuspendPlan>,
        current_resume_entry_fn: Option<FunctionValue<'ctx>>,
        f: F,
    ) -> Result<T, LlvmEmitError>
    where
        F: FnOnce(&mut Self) -> Result<T, LlvmEmitError>,
    {
        let saved_callee_suspend = std::mem::take(&mut self.effect_cx.callee_suspend);
        self.effect_cx.callee_suspend = CalleeSuspendLoweringCodegenCx {
            current_suspend_plan,
            current_resume_entry_fn,
        };
        let result = f(self);
        self.effect_cx.callee_suspend = saved_callee_suspend;
        result
    }

    fn with_active_suspend_site_any_effect_outcome_capture<T, F>(
        &mut self,
        site_id: u32,
        f: F,
    ) -> Result<T, LlvmEmitError>
    where
        F: FnOnce(&mut Self) -> Result<T, LlvmEmitError>,
    {
        let saved_capture = self.effect_cx.suspend_site_effect_outcomes.active_capture;
        self.effect_cx.suspend_site_effect_outcomes.active_capture =
            Some(ActiveSuspendSiteEffectOutcomeCapture {
                site_id,
                call_span: crate::span::Span::new(0, 0),
                capture_any: true,
            });
        self.effect_cx
            .suspend_site_effect_outcomes
            .explicit_outcomes
            .remove(&site_id);
        let result = f(self);
        self.effect_cx.suspend_site_effect_outcomes.active_capture = saved_capture;
        result
    }

    fn with_ordinary_effect_propagation_suppressed<T, F>(
        &mut self,
        f: F,
    ) -> Result<T, LlvmEmitError>
    where
        F: FnOnce(&mut Self) -> Result<T, LlvmEmitError>,
    {
        let saved_return_ty = self.function_cx.current_fun_return_ty.take();
        let result = f(self);
        self.function_cx.current_fun_return_ty = saved_return_ty;
        result
    }

    fn current_local_effect_escape_target(&self) -> Option<inkwell::basic_block::BasicBlock<'ctx>> {
        self.function_cx.local_effect_escape_targets.last().copied()
    }

    fn with_local_effect_escape_target<T, F>(
        &mut self,
        target: inkwell::basic_block::BasicBlock<'ctx>,
        f: F,
    ) -> Result<T, LlvmEmitError>
    where
        F: FnOnce(&mut Self) -> Result<T, LlvmEmitError>,
    {
        self.function_cx.local_effect_escape_targets.push(target);
        let result = f(self);
        let _ = self.function_cx.local_effect_escape_targets.pop();
        result
    }

    fn when_pat_binding_hir_ty(
        &self,
        span: crate::span::Span,
    ) -> Result<Option<TypeId>, LlvmEmitError> {
        let source = self.current_source()?;
        Ok(self
            .when_pat_binding_tys
            .get(&hir::WhenPatBindingSite {
                source_path: source.path().to_path_buf(),
                decl_span: span,
            })
            .copied())
    }

    fn current_source(&self) -> Result<&SourceFile, LlvmEmitError> {
        self.source_map
            .source(self.current_source_id)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "current source lookup",
                at: crate::span::Span::new(0, 0).into(),
            })
    }

    fn current_call_site(&self, span: crate::span::Span) -> Result<hir::CallSite, LlvmEmitError> {
        let source = self.current_source()?;
        Ok(hir::CallSite::new(source.path().to_path_buf(), span))
    }

    fn current_top_level_fun_call_binding(
        &self,
        span: crate::span::Span,
    ) -> Result<Option<&ast::TopLevelFunCallBinding>, LlvmEmitError> {
        let call_site = self.current_call_site(span)?;
        Ok(self.top_level_fun_call_sites.get(&call_site))
    }

    fn concrete_top_level_fun_call_fqn(
        &self,
        span: crate::span::Span,
        fallback_fqn: &str,
    ) -> Result<String, LlvmEmitError> {
        fn callable_dispatch_base_fqn(fqn: &str) -> &str {
            let base = fqn.rsplit_once("::<").map(|(base, _)| base).unwrap_or(fqn);
            base.split_once("$overload$")
                .map(|(base, _)| base)
                .unwrap_or(base)
        }

        fn callable_fqn_specificity(fqn: &str) -> u8 {
            u8::from(fqn.contains("$overload$")) + u8::from(fqn.contains("::<"))
        }

        fn type_contains_param(types: &TypeStore, ty: TypeId) -> bool {
            let mut stack = vec![ty];
            while let Some(id) = stack.pop() {
                match types.kind(id) {
                    TypeKind::Param(_) => return true,
                    TypeKind::StarProjection(star) => stack.push(star.read_ty),
                    TypeKind::Ref(RefTypeKind::Nominal(nominal))
                    | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                        stack.extend(nominal.args.iter().copied());
                        if let Some(eff) = &nominal.eff {
                            stack.extend(eff.terms.iter().copied());
                        }
                    }
                    TypeKind::Ref(RefTypeKind::Function(fun)) => {
                        if let Some(receiver) = fun.receiver {
                            stack.push(receiver);
                        }
                        stack.extend(fun.params.iter().copied());
                        stack.push(fun.return_ty);
                        stack.extend(fun.effects.terms.iter().copied());
                    }
                    TypeKind::Ref(RefTypeKind::Union(union)) => {
                        stack.extend(union.variants.iter().copied());
                    }
                    TypeKind::Value(ValueTypeKind::Option(inner)) => stack.push(*inner),
                    TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
                        stack.extend(elements.iter().copied());
                    }
                    TypeKind::Ref(RefTypeKind::Any | RefTypeKind::String)
                    | TypeKind::Value(
                        ValueTypeKind::Unit
                        | ValueTypeKind::Nothing
                        | ValueTypeKind::Bool
                        | ValueTypeKind::Char
                        | ValueTypeKind::Float64
                        | ValueTypeKind::Float32
                        | ValueTypeKind::Int
                        | ValueTypeKind::UInt
                        | ValueTypeKind::IntN(_)
                        | ValueTypeKind::UIntN(_),
                    ) => {}
                }
            }
            false
        }

        let Some(binding) = self.current_top_level_fun_call_binding(span)? else {
            return Ok(fallback_fqn.to_string());
        };
        let binding_fqn = if binding.type_args.is_empty() {
            binding.fqn.clone()
        } else {
            let args = binding
                .type_args
                .iter()
                .map(|ty| self.types.display(*ty).to_string())
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}::<{}>", binding.fqn, args)
        };
        let binding_contains_unresolved_params = binding
            .type_args
            .iter()
            .any(|&ty| type_contains_param(self.types, ty))
            || binding.eff_args.iter().any(|row| {
                row.terms
                    .iter()
                    .any(|&ty| type_contains_param(self.types, ty))
            });
        if binding_contains_unresolved_params && callable_fqn_specificity(fallback_fqn) > 0 {
            return Ok(fallback_fqn.to_string());
        }
        let fallback_base = callable_dispatch_base_fqn(fallback_fqn);
        let binding_base = callable_dispatch_base_fqn(&binding_fqn);
        if binding_base == fallback_base
            && callable_fqn_specificity(binding_fqn.as_str())
                < callable_fqn_specificity(fallback_fqn)
        {
            return Ok(fallback_fqn.to_string());
        }
        if binding_base != fallback_base && callable_fqn_specificity(fallback_fqn) > 0 {
            return Ok(fallback_fqn.to_string());
        }
        Ok(binding_fqn)
    }

    fn source_id_for_path(
        &self,
        path: &Path,
        at: crate::span::Span,
    ) -> Result<SourceId, LlvmEmitError> {
        self.source_map
            .source_id_of_path(path)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "source file lookup",
                at: at.into(),
            })
    }

    fn top_level_value_ty(&self, fqn: &str) -> Option<TypeId> {
        self.top_level_vars
            .get(fqn)
            .map(|var| var.ty)
            .or_else(|| {
                self.materialized_extern_global_root(fqn)
                    .map(|global| global.ty)
            })
            .or_else(|| self.extern_globals.get(fqn).map(|global| global.ty))
            .or_else(|| self.top_level_consts.get(fqn).map(|value| value.ty))
            .or_else(|| {
                self.top_level_immutable_values
                    .get(fqn)
                    .map(|value| value.ty)
            })
    }

    fn materialized_extern_global_root(&self, fqn: &str) -> Option<&crate::mir::ExternGlobalRoot> {
        self.materialized_pass_view()?
            .materialized()
            .file
            .items
            .iter()
            .find_map(|item| match item {
                crate::mir::Item::ExternGlobal(root) if root.fqn == fqn => Some(root),
                _ => None,
            })
    }

    fn has_extern_global_contract(&self, fqn: &str) -> bool {
        self.materialized_extern_global_root(fqn).is_some() || self.extern_globals.contains_key(fqn)
    }

    fn source_slice_at(
        &self,
        source_id: SourceId,
        span: crate::span::Span,
    ) -> Result<&str, LlvmEmitError> {
        let bound = self.source_map.bind_span(source_id, span).map_err(|_| {
            LlvmEmitError::UnsupportedMainBody {
                kind: "source-backed literal span",
                at: span.into(),
            }
        })?;
        self.source_map
            .slice(bound)
            .map_err(|_| LlvmEmitError::UnsupportedMainBody {
                kind: "source-backed literal slice",
                at: span.into(),
            })
    }

    fn current_source_slice(&self, span: crate::span::Span) -> Result<&str, LlvmEmitError> {
        self.source_slice_at(self.current_source_id, span)
    }

    fn parse_current_int_literal(&self, span: crate::span::Span) -> Result<u128, LlvmEmitError> {
        Ok(parse_int_literal(self.current_source_slice(span)?))
    }

    fn int_literal_bits_for_ty(
        &self,
        span: crate::span::Span,
        int_ty: IntTy,
    ) -> Result<u64, LlvmEmitError> {
        let source = self.current_source()?;
        let text = self.current_source_slice(span)?;
        let raw = self.parse_current_int_literal(span)?;
        let bits = checked_positive_int_literal_bits(raw, int_ty).ok_or_else(|| {
            LlvmEmitError::invalid_literal(
                source,
                span,
                "integer literal",
                "超出目标整数类型可表示范围",
                text,
            )
        })?;
        Ok(bits as u64)
    }

    fn int_literal_bits_from_text_for_ty(
        &self,
        span: crate::span::Span,
        text: &str,
        int_ty: IntTy,
    ) -> Result<u64, LlvmEmitError> {
        let source = self.current_source()?;
        let raw = parse_int_literal_checked(text).map_err(|err| {
            LlvmEmitError::invalid_literal(source, span, "integer literal", err.reason(), text)
        })?;
        let bits = checked_positive_int_literal_bits(raw, int_ty).ok_or_else(|| {
            LlvmEmitError::invalid_literal(
                source,
                span,
                "integer literal",
                "超出目标整数类型可表示范围",
                text,
            )
        })?;
        Ok(bits as u64)
    }

    fn negated_int_literal_bits_for_ty(
        &self,
        span: crate::span::Span,
        literal_span: crate::span::Span,
        int_ty: IntTy,
    ) -> Result<u64, LlvmEmitError> {
        let source = self.current_source()?;
        let text = self.current_source_slice(span)?;
        let raw = self.parse_current_int_literal(literal_span)?;
        let bits = checked_negated_int_literal_bits(raw, int_ty).ok_or_else(|| {
            LlvmEmitError::invalid_literal(
                source,
                span,
                "integer literal",
                "超出目标整数类型可表示范围",
                text,
            )
        })?;
        Ok(bits as u64)
    }

    fn int_literal_bits_from_source_span_if_present(
        &self,
        span: crate::span::Span,
        int_ty: IntTy,
    ) -> Result<Option<u64>, LlvmEmitError> {
        let Ok(text) = self.current_source_slice(span) else {
            return Ok(None);
        };
        let source = self.current_source()?;
        let Some((negative, body)) = source_text_int_literal_body(text) else {
            return Ok(None);
        };
        let raw = parse_int_literal_checked(body).map_err(|err| {
            LlvmEmitError::invalid_literal(source, span, "integer literal", err.reason(), text)
        })?;
        let bits = if negative {
            checked_negated_int_literal_bits(raw, int_ty)
        } else {
            checked_positive_int_literal_bits(raw, int_ty)
        }
        .ok_or_else(|| {
            LlvmEmitError::invalid_literal(
                source,
                span,
                "integer literal",
                "超出目标整数类型可表示范围",
                text,
            )
        })?;
        Ok(Some(bits as u64))
    }

    fn parse_current_string_literal_bytes(
        &self,
        span: crate::span::Span,
    ) -> Result<Vec<u8>, LlvmEmitError> {
        let text = self.current_source_slice(span)?;
        let source = self.current_source()?;
        parse_string_literal_bytes(text).map_err(|err| {
            LlvmEmitError::invalid_literal(
                source,
                span,
                "string literal",
                string_literal_parse_reason(err),
                text,
            )
        })
    }

    /// 获取 effect operation 的稳定 op_tag（T1608）。
    ///
    /// 规则：
    /// - `scoop.core.Raise.raise` → 1（固定；与 runtime 约定兼容）。
    /// - 其余 effect op：首次出现时分配递增编号（从 2 开始），后续查表复用。
    /// - 同一编译单元内 tag 稳定（相同 FQN 总是得到相同 tag）。
    pub(super) fn effect_op_tag(&mut self, fqn: &str) -> u32 {
        let mut state = self.effect_op_tags.borrow_mut();
        if let Some(&tag) = state.map.get(fqn) {
            return tag;
        }
        let tag = state.next;
        state.next = state.next.saturating_add(1);
        state.map.insert(fqn.to_string(), tag);
        tag
    }

    fn effect_nominal(&self, ty: TypeId) -> Option<&NominalType> {
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.types.kind(ty) else {
            return None;
        };
        if !matches!(
            self.nominal_kinds.get(&nominal.fqn),
            Some(ast::TypeKind::Effect)
        ) {
            return None;
        }
        Some(nominal)
    }

    fn is_runtime_error_type(&self, ty: TypeId) -> bool {
        match self.types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
            | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                nominal.fqn == "scoop.core.RuntimeError"
            }
            _ => false,
        }
    }

    fn is_raise_runtime_error_effect(&self, effect_ty: TypeId) -> bool {
        let Some(nominal) = self.effect_nominal(effect_ty) else {
            return false;
        };
        nominal.fqn == "scoop.core.Raise"
            && nominal.args.len() == 1
            && self.is_runtime_error_type(nominal.args[0])
    }

    fn known_effect_instance_types_for_fqn(&self, effect_fqn: &str) -> Vec<TypeId> {
        let mut ids = self
            .known_effect_instances_by_effect_fqn
            .get(effect_fqn)
            .cloned()
            .unwrap_or_default();

        ids.extend(self.types.iter_ids().filter(|type_id| {
            let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.types.kind(*type_id) else {
                return false;
            };
            nominal.fqn == effect_fqn
                && matches!(
                    self.nominal_kinds.get(&nominal.fqn),
                    Some(ast::TypeKind::Effect)
                )
        }));

        ids.sort_by(|lhs, rhs| {
            let lhs_display = self.types.display(*lhs).to_string();
            let rhs_display = self.types.display(*rhs).to_string();
            lhs_display.cmp(&rhs_display).then_with(|| lhs.cmp(rhs))
        });
        ids.dedup();
        ids
    }

    pub(super) fn effect_instance_key(&self, effect_ty: TypeId) -> Option<u32> {
        if self.is_raise_runtime_error_effect(effect_ty) {
            return Some(EFFECT_INSTANCE_KEY_RAISE_RUNTIME_ERROR);
        }

        let nominal = self.effect_nominal(effect_ty)?;
        self.known_effect_instance_types_for_fqn(&nominal.fqn)
            .iter()
            .position(|candidate| *candidate == effect_ty)
            .and_then(|index| u32::try_from(index).ok())
    }

    #[allow(dead_code)]
    pub(super) fn effect_instance_key_for_family(
        &self,
        family: &crate::effect_facts::EffectFamilyKey,
    ) -> Option<u32> {
        if family.effect_fqn() == "scoop.core.Raise"
            && family.type_args().len() == 1
            && self.is_runtime_error_type(family.type_args()[0])
        {
            return Some(EFFECT_INSTANCE_KEY_RAISE_RUNTIME_ERROR);
        }

        self.known_effect_instance_types_for_fqn(family.effect_fqn())
            .iter()
            .position(|candidate| {
                self.effect_nominal(*candidate)
                    .is_some_and(|nominal| nominal.args.as_slice() == family.type_args())
            })
            .and_then(|index| u32::try_from(index).ok())
    }

    fn raise_runtime_error_effect_ty(&self) -> Option<TypeId> {
        self.known_effect_instance_types_for_fqn("scoop.core.Raise")
            .into_iter()
            .find(|type_id| self.is_raise_runtime_error_effect(*type_id))
    }

    fn box_composite_effect_transport_value(
        &mut self,
        at: crate::span::Span,
        source_ty: TypeId,
        source: CgValue<'ctx>,
        label: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let deferred_source =
            self.defer_gc_sensitive_cg_value(at, &format!("{label}_source"), source)?;
        let box_obj_ty = self.mir_value_box_object_type(at, source_ty, source.ty)?;
        let obj_size_bytes = self.target_data.get_store_size(&box_obj_ty);
        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);
        let box_desc =
            self.get_or_create_mir_value_box_type_desc_global(at, source_ty, box_obj_ty)?;
        let box_desc_i8 = self.builder.build_pointer_cast(
            box_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            &format!("{label}_desc_i8"),
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            at,
            rt_alloc,
            &[box_desc_i8.into(), size_v.into()],
            &format!("rt_alloc_{label}"),
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect transport value box return value",
                at: at.into(),
            })?;
        let BasicValueEnum::PointerValue(obj_i8) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect transport value box return type",
                at: at.into(),
            });
        };

        let obj_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let obj_ptr =
            self.builder
                .build_pointer_cast(obj_i8, obj_ptr_ty, &format!("{label}_obj_ptr"))?;
        let deferred_obj = self.defer_gc_ref_pointer(at, &format!("{label}_obj_root"), obj_ptr)?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            at,
            &format!("{label}_obj_reload"),
            &deferred_obj,
        )?;
        let payload_gep = self.builder.build_struct_gep(
            box_obj_ty,
            obj_ptr,
            1,
            &format!("{label}_payload_gep"),
        )?;
        let payload = self.materialize_deferred_cg_value(
            at,
            &format!("{label}_source_reload"),
            deferred_source,
        )?;
        let _ = self.store_local_value(at, payload_gep, source.ty, payload)?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            at,
            &format!("{label}_return"),
            &deferred_obj,
        )?;
        let gc_ref = self.builder.build_pointer_cast(
            obj_ptr,
            self.llvm_gc_i8_ptr_type(),
            &format!("{label}_gc_ref"),
        )?;
        self.clear_deferred_cg_value_root_homes(
            at,
            &format!("{label}_obj_root_drop"),
            &deferred_obj,
        )?;
        Ok(gc_ref)
    }

    fn emit_raise_runtime_error_variant(
        &mut self,
        span: crate::span::Span,
        variant_name: &str,
    ) -> Result<(), LlvmEmitError> {
        let outcome_ptr = self.function_cx.current_effect_outcome_ptr.ok_or_else(|| {
            LlvmEmitError::Frontend {
                message: format!(
                    "direct runtime-error raise `{variant_name}` 缺少当前 explicit EffectOutcome 槽位；该路径应由 published late-lowered/local-effect-control handoff 接管"
                ),
            }
        })?;
        let raise_runtime_error_effect = self.raise_runtime_error_effect_ty().ok_or_else(|| {
            LlvmEmitError::Frontend {
                message: "缺少 Raise<RuntimeError> effect type；HIR/MIR runtime-error lowering contract 未闭合"
                    .to_string(),
            }
        })?;
        let effect_instance_key = self
            .effect_instance_key(raise_runtime_error_effect)
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: "Raise<RuntimeError> effect instance key 未发布到 active codegen contract"
                    .to_string(),
            })?;
        let variant_fqn = format!("scoop.core.RuntimeError.{variant_name}");
        let payload_value = self
            .try_codegen_qualified_enum_unit_variant_value(span, &variant_fqn)?
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "RuntimeError unit variant value",
                at: span.into(),
            })?;
        let CgTy::Enum(payload_ty) = payload_value.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "RuntimeError payload cg type",
                at: span.into(),
            });
        };
        let payload_gc_ref = self.box_composite_effect_transport_value(
            span,
            payload_ty,
            payload_value,
            "raise_runtime_error_payload",
        )?;
        let zero_transport = effect_outcome::ValueTransportParts {
            word: self.context.i64_type().const_zero(),
            gc_ref: self.llvm_gc_i8_ptr_type().const_null(),
        };
        let raise_op_tag = self.effect_op_tag("scoop.core.Raise.raise");
        let signal = self.build_effect_signal(
            self.context
                .i32_type()
                .const_int(u64::from(raise_op_tag), false),
            self.context
                .i32_type()
                .const_int(u64::from(effect_instance_key), false),
            effect_outcome::ValueTransportParts {
                word: self.context.i64_type().const_zero(),
                gc_ref: payload_gc_ref,
            },
            self.llvm_gc_i8_ptr_type().const_null(),
        )?;
        let outcome = self.build_effect_outcome(
            effect_outcome::EffectOutcomeTag::Propagate,
            zero_transport,
            signal,
        )?;
        self.builder.build_store(outcome_ptr, outcome)?;
        Ok(())
    }

    pub(crate) fn declare_top_level_fun(
        &mut self,
        fun: &hir::FunDecl,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let llvm_name = self
            .extern_funs
            .get(&fun.fqn)
            .map(|e| e.symbol.as_str())
            .unwrap_or(fun.fqn.as_str());
        self.declare_top_level_fun_with_symbol(fun, llvm_name)
    }

    pub(crate) fn declare_top_level_fun_with_symbol(
        &mut self,
        fun: &hir::FunDecl,
        llvm_name: &str,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        if let Some(existing) = self.module.get_function(llvm_name) {
            return Ok(existing);
        }

        let is_extern = self.extern_funs.contains_key(&fun.fqn);

        // `@Extern` 调用点会在进入 native 前把 managed roots 暴露为 `native_roots` slots；
        // 从 LLVM GC/statepoint 的视角看，这些调用必须视作 leaf：
        // - native 内部即使触发 GC，也应以 slots 更新为准；
        // - 不能再依赖 caller frame 上的 SSA `gc.relocate` / stackmap 结果。
        //
        // 历史上我们还会把“返回 GC-free aggregate 的普通函数”标成 leaf，以绕开
        // `gc.result` 不能承载多寄存器 aggregate 的 LLVM 限制；但现在 ordinary path
        // 已统一把 aggregate return 改成 hidden sret，这类函数不应再被视作 leaf，
        // 否则它们内部的 managed calls 会被错误跳过 statepoint rewrite。
        let returns_gc_free_aggregate = self.returns_gc_free_aggregate(fun.return_ty);

        let Some(return_cg) = self.cg_ty_of(fun.return_ty) else {
            tracing::warn!(
                "declare_top_level_fun: unsupported return type for {} -> {}",
                fun.fqn,
                self.types.display(fun.return_ty)
            );
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "function return type",
                at: fun.span.into(),
            });
        };

        let hidden_sret_result_ty = if is_extern {
            None
        } else {
            self.hidden_sret_result_ty(fun.span, return_cg)?
        };
        let uses_explicit_effect_hidden_abi =
            !is_extern && self.callable_uses_explicit_effect_hidden_abi(&fun.fqn);
        let is_gc_leaf =
            is_extern || (returns_gc_free_aggregate && hidden_sret_result_ty.is_none());

        let mut llvm_params = Vec::with_capacity(
            fun.params.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + self.explicit_effect_hidden_abi_param_count(uses_explicit_effect_hidden_abi)
                    as usize,
        );
        if hidden_sret_result_ty.is_some() {
            llvm_params.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        if uses_explicit_effect_hidden_abi {
            self.push_explicit_effect_hidden_abi_param_tys(&mut llvm_params);
        }
        for param in &fun.params {
            let llvm_param_ty = if is_extern {
                self.llvm_param_ty(param.span, param.ty)
            } else {
                self.ordinary_param_abi(param.span, param.ty)
                    .map(OrdinaryParamAbi::llvm_param_ty)
            };
            match llvm_param_ty {
                Ok(ty) => llvm_params.push(ty),
                Err(err) => {
                    tracing::warn!(
                        "declare_top_level_fun: unsupported param type for {} param {} -> {}",
                        fun.fqn,
                        param.name,
                        self.types.display(param.ty)
                    );
                    return Err(err);
                }
            }
        }

        let fn_ty = match (hidden_sret_result_ty, return_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_params, false)
            }
            (None, other) => self
                .llvm_basic_type_of(fun.span, other)?
                .fn_type(&llvm_params, false),
        };

        let llvm_fun = if is_extern {
            self.declare_runtime_or_native_import_function(llvm_name, fn_ty)
        } else {
            self.declare_exported_abi_function(llvm_name, fn_ty)
        };
        // `@CallingConvention(...)`：缺省为 C ABI（LLVM callconv 0）。
        llvm_fun.set_call_conventions(self.llvm_call_convention_for_fqn(&fun.fqn));
        if let Some(result_ty) = hidden_sret_result_ty {
            self.add_sret_attribute_to_function(llvm_fun, 0, result_ty);
        }
        if is_gc_leaf {
            self.mark_gc_leaf_function(llvm_fun);
        }
        Ok(llvm_fun)
    }

    pub(crate) fn declare_top_level_fun_with_signature_override(
        &mut self,
        fun: &hir::FunDecl,
        llvm_name: &str,
        param_tys: &[TypeId],
        return_ty: TypeId,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        if let Some(existing) = self.module.get_function(llvm_name) {
            return Ok(existing);
        }

        let is_extern = self.extern_funs.contains_key(&fun.fqn);
        let returns_gc_free_aggregate = self.returns_gc_free_aggregate(return_ty);

        let Some(return_cg) = self.cg_ty_of(return_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "function return type",
                at: fun.span.into(),
            });
        };
        if param_tys.len() != fun.params.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "function param type",
                at: fun.span.into(),
            });
        }

        let hidden_sret_result_ty = if is_extern {
            None
        } else {
            self.hidden_sret_result_ty(fun.span, return_cg)?
        };
        let uses_explicit_effect_hidden_abi =
            !is_extern && self.callable_uses_explicit_effect_hidden_abi(&fun.fqn);
        let is_gc_leaf =
            is_extern || (returns_gc_free_aggregate && hidden_sret_result_ty.is_none());

        let mut llvm_params = Vec::with_capacity(
            param_tys.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + self.explicit_effect_hidden_abi_param_count(uses_explicit_effect_hidden_abi)
                    as usize,
        );
        if hidden_sret_result_ty.is_some() {
            llvm_params.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        if uses_explicit_effect_hidden_abi {
            self.push_explicit_effect_hidden_abi_param_tys(&mut llvm_params);
        }
        for (param, param_ty) in fun.params.iter().zip(param_tys.iter().copied()) {
            let llvm_param_ty = if is_extern {
                self.llvm_param_ty(param.span, param_ty)
            } else {
                self.ordinary_param_abi(param.span, param_ty)
                    .map(OrdinaryParamAbi::llvm_param_ty)
            }?;
            llvm_params.push(llvm_param_ty);
        }

        let fn_ty = match (hidden_sret_result_ty, return_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_params, false)
            }
            (None, other) => self
                .llvm_basic_type_of(fun.span, other)?
                .fn_type(&llvm_params, false),
        };

        let llvm_fun = if is_extern {
            self.declare_runtime_or_native_import_function(llvm_name, fn_ty)
        } else {
            self.declare_exported_abi_function(llvm_name, fn_ty)
        };
        llvm_fun.set_call_conventions(self.llvm_call_convention_for_fqn(&fun.fqn));
        if let Some(result_ty) = hidden_sret_result_ty {
            self.add_sret_attribute_to_function(llvm_fun, 0, result_ty);
        }
        if is_gc_leaf {
            self.mark_gc_leaf_function(llvm_fun);
        }
        Ok(llvm_fun)
    }

    pub(crate) fn declare_materialized_top_level_fun_with_symbol(
        &mut self,
        fun: &crate::mir::FunDecl,
        llvm_name: &str,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        if let Some(existing) = self.module.get_function(llvm_name) {
            return Ok(existing);
        }

        let mir_types = self
            .materialized_pass_view()
            .map(|view| &view.materialized().types)
            .ok_or(LlvmEmitError::MissingMaterializedPassView)?;
        let is_extern = self.extern_funs.contains_key(&fun.fqn);
        let codegen_return_ty = self
            .equivalent_codegen_type_id(mir_types, fun.return_ty)
            .unwrap_or(fun.return_ty);
        let returns_gc_free_aggregate = self.returns_gc_free_aggregate(codegen_return_ty);

        let Some(return_cg) = self
            .cg_ty_of_mir_type(mir_types, fun.return_ty)
            .or_else(|| self.cg_ty_of(codegen_return_ty))
        else {
            tracing::warn!(
                "declare_materialized_top_level_fun: unsupported return type for {} -> {}",
                fun.fqn,
                mir_types.display(fun.return_ty)
            );
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "function return type",
                at: fun.span.into(),
            });
        };

        let hidden_sret_result_ty = if is_extern {
            None
        } else {
            self.hidden_sret_result_ty(fun.span, return_cg)?
        };
        let uses_explicit_effect_hidden_abi =
            !is_extern && self.callable_uses_explicit_effect_hidden_abi(&fun.fqn);
        let is_gc_leaf =
            is_extern || (returns_gc_free_aggregate && hidden_sret_result_ty.is_none());

        let mut llvm_params = Vec::with_capacity(
            fun.params.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + self.explicit_effect_hidden_abi_param_count(uses_explicit_effect_hidden_abi)
                    as usize,
        );
        if hidden_sret_result_ty.is_some() {
            llvm_params.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        if uses_explicit_effect_hidden_abi {
            self.push_explicit_effect_hidden_abi_param_tys(&mut llvm_params);
        }
        for param in &fun.params {
            let param_ty = self
                .equivalent_codegen_type_id(mir_types, param.ty)
                .unwrap_or(param.ty);
            let llvm_param_ty = if is_extern {
                self.llvm_param_ty(param.span, param_ty)
            } else {
                self.ordinary_param_abi(param.span, param_ty)
                    .map(OrdinaryParamAbi::llvm_param_ty)
            };
            match llvm_param_ty {
                Ok(ty) => llvm_params.push(ty),
                Err(err) => {
                    tracing::warn!(
                        "declare_materialized_top_level_fun: unsupported param type for {} param {} -> {}",
                        fun.fqn,
                        param.name,
                        mir_types.display(param.ty)
                    );
                    return Err(err);
                }
            }
        }

        let fn_ty = match (hidden_sret_result_ty, return_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_params, false)
            }
            (None, other) => self
                .llvm_basic_type_of(fun.span, other)?
                .fn_type(&llvm_params, false),
        };

        let llvm_fun = if is_extern {
            self.declare_runtime_or_native_import_function(llvm_name, fn_ty)
        } else {
            self.declare_exported_abi_function(llvm_name, fn_ty)
        };
        llvm_fun.set_call_conventions(self.llvm_call_convention_for_fqn(&fun.fqn));
        if let Some(result_ty) = hidden_sret_result_ty {
            self.add_sret_attribute_to_function(llvm_fun, 0, result_ty);
        }
        if is_gc_leaf {
            self.mark_gc_leaf_function(llvm_fun);
        }
        Ok(llvm_fun)
    }

    fn declare_callee_resume_entry_function(
        &mut self,
        at: crate::span::Span,
        name: &str,
        return_cg: CgTy,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        if let Some(existing) = self.module.get_function(name) {
            return Ok(existing);
        }

        let hidden_sret_result_ty = self.hidden_sret_result_ty(at, return_cg)?;
        let mut llvm_params = Vec::with_capacity(
            usize::from(hidden_sret_result_ty.is_some())
                + self.explicit_effect_hidden_abi_param_count(true) as usize,
        );
        if hidden_sret_result_ty.is_some() {
            llvm_params.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        self.push_explicit_effect_hidden_abi_param_tys(&mut llvm_params);

        let fn_ty = match (hidden_sret_result_ty, return_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_params, false)
            }
            (None, other) => self
                .llvm_basic_type_of(at, other)?
                .fn_type(&llvm_params, false),
        };
        let llvm_fun =
            self.declare_compiler_private_helper_function(name, fn_ty, Linkage::Internal);
        llvm_fun.set_call_conventions(0);
        if let Some(result_ty) = hidden_sret_result_ty {
            self.add_sret_attribute_to_function(llvm_fun, 0, result_ty);
        }
        Ok(llvm_fun)
    }

    #[allow(dead_code)]
    fn declare_top_level_fun_callee_resume_entry(
        &mut self,
        fun: &hir::FunDecl,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let return_cg = self
            .cg_ty_of(fun.return_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "function return type",
                at: fun.span.into(),
            })?;
        self.declare_callee_resume_entry_function(
            fun.span,
            &top_level_callee_resume_entry_fn_name(&fun.fqn),
            return_cg,
        )
    }

    pub(super) fn mark_gc_leaf_function(&self, function: FunctionValue<'ctx>) {
        let attr = self.context.create_string_attribute("gc-leaf-function", "");
        function.add_attribute(inkwell::attributes::AttributeLoc::Function, attr);
    }

    fn llvm_call_convention_for_fqn(&self, fqn: &str) -> u32 {
        let Some(extern_fun) = self.extern_funs.get(fqn) else {
            return 0;
        };
        let Some(name) = extern_fun.calling_convention.as_deref() else {
            return 0;
        };

        match name.trim().to_ascii_lowercase().as_str() {
            "c" | "cdecl" => 0,
            // 其它 calling convention 名称留到后续任务再补齐（spec §15.5.4）。
            _ => 0,
        }
    }

    fn llvm_type_needs_sret(ty: BasicTypeEnum<'ctx>) -> bool {
        matches!(
            ty,
            BasicTypeEnum::StructType(_)
                | BasicTypeEnum::ArrayType(_)
                | BasicTypeEnum::VectorType(_)
                | BasicTypeEnum::ScalableVectorType(_)
        )
    }

    fn hidden_sret_result_ty(
        &mut self,
        at: crate::span::Span,
        ret_cg: CgTy,
    ) -> Result<Option<BasicTypeEnum<'ctx>>, LlvmEmitError> {
        let llvm_ret_ty = self.llvm_basic_type_of(at, ret_cg)?;
        Ok(Self::llvm_type_needs_sret(llvm_ret_ty).then_some(llvm_ret_ty))
    }

    fn sret_type_attribute(&self, result_ty: BasicTypeEnum<'ctx>) -> Attribute {
        let kind_id = Attribute::get_named_enum_kind_id("sret");
        self.context
            .create_type_attribute(kind_id, result_ty.as_any_type_enum())
    }

    fn add_sret_attribute_to_function(
        &self,
        llvm_fun: FunctionValue<'ctx>,
        param_index: u32,
        result_ty: BasicTypeEnum<'ctx>,
    ) {
        llvm_fun.add_attribute(
            AttributeLoc::Param(param_index),
            self.sret_type_attribute(result_ty),
        );
    }

    fn add_sret_attribute_to_call(
        &self,
        call_site: CallSiteValue<'ctx>,
        param_index: u32,
        result_ty: BasicTypeEnum<'ctx>,
    ) {
        call_site.add_attribute(
            AttributeLoc::Param(param_index),
            self.sret_type_attribute(result_ty),
        );
    }

    fn track_gc_root_slots_for_spill_slot(
        &mut self,
        at: crate::span::Span,
        slot: PointerValue<'ctx>,
        value_ty: BasicTypeEnum<'ctx>,
        name_prefix: &str,
    ) -> Result<(), LlvmEmitError> {
        let slot =
            self.rematerialize_ptr_in_current_block(at, slot, &format!("{name_prefix}_slot"))?;
        let mut gc_leaf_slots = Vec::new();
        self.collect_gc_ptr_leaf_slots_in_spill(slot, value_ty, name_prefix, &mut gc_leaf_slots)?;
        let explicit_frame_enabled = self
            .function_cx
            .explicit_frame_layout
            .frame_storage
            .is_some();
        let frame_slots = self
            .explicit_frame_slot_mirrors_for(slot)
            .map(|slots| slots.to_vec());
        if explicit_frame_enabled && frame_slots.is_none() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "spill slot explicit frame mirrors",
                at: at.into(),
            });
        }
        let frame_slots = frame_slots.unwrap_or_default();
        if explicit_frame_enabled && frame_slots.len() != gc_leaf_slots.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "spill slot/frame slot count mismatch",
                at: at.into(),
            });
        }
        for (index, (slot, value_ptr_ty)) in gc_leaf_slots.into_iter().enumerate() {
            let frame_slot = frame_slots.get(index).copied().unwrap_or(slot);
            self.function_cx
                .tracked_gc_root_slots
                .push(TrackedGcRootSlot {
                    slot,
                    value_ptr_ty,
                    frame_slot,
                });
        }
        Ok(())
    }

    fn sync_hidden_sret_result_roots(
        &mut self,
        at: crate::span::Span,
        ret_cg: CgTy,
        result_ptr: PointerValue<'ctx>,
        name_prefix: &str,
    ) -> Result<(), LlvmEmitError> {
        let llvm_ret_ty = self.llvm_basic_type_of(at, ret_cg)?;
        if !self.basic_type_contains_gc_ptrs(at, llvm_ret_ty)? {
            return Ok(());
        }
        self.sync_storage_slot_into_explicit_frame(at, result_ptr, llvm_ret_ty, name_prefix)
    }

    fn load_hidden_sret_result_from_ptr(
        &mut self,
        at: crate::span::Span,
        ret_cg: CgTy,
        result_ptr: PointerValue<'ctx>,
        name_prefix: &str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let llvm_ret_ty = self.llvm_basic_type_of(at, ret_cg)?;
        self.sync_hidden_sret_result_roots(at, ret_cg, result_ptr, name_prefix)?;
        let reload_slot = self.storage_slot_for_use(at, result_ptr, ret_cg, name_prefix)?;
        let loaded = self
            .builder
            .build_load(llvm_ret_ty, reload_slot, "sret_result")?;
        let result = self.cg_value_from_loaded(at, ret_cg, loaded)?;
        self.clear_spill_slot_root_homes(at, result_ptr, llvm_ret_ty, name_prefix)?;
        Ok(result)
    }

    fn defer_direct_call_result(
        &mut self,
        at: crate::span::Span,
        ret_cg: CgTy,
        call_site: CallSiteValue<'ctx>,
        name: &str,
    ) -> Result<Option<DeferredCgValue<'ctx>>, LlvmEmitError> {
        match ret_cg {
            CgTy::Unit | CgTy::Never => Ok(None),
            _ => {
                let raw = call_site.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "call return value",
                        at: at.into(),
                    },
                )?;
                let value = self.cg_value_from_loaded(at, ret_cg, raw)?;
                Ok(Some(self.defer_gc_sensitive_cg_value(at, name, value)?))
            }
        }
    }

    fn clear_gc_locals_in_current_scope(
        &mut self,
        at: crate::span::Span,
        name_prefix: &str,
    ) -> Result<(), LlvmEmitError> {
        let Some(scope) = self.function_cx.env.scopes.last() else {
            return Ok(());
        };

        let locals: Vec<CgLocal<'ctx>> = scope.values().copied().collect();
        for local in locals {
            let llvm_ty = self.llvm_basic_type_of(at, local.ty)?;
            if !self.basic_type_contains_gc_ptrs(at, llvm_ty)? {
                continue;
            }
            self.clear_spill_slot_root_homes(at, local.ptr, llvm_ty, name_prefix)?;
        }
        Ok(())
    }

    fn defer_class_field_place(
        &mut self,
        receiver: &hir::Expr,
        member_span: crate::span::Span,
        field_fqn: &str,
        receiver_hir_ty: TypeId,
        name_prefix: &str,
    ) -> Result<Option<DeferredClassFieldPlace<'ctx>>, LlvmEmitError> {
        let Some((class, field_idx, field_cg)) =
            self.lookup_class_field_by_fqn(field_fqn, member_span, Some(receiver_hir_ty))?
        else {
            return Ok(None);
        };
        let field =
            class
                .fields
                .get(field_idx as usize)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "class field index",
                    at: member_span.into(),
                })?;
        let writable = field.mutable;
        let recv = self.codegen_expr_in_expected_context(receiver, Some(CgTy::Ref))?;
        let recv = self.coerce_value(receiver.span, recv, CgTy::Ref)?;
        let Some(raw) = recv.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "class field receiver value",
                at: receiver.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(obj_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "class field receiver type",
                at: receiver.span.into(),
            });
        };

        Ok(Some(DeferredClassFieldPlace {
            class,
            field_idx,
            field_cg,
            writable,
            receiver: self.defer_gc_ref_pointer(
                receiver.span,
                &format!("{name_prefix}_receiver"),
                obj_ptr,
            )?,
        }))
    }

    fn reload_deferred_class_field_place_ptr(
        &mut self,
        at: crate::span::Span,
        place: &DeferredClassFieldPlace<'ctx>,
        name_prefix: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let receiver = self.reload_deferred_gc_ref_without_clearing(
            at,
            &format!("{name_prefix}_receiver_reload"),
            &place.receiver,
        )?;
        self.codegen_class_field_ptr(at, &place.class, receiver, place.field_idx)
    }

    fn declare_top_level_var_global(
        &mut self,
        v: &hir::TopLevelVar,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let name = top_level_var_global_name(&v.fqn);
        if let Some(existing) = self.module.get_global(&name) {
            return Ok(existing);
        }

        let cg_ty = self
            .cg_ty_of(v.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level var type",
                at: v.span.into(),
            })?;

        let llvm_ty = self.llvm_basic_type_of(v.span, cg_ty)?;
        let gv = self.module.add_global(llvm_ty, None, &name);
        gv.set_linkage(Linkage::Internal);

        if v.storage == hir::TopLevelVarStorage::ThreadLocal {
            gv.set_thread_local(true);
        }

        let saved_source_id = self.current_source_id;
        self.current_source_id = self.source_id_for_path(v.source_path.as_path(), v.span)?;
        let init = self.const_initializer_for_top_level_var(v, cg_ty, llvm_ty);
        self.current_source_id = saved_source_id;
        let init = init?;
        gv.set_initializer(&init);

        // `@CLayout(aligned = N)`：对显式对齐的值类型，在全局存储上透传 alignment。
        if let CgTy::Struct(struct_ty) = cg_ty
            && let Some(aligned) = self.struct_clayout(struct_ty).and_then(|c| c.aligned)
        {
            gv.set_alignment(aligned);
        }
        Ok(gv)
    }

    fn declare_extern_global(
        &mut self,
        global: &hir::ExternGlobal,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        self.declare_extern_global_storage(
            global.span,
            global.ty,
            &global.symbol,
            global.linkage,
            global.storage,
            global.initializer_absent,
        )
    }

    fn declare_mir_extern_global(
        &mut self,
        global: &crate::mir::ExternGlobalRoot,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        self.declare_extern_global_storage(
            global.span,
            global.ty,
            &global.symbol,
            global.linkage,
            global.storage,
            global.initializer_absent,
        )
    }

    fn declare_extern_global_storage(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
        symbol: &str,
        linkage: hir::ExternGlobalLinkage,
        storage: hir::TopLevelVarStorage,
        initializer_absent: bool,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        if !initializer_absent {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "extern global initializer contract",
                at: span.into(),
            });
        }
        let cg_ty = self
            .cg_ty_of(ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "extern global type",
                at: span.into(),
            })?;
        let llvm_ty = self.llvm_basic_type_of(span, cg_ty)?;
        let gv = self
            .module
            .get_global(symbol)
            .unwrap_or_else(|| self.module.add_global(llvm_ty, None, symbol));
        match linkage {
            hir::ExternGlobalLinkage::External => gv.set_linkage(Linkage::External),
        }
        gv.set_thread_local(storage == hir::TopLevelVarStorage::ThreadLocal);

        if let CgTy::Struct(struct_ty) = cg_ty
            && let Some(aligned) = self.struct_clayout(struct_ty).and_then(|c| c.aligned)
        {
            gv.set_alignment(aligned);
        }

        Ok(gv)
    }

    fn const_initializer_for_top_level_var(
        &mut self,
        v: &hir::TopLevelVar,
        cg_ty: CgTy,
        llvm_ty: BasicTypeEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, LlvmEmitError> {
        let Some(init) = v.init.as_ref() else {
            return Ok(self.zero_initializer_for_basic_type(llvm_ty));
        };

        Ok(match cg_ty {
            CgTy::Unit | CgTy::Never => self.context.i8_type().const_int(0, false).into(),
            CgTy::Bool => {
                let value =
                    self.const_eval_bool_expr(init)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "top-level var initializer (bool const)",
                            at: init.span.into(),
                        })?;
                self.context
                    .bool_type()
                    .const_int(value as u64, false)
                    .into()
            }
            CgTy::Int(int_ty) => {
                let bits = self.const_eval_int_expr_bits(init, int_ty)?.ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "top-level var initializer (int const)",
                        at: init.span.into(),
                    },
                )?;
                let value = mask_to_bits(bits, int_ty.bits) as u64;
                self.int_type(int_ty).const_int(value, false).into()
            }
            CgTy::Float64 | CgTy::Float32 => self.const_eval_float_expr(init, cg_ty).ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "top-level var initializer (float const)",
                    at: init.span.into(),
                },
            )?,
            // 早期阶段：仅支持"静态全零初始化"；更复杂的值类型常量构造留给后续任务补齐。
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "top-level var initializer (aggregate const)",
                    at: init.span.into(),
                });
            }
            CgTy::String | CgTy::Ref => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "top-level var initializer (non-gc-free)",
                    at: init.span.into(),
                });
            }
        })
    }

    fn const_eval_float_expr(
        &self,
        expr: &hir::Expr,
        target_ty: CgTy,
    ) -> Option<BasicValueEnum<'ctx>> {
        let value = self.const_eval_float_expr_value(expr)?;
        match target_ty {
            CgTy::Float64 => Some(self.context.f64_type().const_float(value).into()),
            CgTy::Float32 => Some(
                self.context
                    .f32_type()
                    .const_float(f64::from(value as f32))
                    .into(),
            ),
            _ => None,
        }
    }

    fn const_eval_float_expr_value(&self, expr: &hir::Expr) -> Option<f64> {
        match &expr.kind {
            hir::ExprKind::Literal(hir::LiteralKind::Float64(value)) => Some(*value),
            hir::ExprKind::Literal(hir::LiteralKind::Float32(value)) => Some(f64::from(*value)),
            hir::ExprKind::Unary {
                op: ast::UnaryOp::Neg,
                expr: inner,
                ..
            } => Some(-self.const_eval_float_expr_value(inner)?),
            _ => None,
        }
    }

    fn const_eval_int_expr_bits(
        &self,
        expr: &hir::Expr,
        int_ty: IntTy,
    ) -> Result<Option<u128>, LlvmEmitError> {
        match &expr.kind {
            hir::ExprKind::Literal(hir::LiteralKind::Int) => Ok(Some(u128::from(
                self.int_literal_bits_for_ty(expr.span, int_ty)?,
            ))),
            hir::ExprKind::Literal(hir::LiteralKind::SynthInt(v)) => {
                Ok(Some(mask_to_bits(*v as u128, int_ty.bits)))
            }
            hir::ExprKind::Unary {
                op: ast::UnaryOp::Neg,
                expr: inner,
                ..
            } if matches!(inner.kind, hir::ExprKind::Literal(hir::LiteralKind::Int)) => Ok(Some(
                u128::from(self.negated_int_literal_bits_for_ty(expr.span, inner.span, int_ty)?),
            )),
            hir::ExprKind::Unary {
                op: ast::UnaryOp::Neg,
                expr: inner,
                ..
            } => Ok(self
                .const_eval_int_expr_bits(inner, int_ty)?
                .map(|v| mask_to_bits(0u128.wrapping_sub(v), int_ty.bits))),
            hir::ExprKind::Unary {
                op: ast::UnaryOp::BitNot,
                expr: inner,
                ..
            } => Ok(self
                .const_eval_int_expr_bits(inner, int_ty)?
                .map(|v| mask_to_bits(!v, int_ty.bits))),
            _ => Ok(None),
        }
    }

    fn const_eval_bool_expr(&self, expr: &hir::Expr) -> Option<bool> {
        match &expr.kind {
            hir::ExprKind::Literal(hir::LiteralKind::Bool(v)) => Some(*v),
            hir::ExprKind::Unary {
                op: ast::UnaryOp::Not,
                expr: inner,
                ..
            } => Some(!self.const_eval_bool_expr(inner)?),
            _ => None,
        }
    }

    fn zero_initializer_for_basic_type(
        &self,
        llvm_ty: BasicTypeEnum<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        match llvm_ty {
            BasicTypeEnum::IntType(ty) => BasicValueEnum::IntValue(ty.const_int(0, false)),
            BasicTypeEnum::PointerType(ty) => BasicValueEnum::PointerValue(ty.const_null()),
            BasicTypeEnum::StructType(ty) => BasicValueEnum::StructValue(ty.const_zero()),
            BasicTypeEnum::ArrayType(ty) => BasicValueEnum::ArrayValue(ty.const_zero()),
            BasicTypeEnum::FloatType(ty) => BasicValueEnum::FloatValue(ty.const_float(0.0)),
            BasicTypeEnum::VectorType(ty) => BasicValueEnum::VectorValue(ty.const_zero()),
            BasicTypeEnum::ScalableVectorType(ty) => {
                BasicValueEnum::ScalableVectorValue(ty.const_zero())
            }
        }
    }

    fn rematerialize_ptr_in_current_block(
        &mut self,
        at: crate::span::Span,
        ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let Some(inst) = ptr.as_instruction_value() else {
            return Ok(ptr);
        };

        match inst.get_opcode() {
            inkwell::values::InstructionOpcode::Load => {
                let base = inst
                    .get_operand(0)
                    .and_then(|operand| operand.value())
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "local slot load base operand",
                        at: at.into(),
                    })?;
                let BasicValueEnum::PointerValue(base_ptr) = base else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "local slot load base pointer type",
                        at: at.into(),
                    });
                };
                let base_ptr =
                    self.rematerialize_ptr_in_current_block(at, base_ptr, &format!("{name}_base"))?;
                let rebuilt = self.builder.build_load(ptr.get_type(), base_ptr, name)?;
                return Ok(rebuilt.into_pointer_value());
            }
            inkwell::values::InstructionOpcode::BitCast
            | inkwell::values::InstructionOpcode::AddrSpaceCast => {
                let base = inst
                    .get_operand(0)
                    .and_then(|operand| operand.value())
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "local slot cast base operand",
                        at: at.into(),
                    })?;
                let BasicValueEnum::PointerValue(base_ptr) = base else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "local slot cast base pointer type",
                        at: at.into(),
                    });
                };
                let base_ptr =
                    self.rematerialize_ptr_in_current_block(at, base_ptr, &format!("{name}_base"))?;
                let target_ty = ptr.get_type();
                return if base_ptr.get_type().get_address_space() == target_ty.get_address_space() {
                    Ok(self.builder.build_pointer_cast(base_ptr, target_ty, name)?)
                } else {
                    Ok(self
                        .builder
                        .build_address_space_cast(base_ptr, target_ty, name)?)
                };
            }
            inkwell::values::InstructionOpcode::GetElementPtr => {}
            _ => return Ok(ptr),
        }

        let base = inst
            .get_operand(0)
            .and_then(|operand| operand.value())
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "local slot gep base operand",
                at: at.into(),
            })?;
        let BasicValueEnum::PointerValue(base_ptr) = base else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "local slot gep base pointer type",
                at: at.into(),
            });
        };
        let base_ptr =
            self.rematerialize_ptr_in_current_block(at, base_ptr, &format!("{name}_base"))?;

        let source_ty =
            inst.get_gep_source_element_type()
                .map_err(|_| LlvmEmitError::UnsupportedMainBody {
                    kind: "local slot gep source type",
                    at: at.into(),
                })?;
        match source_ty {
            BasicTypeEnum::StructType(struct_ty) => {
                let mut indices = inst.get_indices();
                if indices.is_empty() {
                    for operand_index in 1..inst.get_num_operands() {
                        let Some(operand) =
                            inst.get_operand(operand_index).and_then(|op| op.value())
                        else {
                            return Ok(ptr);
                        };
                        let BasicValueEnum::IntValue(index_value) = operand else {
                            return Ok(ptr);
                        };
                        let Some(index) = index_value.get_zero_extended_constant() else {
                            return Ok(ptr);
                        };
                        indices.push(index as u32);
                    }
                }
                let field_index = match indices.as_slice() {
                    [field_index] => *field_index,
                    [0, field_index] => *field_index,
                    _ => return Ok(ptr),
                };

                Ok(self
                    .builder
                    .build_struct_gep(struct_ty, base_ptr, field_index, name)?)
            }
            BasicTypeEnum::IntType(int_ty) if int_ty.get_bit_width() == 8 => {
                let mut index = None;
                for operand_index in 1..inst.get_num_operands() {
                    let Some(operand) = inst.get_operand(operand_index).and_then(|op| op.value())
                    else {
                        return Ok(ptr);
                    };
                    let BasicValueEnum::IntValue(index_value) = operand else {
                        return Ok(ptr);
                    };
                    let Some(constant) = index_value.get_zero_extended_constant() else {
                        return Ok(ptr);
                    };
                    if index.replace(constant).is_some() {
                        return Ok(ptr);
                    }
                }
                let Some(index) = index else {
                    return Ok(ptr);
                };
                let rebuilt = unsafe {
                    self.builder.build_in_bounds_gep(
                        int_ty,
                        base_ptr,
                        &[self.context.i64_type().const_int(index, false)],
                        name,
                    )?
                };
                Ok(rebuilt)
            }
            _ => Ok(ptr),
        }
    }

    fn local_ptr_for_use(
        &mut self,
        at: crate::span::Span,
        local: CgLocal<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        self.storage_slot_for_use(at, local.ptr, local.ty, name)
    }

    fn clear_spill_slot_root_homes(
        &mut self,
        at: crate::span::Span,
        slot: PointerValue<'ctx>,
        value_ty: BasicTypeEnum<'ctx>,
        name_prefix: &str,
    ) -> Result<(), LlvmEmitError> {
        for (_, value_ptr_ty, frame_slot) in
            self.explicit_frame_leaf_slot_pairs_for_storage_slot(at, slot, value_ty, name_prefix)?
        {
            let _ = self
                .builder
                .build_store(frame_slot, value_ptr_ty.const_null())?;
        }
        Ok(())
    }

    fn collect_gc_ptr_leaf_slots_in_spill(
        &mut self,
        slot: PointerValue<'ctx>,
        value_ty: BasicTypeEnum<'ctx>,
        name_prefix: &str,
        out: &mut Vec<(PointerValue<'ctx>, PointerType<'ctx>)>,
    ) -> Result<(), LlvmEmitError> {
        match value_ty {
            BasicTypeEnum::PointerType(ptr_ty) => {
                if ptr_ty.get_address_space() == self.gc_address_space() {
                    out.push((slot, ptr_ty));
                }
            }
            BasicTypeEnum::StructType(st) => {
                if st.is_opaque() {
                    return Ok(());
                }
                for (idx, field_ty) in st.get_field_types().into_iter().enumerate() {
                    let field_slot = self.builder.build_struct_gep(
                        st,
                        slot,
                        idx as u32,
                        &format!("{name_prefix}_field_{idx}"),
                    )?;
                    self.collect_gc_ptr_leaf_slots_in_spill(
                        field_slot,
                        field_ty,
                        name_prefix,
                        out,
                    )?;
                }
            }
            BasicTypeEnum::ArrayType(arr) => {
                let i32_ty = self.context.i32_type();
                let zero = i32_ty.const_zero();
                for idx in 0..arr.len() {
                    let elem_slot = unsafe {
                        self.builder.build_in_bounds_gep(
                            arr,
                            slot,
                            &[zero, i32_ty.const_int(idx as u64, false)],
                            &format!("{name_prefix}_elem_{idx}"),
                        )?
                    };
                    self.collect_gc_ptr_leaf_slots_in_spill(
                        elem_slot,
                        arr.get_element_type(),
                        name_prefix,
                        out,
                    )?;
                }
            }
            BasicTypeEnum::IntType(_)
            | BasicTypeEnum::FloatType(_)
            | BasicTypeEnum::VectorType(_)
            | BasicTypeEnum::ScalableVectorType(_) => {}
        }
        Ok(())
    }

    fn defer_gc_sensitive_cg_value(
        &mut self,
        at: crate::span::Span,
        name: &str,
        value: CgValue<'ctx>,
    ) -> Result<DeferredCgValue<'ctx>, LlvmEmitError> {
        let ty = value.ty;
        let Some(raw) = value.value else {
            return Ok(DeferredCgValue {
                ty,
                immediate: None,
                spill: None,
            });
        };

        let llvm_ty = self.llvm_basic_type_of(at, value.ty)?;
        if !self.basic_type_contains_gc_ptrs(at, llvm_ty)? {
            return Ok(DeferredCgValue {
                ty,
                immediate: Some(raw),
                spill: None,
            });
        }

        let slot = self.create_entry_alloca(at, name, ty)?;
        let _ = self.store_local_value_exact(at, slot, ty, value)?;
        self.track_gc_root_slots_for_spill_slot(at, slot, llvm_ty, name)?;

        Ok(DeferredCgValue {
            ty,
            immediate: None,
            spill: Some(DeferredGcSensitiveSpill {
                slot,
                value_ty: llvm_ty,
            }),
        })
    }

    fn defer_gc_ref_pointer(
        &mut self,
        at: crate::span::Span,
        name: &str,
        ptr: PointerValue<'ctx>,
    ) -> Result<DeferredCgValue<'ctx>, LlvmEmitError> {
        self.defer_gc_sensitive_cg_value(
            at,
            name,
            CgValue {
                ty: CgTy::Ref,
                value: Some(ptr.into()),
            },
        )
    }

    fn reload_deferred_gc_ref_without_clearing(
        &mut self,
        at: crate::span::Span,
        name: &str,
        value: &DeferredCgValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        if let Some(spill) = &value.spill {
            let reload_slot = self.storage_slot_for_use(at, spill.slot, value.ty, name)?;
            let loaded = self
                .builder
                .build_load(self.llvm_gc_i8_ptr_type(), reload_slot, name)?;
            return Ok(loaded.into_pointer_value());
        }

        let Some(raw) = value.immediate else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "deferred gc ref reload",
                at: at.into(),
            });
        };
        let BasicValueEnum::PointerValue(ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "deferred gc ref reload type",
                at: at.into(),
            });
        };
        Ok(ptr)
    }

    fn reload_deferred_cg_value_without_clearing(
        &mut self,
        at: crate::span::Span,
        name: &str,
        value: &DeferredCgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if let Some(raw) = value.immediate {
            return Ok(CgValue {
                ty: value.ty,
                value: Some(raw),
            });
        }

        if let Some(spill) = &value.spill {
            let reload_slot = self.storage_slot_for_use(at, spill.slot, value.ty, name)?;
            let llvm_ty = self.llvm_basic_type_of(at, value.ty)?;
            let loaded = self.builder.build_load(llvm_ty, reload_slot, name)?;
            return Ok(CgValue {
                ty: value.ty,
                value: Some(loaded),
            });
        }

        match value.ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "reload deferred value without clearing",
                at: at.into(),
            }),
        }
    }

    fn clear_deferred_cg_value_root_homes(
        &mut self,
        at: crate::span::Span,
        name: &str,
        value: &DeferredCgValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        if let Some(spill) = &value.spill {
            self.clear_spill_slot_root_homes(at, spill.slot, spill.value_ty, name)?;
        }
        Ok(())
    }

    fn materialize_deferred_cg_value(
        &mut self,
        at: crate::span::Span,
        name: &str,
        value: DeferredCgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if let Some(raw) = value.immediate {
            return Ok(CgValue {
                ty: value.ty,
                value: Some(raw),
            });
        }

        if let Some(spill) = value.spill {
            let reload_slot = self.storage_slot_for_use(at, spill.slot, value.ty, name)?;
            let llvm_ty = self.llvm_basic_type_of(at, value.ty)?;
            let loaded = self.builder.build_load(llvm_ty, reload_slot, name)?;
            self.clear_spill_slot_root_homes(at, spill.slot, spill.value_ty, name)?;
            return Ok(CgValue {
                ty: value.ty,
                value: Some(loaded),
            });
        }

        match value.ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "materialize deferred value",
                at: at.into(),
            }),
        }
    }

    fn codegen_initializer_expr(
        &mut self,
        expr: &hir::Expr,
        target_ty: CgTy,
        target_hir_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match &expr.kind {
            hir::ExprKind::Closure(closure) => {
                self.codegen_closure_expr(expr.span, closure, target_hir_ty)
            }
            // 对 call initializer 传入声明类型，避免泛型 ctor 等路径因为 HIR `expr.ty = Any`
            // 丢失结果类型信息。
            hir::ExprKind::Call { callee, args } => self.codegen_call(
                expr.span,
                callee,
                args,
                Some(target_ty),
                Some(target_hir_ty),
            ),
            _ => self.codegen_expr_in_expected_context(expr, Some(target_ty)),
        }
    }

    fn codegen_decl_initializer_expr(
        &mut self,
        decl: &hir::ValDecl,
        target_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let init = decl
            .init
            .as_ref()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "val without initializer",
                at: decl.span.into(),
            })?;

        self.codegen_initializer_expr(init, target_ty, decl.ty)
    }

    fn codegen_top_level_const_ref(
        &mut self,
        span: crate::span::Span,
        top_level_const: &hir::TopLevelConst,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if self
            .function_cx
            .top_level_const_eval_stack
            .iter()
            .any(|current| current == &top_level_const.fqn)
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "recursive top-level const ref",
                at: span.into(),
            });
        }

        let init = top_level_const
            .init
            .as_ref()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level const without initializer",
                at: top_level_const.span.into(),
            })?;
        let target_ty =
            self.cg_ty_of(top_level_const.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "top-level const type",
                    at: top_level_const.span.into(),
                })?;
        let source_id = self.source_id_for_path(top_level_const.source_path.as_path(), span)?;
        let saved_source_id = self.current_source_id;
        self.current_source_id = source_id;
        self.function_cx
            .top_level_const_eval_stack
            .push(top_level_const.fqn.clone());
        let result = self.codegen_initializer_expr(init, target_ty, top_level_const.ty);
        self.function_cx.top_level_const_eval_stack.pop();
        self.current_source_id = saved_source_id;
        result
    }

    fn declare_top_level_immutable_value_guard(&self, value_fqn: &str) -> GlobalValue<'ctx> {
        let name = top_level_immutable_value_guard_global_name(value_fqn);
        if let Some(existing) = self.module.get_global(&name) {
            return existing;
        }

        let gv = self.module.add_global(self.context.i64_type(), None, &name);
        gv.set_linkage(Linkage::Internal);
        gv.set_initializer(&self.context.i64_type().const_int(0, false));
        gv
    }

    fn emit_top_level_immutable_value_initialized_check(
        &mut self,
        at: crate::span::Span,
        value_fqn: &str,
    ) -> Result<(), LlvmEmitError> {
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: at.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: at.into(),
            })?;

        let ready_bb = self.context.append_basic_block(func, "top_level_val_ready");
        let recursive_bb = self
            .context
            .append_basic_block(func, "top_level_val_recursive");

        let guard = self.declare_top_level_immutable_value_guard(value_fqn);
        let guard_word = self
            .builder
            .build_load(
                self.context.i64_type(),
                guard.as_pointer_value(),
                "top_level_val_guard_word",
            )?
            .into_int_value();
        let state_mask = self.context.i64_type().const_int(0x3, false);
        let guard_state =
            self.builder
                .build_and(guard_word, state_mask, "top_level_val_guard_state")?;
        let initialized_state = self.context.i64_type().const_int(2, false);
        let is_initialized = self.builder.build_int_compare(
            IntPredicate::EQ,
            guard_state,
            initialized_state,
            "top_level_val_guard_is_initialized",
        )?;
        self.builder
            .build_conditional_branch(is_initialized, ready_bb, recursive_bb)?;

        self.builder.position_at_end(recursive_bb);
        // `scoop_once_begin` 在同线程重入时会直接返回 0 以避免死锁；若此时继续读取 backing global，
        // 就会把“尚未完成初始化”的零值伪装成合法结果。这里要求 guard 已真正进入 initialized，
        // 否则立即终止，阻止递归初始化落成静默错误值。
        self.emit_exit_with_code(at, 1)?;

        self.builder.position_at_end(ready_bb);
        Ok(())
    }

    fn declare_top_level_immutable_value_global(
        &mut self,
        at: crate::span::Span,
        value: &hir::TopLevelImmutableValue,
        value_cg: CgTy,
    ) -> Result<Option<GlobalValue<'ctx>>, LlvmEmitError> {
        if value_cg == CgTy::Unit {
            return Ok(None);
        }

        let name = top_level_immutable_value_global_name(&value.fqn);
        if let Some(existing) = self.module.get_global(&name) {
            return Ok(Some(existing));
        }

        let llvm_ty = self.llvm_basic_type_of(at, value_cg)?;
        let gv = self.module.add_global(llvm_ty, None, &name);
        gv.set_linkage(Linkage::Internal);
        gv.set_initializer(&self.zero_initializer_for_basic_type(llvm_ty));

        if let CgTy::Struct(struct_ty) = value_cg
            && let Some(aligned) = self.struct_clayout(struct_ty).and_then(|c| c.aligned)
        {
            gv.set_alignment(aligned);
        }

        Ok(Some(gv))
    }

    fn ensure_top_level_immutable_value_init_function_defined(
        &mut self,
        value_fqn: &str,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let Some(value) = self.top_level_immutable_values.get(value_fqn) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level immutable value init (missing metadata)",
                at: crate::span::Span::new(0, 0).into(),
            });
        };

        let name = top_level_immutable_value_init_fn_name(value_fqn);
        let fn_ty = self.context.void_type().fn_type(&[], false);
        let llvm_fun =
            self.declare_compiler_private_helper_function(&name, fn_ty, Linkage::Internal);

        if llvm_fun.get_first_basic_block().is_some() {
            return Ok(llvm_fun);
        }

        let saved_block = self.builder.get_insert_block();

        let mut init_codegen = self.fresh_child_codegen();
        init_codegen.codegen_top_level_immutable_value_init_fun_body(value, llvm_fun)?;

        if let Some(bb) = saved_block {
            self.builder.position_at_end(bb);
        }

        Ok(llvm_fun)
    }

    pub(in crate::llvm::codegen) fn ensure_refactor_top_level_immutable_value_init_bridge_defined(
        &mut self,
        value_fqn: &str,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let Some(value) = self.top_level_immutable_values.get(value_fqn) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor hidden top-level immutable init bridge (missing metadata)",
                at: crate::span::Span::new(0, 0).into(),
            });
        };

        let name = refactor_hidden_top_level_immutable_value_init_bridge_fn_name(value_fqn);
        let fn_ty = self.llvm_effect_outcome_struct_type().fn_type(&[], false);
        let llvm_fun =
            self.declare_compiler_private_helper_function(&name, fn_ty, Linkage::Internal);

        if llvm_fun.get_first_basic_block().is_some() {
            return Ok(llvm_fun);
        }

        let saved_block = self.builder.get_insert_block();

        let mut bridge_codegen = self.fresh_child_codegen();
        bridge_codegen
            .codegen_refactor_top_level_immutable_value_init_bridge_body(value, llvm_fun)?;

        if let Some(bb) = saved_block {
            self.builder.position_at_end(bb);
        }

        Ok(llvm_fun)
    }

    fn codegen_refactor_top_level_immutable_value_init_bridge_body(
        &mut self,
        value: &hir::TopLevelImmutableValue,
        llvm_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let err_span = value
            .init
            .as_ref()
            .map(|init| init.span)
            .unwrap_or(value.span);
        self.current_source_id = self.source_id_for_path(value.source_path.as_path(), err_span)?;

        let entry = self.context.append_basic_block(llvm_fun, "entry");
        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(llvm_fun)?;

        let init_fn = self.ensure_top_level_immutable_value_init_function_defined(&value.fqn)?;
        self.with_conservative_gc_local_root_spills(err_span, |cg| {
            let _ = cg.builder.build_call(init_fn, &[], "top_level_val_init")?;
            Ok(())
        })?;
        let outcome = self.build_zero_complete_effect_outcome()?;
        self.builder.build_return(Some(&outcome))?;
        self.finish_function_explicit_frame_layout(err_span)?;
        Ok(())
    }

    fn codegen_top_level_immutable_value_init_fun_body(
        &mut self,
        value: &hir::TopLevelImmutableValue,
        llvm_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let err_span = value
            .init
            .as_ref()
            .map(|init| init.span)
            .unwrap_or(value.span);
        self.current_source_id = self.source_id_for_path(value.source_path.as_path(), err_span)?;

        let entry = self.context.append_basic_block(llvm_fun, "entry");
        let init_bb = self.context.append_basic_block(llvm_fun, "init");
        let done_bb = self.context.append_basic_block(llvm_fun, "done");

        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(llvm_fun)?;
        self.function_cx.current_fun_return_ty = Some(CgTy::Unit);

        let guard = self.declare_top_level_immutable_value_guard(&value.fqn);
        let once_begin = self.declare_runtime_once_begin();
        let call = self.builder.build_call(
            once_begin,
            &[guard.as_pointer_value().into()],
            "once_begin",
        )?;
        let ret = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level immutable value once begin return value",
                at: err_span.into(),
            })?;
        let BasicValueEnum::IntValue(should_init) = ret else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level immutable value once begin return type",
                at: err_span.into(),
            });
        };
        let i32_ty = self.context.i32_type();
        let cond = self.builder.build_int_compare(
            IntPredicate::NE,
            should_init,
            i32_ty.const_int(0, false),
            "should_init",
        )?;
        self.builder
            .build_conditional_branch(cond, init_bb, done_bb)?;

        self.builder.position_at_end(init_bb);

        let init = value
            .init
            .as_ref()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level immutable value without initializer",
                at: value.span.into(),
            })?;
        let value_cg = self
            .cg_ty_of(value.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level immutable value type",
                at: value.span.into(),
            })?;
        let init_value = self.codegen_initializer_expr(init, value_cg, value.ty)?;
        if let Some(global) =
            self.declare_top_level_immutable_value_global(init.span, value, value_cg)?
        {
            let _stored =
                self.store_local_value(init.span, global.as_pointer_value(), value_cg, init_value)?;
            let storage_ty = self.llvm_basic_type_of(init.span, value_cg)?;
            let global_name = top_level_immutable_value_global_name(&value.fqn);
            self.register_global_root_if_needed(init.span, global, &global_name, storage_ty)?;
        }

        let once_end = self.declare_runtime_once_end();
        let _ =
            self.builder
                .build_call(once_end, &[guard.as_pointer_value().into()], "once_end")?;
        self.builder.build_unconditional_branch(done_bb)?;

        self.builder.position_at_end(done_bb);
        self.builder.build_return(None)?;
        self.finish_function_explicit_frame_layout(err_span)?;
        Ok(())
    }

    fn codegen_top_level_immutable_value_access(
        &mut self,
        at: crate::span::Span,
        value: &hir::TopLevelImmutableValue,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let value_cg = self
            .cg_ty_of(value.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level immutable value type",
                at: value.span.into(),
            })?;
        let init_fn = self.ensure_top_level_immutable_value_init_function_defined(&value.fqn)?;
        self.with_conservative_gc_local_root_spills(at, |cg| {
            let _ = cg.builder.build_call(init_fn, &[], "top_level_val_init")?;
            Ok(())
        })?;
        self.emit_top_level_immutable_value_initialized_check(at, &value.fqn)?;

        if value_cg == CgTy::Unit {
            return Ok(CgValue::unit());
        }

        let Some(global) = self.declare_top_level_immutable_value_global(at, value, value_cg)?
        else {
            return Ok(CgValue::unit());
        };
        let llvm_ty = self.llvm_basic_type_of(at, value_cg)?;
        let loaded =
            self.builder
                .build_load(llvm_ty, global.as_pointer_value(), "load_top_level_val")?;
        self.cg_value_from_loaded(at, value_cg, loaded)
    }

    pub(in crate::llvm::codegen) fn load_initialized_top_level_immutable_value(
        &mut self,
        at: crate::span::Span,
        value: &hir::TopLevelImmutableValue,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let value_cg = self
            .cg_ty_of(value.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level immutable value type",
                at: value.span.into(),
            })?;
        self.emit_top_level_immutable_value_initialized_check(at, &value.fqn)?;

        if value_cg == CgTy::Unit {
            return Ok(CgValue::unit());
        }

        let Some(global) = self.declare_top_level_immutable_value_global(at, value, value_cg)?
        else {
            return Ok(CgValue::unit());
        };
        let llvm_ty = self.llvm_basic_type_of(at, value_cg)?;
        let loaded =
            self.builder
                .build_load(llvm_ty, global.as_pointer_value(), "load_top_level_val")?;
        self.cg_value_from_loaded(at, value_cg, loaded)
    }

    fn codegen_top_level_value_ref(
        &mut self,
        span: crate::span::Span,
        fqn: &str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // T1311：object/companion object 单例值在表达式位置可用：
        // - 读取单例值应触发一次初始化（init block / 属性 init）；
        // - 运行期用一个 module-local 的唯一地址作为"单例实例指针"（ref type）。
        if self.object_inits.contains_key(fqn) {
            return self.codegen_object_value_access(span, fqn);
        }

        if let Some(top_level_const) = self.top_level_consts.get(fqn).cloned() {
            return self.codegen_top_level_const_ref(span, &top_level_const);
        }

        if let Some(value) = self.top_level_immutable_values.get(fqn).cloned() {
            return self.codegen_top_level_immutable_value_access(span, &value);
        }

        if let Some(global) = self.materialized_extern_global_root(fqn).cloned() {
            return self.codegen_mir_extern_global_access(span, &global);
        }

        if let Some(global) = self.extern_globals.get(fqn).cloned() {
            return self.codegen_extern_global_access(span, &global);
        }

        // T1023：`@ThreadLocal/@Global var` 顶层可变变量。
        let Some(var) = self.top_level_vars.get(fqn) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level value ref",
                at: span.into(),
            });
        };

        let cg_ty = self
            .cg_ty_of(var.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level var type",
                at: var.span.into(),
            })?;

        if cg_ty == CgTy::Unit {
            return Ok(CgValue::unit());
        }

        let gv = self.declare_top_level_var_global(var)?;
        let llvm_ty = self.llvm_basic_type_of(span, cg_ty)?;
        let loaded =
            self.builder
                .build_load(llvm_ty, gv.as_pointer_value(), "load_top_level_var")?;

        Ok(match cg_ty {
            CgTy::Bool => CgValue::bool(loaded.into_int_value()),
            CgTy::Float64 | CgTy::Float32 => CgValue::float(loaded.into_float_value(), cg_ty),
            CgTy::Int(int_ty) => CgValue::int(loaded.into_int_value(), int_ty),
            CgTy::String => CgValue {
                ty: CgTy::String,
                value: Some(loaded.into_pointer_value().into()),
            },
            CgTy::Ref => CgValue {
                ty: CgTy::Ref,
                value: Some(loaded.into_pointer_value().into()),
            },
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => CgValue {
                ty: cg_ty,
                value: Some(loaded),
            },
            CgTy::Unit => CgValue::unit(),
            CgTy::Never => CgValue::never(),
        })
    }

    fn codegen_extern_global_access(
        &mut self,
        span: crate::span::Span,
        global: &hir::ExternGlobal,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let cg_ty = self
            .cg_ty_of(global.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "extern global type",
                at: global.span.into(),
            })?;
        if cg_ty == CgTy::Unit {
            return Ok(CgValue::unit());
        }
        let gv = self.declare_extern_global(global)?;
        let llvm_ty = self.llvm_basic_type_of(span, cg_ty)?;
        let loaded =
            self.builder
                .build_load(llvm_ty, gv.as_pointer_value(), "load_extern_global")?;
        self.cg_value_from_loaded(span, cg_ty, loaded)
    }

    fn codegen_mir_extern_global_access(
        &mut self,
        span: crate::span::Span,
        global: &crate::mir::ExternGlobalRoot,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let cg_ty = self
            .cg_ty_of(global.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "extern global type",
                at: global.span.into(),
            })?;
        if cg_ty == CgTy::Unit {
            return Ok(CgValue::unit());
        }
        let gv = self.declare_mir_extern_global(global)?;
        let llvm_ty = self.llvm_basic_type_of(span, cg_ty)?;
        let loaded =
            self.builder
                .build_load(llvm_ty, gv.as_pointer_value(), "load_extern_global")?;
        self.cg_value_from_loaded(span, cg_ty, loaded)
    }

    fn register_global_root_if_needed(
        &mut self,
        at: crate::span::Span,
        global: GlobalValue<'ctx>,
        global_name: &str,
        storage_ty: BasicTypeEnum<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let Some(type_desc) =
            self.get_or_create_global_root_type_desc_global(at, global_name, storage_ty)?
        else {
            return Ok(());
        };

        let rt_register = self.declare_runtime_gc_register_global_root();
        let _ = self.builder.build_call(
            rt_register,
            &[
                global.as_pointer_value().into(),
                type_desc.as_pointer_value().into(),
            ],
            "gc_register_global_root",
        )?;
        Ok(())
    }

    #[allow(dead_code)]
    fn build_fun_callee_suspend_plan(&self, fun: &hir::FunDecl) -> Option<CalleeSuspendPlan> {
        self.build_fun_callee_suspend_plan_impl(fun)
    }

    /// Create the shared function-level return context used by ordinary frames.
    fn setup_function_return_context(
        &mut self,
        at: crate::span::Span,
        llvm_fun: FunctionValue<'ctx>,
        declared_return_cg: CgTy,
    ) -> Result<
        (
            inkwell::basic_block::BasicBlock<'ctx>,
            Option<inkwell::values::PointerValue<'ctx>>,
        ),
        LlvmEmitError,
    > {
        let return_bb = self.context.append_basic_block(llvm_fun, "return");
        let return_alloca = match declared_return_cg {
            CgTy::Unit | CgTy::Never => None,
            _ => Some(self.builder.build_alloca(
                self.llvm_basic_type_of(at, declared_return_cg)?,
                "return_val",
            )?),
        };
        self.function_cx.return_context = Some(ReturnContext {
            return_bb,
            return_alloca,
        });
        Ok((return_bb, return_alloca))
    }

    /// Emit the shared return block terminator after body/resume paths branch into it.
    fn emit_function_return_block(
        &mut self,
        at: crate::span::Span,
        declared_return_cg: CgTy,
        return_bb: inkwell::basic_block::BasicBlock<'ctx>,
        return_alloca: Option<inkwell::values::PointerValue<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        self.builder.position_at_end(return_bb);
        match declared_return_cg {
            CgTy::Unit => {
                self.builder.build_return(None)?;
            }
            CgTy::Never => {
                self.builder.build_unreachable()?;
            }
            _ => {
                let alloca = return_alloca.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "return alloca",
                    at: at.into(),
                })?;
                let loaded = self.builder.build_load(
                    self.llvm_basic_type_of(at, declared_return_cg)?,
                    alloca,
                    "ret_load",
                )?;
                let ret_v = self.cg_value_from_loaded(at, declared_return_cg, loaded)?;
                self.emit_return(at, declared_return_cg, ret_v)?;
            }
        }
        self.function_cx.return_context = None;
        Ok(())
    }

    fn finish_function_return_path(
        &mut self,
        at: crate::span::Span,
        declared_return_cg: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        if self
            .builder
            .get_insert_block()
            .is_some_and(|bb| bb.get_terminator().is_some())
        {
            return Ok(());
        }

        if let Some(return_ctx) = self.function_cx.return_context {
            if let Some(alloca) = return_ctx.return_alloca
                && let Some(raw) = value.value
            {
                self.builder.build_store(alloca, raw)?;
            }
            self.builder
                .build_unconditional_branch(return_ctx.return_bb)?;
            return Ok(());
        }

        self.emit_return(at, declared_return_cg, value)
    }

    fn codegen_callee_resume_dispatch(
        &mut self,
        at: crate::span::Span,
        llvm_fun: FunctionValue<'ctx>,
        plan: &CalleeSuspendPlan,
        base_env: &Env<'ctx>,
        declared_return_cg: CgTy,
        incoming_resume_token: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        self.codegen_callee_resume_dispatch_impl(
            at,
            llvm_fun,
            plan,
            base_env,
            declared_return_cg,
            incoming_resume_token,
        )
    }

    fn codegen_callee_resume_entry_function(
        &mut self,
        at: crate::span::Span,
        resume_fun: FunctionValue<'ctx>,
        plan: &CalleeSuspendPlan,
        declared_return_cg: CgTy,
    ) -> Result<(), LlvmEmitError> {
        self.codegen_callee_resume_entry_function_impl(at, resume_fun, plan, declared_return_cg)
    }

    #[allow(dead_code)]
    pub(crate) fn codegen_top_level_fun(
        mut self,
        fun: &hir::FunDecl,
        llvm_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let Some(body) = fun.body.as_ref() else {
            // extern / declaration-only：由调用点按需声明即可，这里不生成 body。
            return Ok(());
        };

        self.current_source_id = self.source_id_for_path(fun.source_path.as_path(), fun.span)?;
        self.function_cx.current_callable_fqn = Some(fun.fqn.clone());

        let entry = self.context.append_basic_block(llvm_fun, "entry");
        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(llvm_fun)?;

        let Some(declared_return_cg) = self.cg_ty_of(fun.return_ty) else {
            tracing::warn!(
                "codegen_top_level_fun: unsupported return type for {} -> {}",
                fun.fqn,
                self.types.display(fun.return_ty)
            );
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "function return type",
                at: fun.span.into(),
            });
        };
        self.function_cx.current_fun_return_ty = Some(declared_return_cg);
        let uses_hidden_sret = self
            .hidden_sret_result_ty(fun.span, declared_return_cg)?
            .is_some();
        let uses_explicit_effect_hidden_abi =
            self.callable_uses_explicit_effect_hidden_abi(&fun.fqn);
        self.function_cx.current_sret_return_ptr = if uses_hidden_sret {
            Some(
                llvm_fun
                    .get_nth_param(0)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "missing llvm function sret param",
                        at: fun.span.into(),
                    })?
                    .into_pointer_value(),
            )
        } else {
            None
        };
        self.bind_explicit_effect_hidden_abi_slots(
            fun.span,
            llvm_fun,
            u32::from(uses_hidden_sret),
            uses_explicit_effect_hidden_abi,
        )?;

        self.function_cx.env.push_scope();
        self.codegen_fun_params(
            fun,
            llvm_fun,
            u32::from(uses_hidden_sret)
                + self.explicit_effect_hidden_abi_param_count(uses_explicit_effect_hidden_abi),
        )?;

        // T0141: Set up function-level return context for early return support.
        // The return slot lives in the entry block before body codegen.
        let (return_bb, return_alloca) =
            self.setup_function_return_context(fun.span, llvm_fun, declared_return_cg)?;

        let callee_suspend_plan = self.build_fun_callee_suspend_plan(fun);
        let callee_resume_entry_fn = if callee_suspend_plan.is_some() {
            Some(self.declare_top_level_fun_callee_resume_entry(fun)?)
        } else {
            None
        };
        let ret_v = if let Some(plan) = callee_suspend_plan.as_ref() {
            self.with_callee_suspend_lowering(Some(plan.clone()), callee_resume_entry_fn, |cg| {
                cg.codegen_block_as_return_value(body, declared_return_cg)
            })?
        } else {
            self.codegen_block_as_return_value(body, declared_return_cg)?
        };
        self.finish_function_return_path(fun.span, declared_return_cg, ret_v)?;

        self.emit_function_return_block(fun.span, declared_return_cg, return_bb, return_alloca)?;
        self.finish_function_explicit_frame_layout(fun.span)?;
        if let (Some(plan), Some(resume_fun)) =
            (callee_suspend_plan.as_ref(), callee_resume_entry_fn)
        {
            self.codegen_callee_resume_entry_function(
                fun.span,
                resume_fun,
                plan,
                declared_return_cg,
            )?;
        }
        self.clear_explicit_effect_hidden_abi_slots();
        self.function_cx.current_sret_return_ptr = None;
        self.function_cx.env.pop_scope();
        Ok(())
    }

    // 表达式/语句/控制流 codegen 已拆分到子模块（T0102d）。

    fn codegen_call(
        &mut self,
        span: crate::span::Span,
        callee: &hir::Expr,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
        // T0125：call expression 的结果 TypeId（用于泛型 class ctor 的 mangled FQN 查找）。
        result_ty: Option<TypeId>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_call_impl(span, callee, args, expected, result_ty)
    }

    // 原子 intrinsics 需要“真实可寻址的槽位地址”，不能先把 member access 降成 rvalue load。
    fn codegen_addressable_place(
        &mut self,
        expr: &hir::Expr,
    ) -> Result<AddressablePlace<'ctx>, LlvmEmitError> {
        match &expr.kind {
            hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => {
                let local =
                    self.function_cx
                        .env
                        .get(*id)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "atomicInt lvalue local",
                            at: expr.span.into(),
                        })?;

                let ptr = self.local_ptr_for_use(expr.span, local, "atomic_int_slot")?;
                Ok(AddressablePlace {
                    ptr,
                    ty: local.ty,
                    writable: local.mutable,
                })
            }
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => {
                if let Some(global) = self.materialized_extern_global_root(fqn).cloned() {
                    let gv = self.declare_mir_extern_global(&global)?;
                    let cg_ty =
                        self.cg_ty_of(global.ty)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "atomicInt extern global type",
                                at: expr.span.into(),
                            })?;
                    return Ok(AddressablePlace {
                        ptr: gv.as_pointer_value(),
                        ty: cg_ty,
                        writable: global.mutable,
                    });
                }

                if let Some(global) = self.extern_globals.get(fqn).cloned() {
                    let gv = self.declare_extern_global(&global)?;
                    let cg_ty =
                        self.cg_ty_of(global.ty)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "atomicInt extern global type",
                                at: expr.span.into(),
                            })?;
                    return Ok(AddressablePlace {
                        ptr: gv.as_pointer_value(),
                        ty: cg_ty,
                        writable: global.mutable,
                    });
                }

                let Some(var) = self.top_level_vars.get(fqn) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicInt lvalue top-level var",
                        at: expr.span.into(),
                    });
                };

                let gv = self.declare_top_level_var_global(var)?;
                let cg_ty = self
                    .cg_ty_of(var.ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicInt top-level var type",
                        at: expr.span.into(),
                    })?;
                Ok(AddressablePlace {
                    ptr: gv.as_pointer_value(),
                    ty: cg_ty,
                    writable: true,
                })
            }
            hir::ExprKind::MemberAccess { receiver, member } => {
                let Some(hir::MemberRef::Value { fqn, .. }) = member.resolved.as_ref() else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicInt target must be an lvalue",
                        at: expr.span.into(),
                    });
                };

                let receiver_hir_ty = self
                    .resolve_expr_concrete_type(receiver)
                    .unwrap_or(receiver.ty);
                if let Some((class, field_idx, field_cg)) =
                    self.lookup_class_field_by_fqn(fqn, member.span, Some(receiver_hir_ty))?
                {
                    let field = class.fields.get(field_idx as usize).ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "class field index",
                            at: member.span.into(),
                        },
                    )?;
                    let recv = self.codegen_expr_in_expected_context(receiver, Some(CgTy::Ref))?;
                    let recv = self.coerce_value(receiver.span, recv, CgTy::Ref)?;
                    let Some(raw) = recv.value else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "class field receiver value",
                            at: receiver.span.into(),
                        });
                    };
                    let BasicValueEnum::PointerValue(obj_ptr) = raw else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "class field receiver type",
                            at: receiver.span.into(),
                        });
                    };

                    let ptr =
                        self.codegen_class_field_ptr(member.span, &class, obj_ptr, field_idx)?;
                    return Ok(AddressablePlace {
                        ptr,
                        ty: field_cg,
                        writable: field.mutable,
                    });
                }

                let base = self.codegen_addressable_place(receiver)?;
                let CgTy::Struct(struct_ty) = base.ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicInt target must be an lvalue",
                        at: expr.span.into(),
                    });
                };

                let (field_idx, field_ty) =
                    self.lookup_struct_field(struct_ty, fqn, member.span)?;
                let llvm_struct_ty = self.llvm_struct_type(member.span, struct_ty)?;
                let ptr = self.builder.build_struct_gep(
                    llvm_struct_ty,
                    base.ptr,
                    field_idx,
                    "atomic_int_field_gep",
                )?;
                Ok(AddressablePlace {
                    ptr,
                    ty: field_ty,
                    writable: base.writable,
                })
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "atomicInt target must be an lvalue",
                at: expr.span.into(),
            }),
        }
    }

    /// T0127: 从 HIR 表达式中尽量提取具体（非 Any/Param）的 TypeId。
    ///
    /// 对于字面量表达式直接返回其 HIR type；对于变量引用尝试从 env 中获取 hir_ty。
    /// T0130: 对于 Call 表达式，尝试通过 callee 的已知签名推导返回类型。
    fn resolve_expr_concrete_type(&self, expr: &hir::Expr) -> Option<crate::ty::TypeId> {
        ExprFactResolver::new(self.types, self.program_facts.as_ref(), |id| {
            self.function_cx.env.get(id).and_then(|local| local.hir_ty)
        })
        .resolve_expr_concrete_type(expr)
    }

    fn maybe_record_active_suspend_site_effect_outcome(
        &mut self,
        call_span: crate::span::Span,
        outcome_slot: PointerValue<'ctx>,
    ) {
        if let Some(capture) = self.effect_cx.suspend_site_effect_outcomes.active_capture
            && (capture.capture_any || capture.call_span == call_span)
        {
            self.effect_cx
                .suspend_site_effect_outcomes
                .explicit_outcomes
                .insert(capture.site_id, outcome_slot);
        }
    }

    fn codegen_top_level_fun_call(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_top_level_fun_call_impl(span, callee_span, fqn, args)
    }

    /// 为 `@Extern` 调用点生成 `scoop_enter_native(root_slots, len)`。
    ///
    /// 设计取舍（v0）：
    /// - 这里采用保守策略：把当前 scope 中所有 `Ref/String` locals 的栈槽地址作为 roots slots；
    /// - 这样可以保证 GC 在 native 期间能扫描/更新这些 slots（moving GC 也可写回更新后的指针）；
    /// - 代价是可能多保活一些对象（但不会漏 roots）。
    fn emit_enter_native_for_extern_call(
        &mut self,
        at: crate::span::Span,
    ) -> Result<(), LlvmEmitError> {
        self.emit_enter_native_for_extern_call_impl(at)
    }

    fn emit_extern_native_call(
        &mut self,
        at: crate::span::Span,
        fqn: &str,
        llvm_fun: FunctionValue<'ctx>,
        llvm_args: &[inkwell::values::BasicMetadataValueEnum<'ctx>],
    ) -> Result<CallSiteValue<'ctx>, LlvmEmitError> {
        self.emit_extern_native_call_impl(at, fqn, llvm_fun, llvm_args)
    }

    fn try_codegen_class_vtable_call(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        self.try_codegen_class_vtable_call_impl(span, callee_span, fqn, args)
    }

    fn try_codegen_interface_itable_call(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        self.try_codegen_interface_itable_call_impl(span, callee_span, fqn, args)
    }

    fn load_class_vtable_slot_fn_ptr_i8(
        &mut self,
        _at: crate::span::Span,
        receiver: PointerValue<'ctx>,
        slot: u32,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        self.load_class_vtable_slot_fn_ptr_i8_impl(_at, receiver, slot)
    }

    fn llvm_scoop_itable_entry_type(&self) -> StructType<'ctx> {
        self.llvm_scoop_itable_entry_type_impl()
    }

    fn llvm_scoop_itable_type(&self) -> StructType<'ctx> {
        self.llvm_scoop_itable_type_impl()
    }

    fn load_interface_itable_slot_fn_ptr_i8(
        &mut self,
        at: crate::span::Span,
        receiver: PointerValue<'ctx>,
        interface_id: u64,
        slot: u32,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        self.load_interface_itable_slot_fn_ptr_i8_impl(at, receiver, interface_id, slot)
    }

    fn codegen_funptr_value_call(
        &mut self,
        funptr_addr: inkwell::values::IntValue<'ctx>,
        funptr_int_ty: IntTy,
        call: CallableValueCallSpec<'_>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_funptr_value_call_impl(funptr_addr, funptr_int_ty, call)
    }

    /// 把调用点上的 positional/named HIR 实参映射为 `arg_idx -> param_idx`。
    ///
    /// 约束与前端 typecheck 保持一致：
    /// - 一旦出现命名实参，后续不能再出现位置实参；
    /// - 所有命名都必须命中形参；
    /// - 每个形参必须且只能被一个显式实参提供。
    fn map_call_args_to_params_by_name(
        &self,
        param_names: &[String],
        args: &[hir::CallArg],
    ) -> Option<Vec<usize>> {
        self.map_call_args_to_params_by_name_impl(param_names, args)
    }

    /// 在保持源码求值顺序的前提下，把调用点实参求值并归位为"按形参顺序排列"的 LLVM 实参。
    fn codegen_bound_call_args(
        &mut self,
        spec: BoundCallArgsSpec,
        param_names: &[String],
        param_tys: &[TypeId],
        args: &[hir::CallArg],
    ) -> Result<Vec<EvaluatedCallArg<'ctx>>, LlvmEmitError> {
        self.codegen_bound_call_args_impl(spec, param_names, param_tys, args)
    }

    fn callable_value_param_names(&self, fun_ty: &crate::ty::FunctionType) -> Vec<String> {
        self.callable_value_param_names_impl(fun_ty)
    }

    fn callable_value_param_tys(&self, fun_ty: &crate::ty::FunctionType) -> Vec<TypeId> {
        self.callable_value_param_tys_impl(fun_ty)
    }

    fn codegen_callable_value_args(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fun_ty: &crate::ty::FunctionType,
        args: &[hir::CallArg],
        kind: &'static str,
        abi_mode: CallArgAbiMode,
    ) -> Result<Vec<EvaluatedCallArg<'ctx>>, LlvmEmitError> {
        self.codegen_callable_value_args_impl(span, callee_span, fun_ty, args, kind, abi_mode)
    }

    fn codegen_function_value_call(
        &mut self,
        local: &CgLocal<'ctx>,
        call: CallableValueCallSpec<'_>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_function_value_call_impl(local, call)
    }

    fn codegen_function_value_call_from_closure_obj(
        &mut self,
        closure_obj_i8: PointerValue<'ctx>,
        call: CallableValueCallSpec<'_>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_function_value_call_from_closure_obj_impl(closure_obj_i8, call)
    }

    // 控制流 codegen（if/when 等）已拆分到子模块（T0102d）。

    fn llvm_param_ty(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
    ) -> Result<BasicMetadataTypeEnum<'ctx>, LlvmEmitError> {
        self.llvm_param_ty_impl(span, ty)
    }

    fn ordinary_param_abi(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
    ) -> Result<OrdinaryParamAbi<'ctx>, LlvmEmitError> {
        self.ordinary_param_abi_impl(span, ty)
    }

    fn callable_uses_explicit_effect_hidden_abi(&self, callable_fqn: &str) -> bool {
        self.callable_uses_explicit_effect_hidden_abi_impl(callable_fqn)
    }

    fn callable_needs_callee_resume_shell(&self, callable_fqn: &str) -> bool {
        self.callable_needs_callee_resume_shell_impl(callable_fqn)
    }

    fn published_callable_signature(&self, callable_fqn: &str) -> Option<(Vec<TypeId>, TypeId)> {
        self.published_callable_signature_impl(callable_fqn)
    }

    fn explicit_effect_hidden_abi_param_count(&self, uses_explicit_effect_hidden_abi: bool) -> u32 {
        self.explicit_effect_hidden_abi_param_count_impl(uses_explicit_effect_hidden_abi)
    }

    fn push_explicit_effect_hidden_abi_param_tys(
        &self,
        llvm_params: &mut Vec<BasicMetadataTypeEnum<'ctx>>,
    ) {
        self.push_explicit_effect_hidden_abi_param_tys_impl(llvm_params)
    }

    fn bind_explicit_effect_hidden_abi_slots(
        &mut self,
        at: crate::span::Span,
        llvm_fun: FunctionValue<'ctx>,
        first_hidden_param_index: u32,
        uses_explicit_effect_hidden_abi: bool,
    ) -> Result<(), LlvmEmitError> {
        self.bind_explicit_effect_hidden_abi_slots_impl(
            at,
            llvm_fun,
            first_hidden_param_index,
            uses_explicit_effect_hidden_abi,
        )
    }

    fn clear_explicit_effect_hidden_abi_slots(&mut self) {
        self.clear_explicit_effect_hidden_abi_slots_impl()
    }

    #[allow(dead_code)]
    fn build_ordinary_callee_suspend_plan(
        &self,
        body: &hir::Block,
        declared_return_ty: TypeId,
    ) -> Option<CalleeSuspendPlan> {
        self.build_ordinary_callee_suspend_plan_impl(body, declared_return_ty)
    }

    fn hir_ty_declared_effectful(&self, hir_ty: Option<TypeId>) -> bool {
        self.hir_ty_declared_effectful_impl(hir_ty)
    }

    fn local_call_may_suspend_from_hir_ty(&self, hir_ty: Option<TypeId>) -> bool {
        self.local_call_may_suspend_from_hir_ty_impl(hir_ty)
    }

    fn known_fun_body_may_outward_effect(&self, fqn: &str, declared_fun_ty: TypeId) -> bool {
        self.known_fun_body_may_outward_effect_impl(fqn, declared_fun_ty)
    }

    fn function_value_expr_body_may_outward_effect_when_called_for_local(
        &self,
        expr: &hir::Expr,
    ) -> bool {
        self.function_value_expr_body_may_outward_effect_when_called_for_local_impl(expr)
    }

    fn type_contains_gc_refs(&self, ty: TypeId, visiting: &mut HashSet<TypeId>) -> bool {
        if !visiting.insert(ty) {
            return false;
        }

        let contains = match self.types.kind(ty) {
            TypeKind::Ref(_) => true,
            TypeKind::StarProjection(star) => self.type_contains_gc_refs(star.read_ty, visiting),
            TypeKind::Param(_) => true,
            TypeKind::Value(kind) => match kind {
                ValueTypeKind::Unit
                | ValueTypeKind::Nothing
                | ValueTypeKind::Bool
                | ValueTypeKind::Char
                | ValueTypeKind::Float64
                | ValueTypeKind::Float32
                | ValueTypeKind::Int
                | ValueTypeKind::UInt
                | ValueTypeKind::IntN(_)
                | ValueTypeKind::UIntN(_) => false,
                ValueTypeKind::Option(inner) => self.type_contains_gc_refs(*inner, visiting),
                ValueTypeKind::Tuple(elements) => elements
                    .iter()
                    .copied()
                    .any(|elem| self.type_contains_gc_refs(elem, visiting)),
                ValueTypeKind::Nominal(nominal) => {
                    let key = self.nominal_layout_key(nominal);
                    if let Some(layout) = self.struct_layouts.get(&key) {
                        layout.fields.iter().any(|field| {
                            field.ty.is_some_and(|field_ty| {
                                self.type_contains_gc_refs(field_ty, visiting)
                            })
                        })
                    } else if let Some(layout) = self.enum_layouts.get(&key) {
                        layout.variants.iter().any(|variant| {
                            variant.fields.iter().any(|field| {
                                field.ty.is_some_and(|field_ty| {
                                    self.type_contains_gc_refs(field_ty, visiting)
                                })
                            })
                        })
                    } else {
                        false
                    }
                }
            },
        };

        visiting.remove(&ty);
        contains
    }

    fn cg_value_from_llvm_param(
        &self,
        at: crate::span::Span,
        llvm_fun: FunctionValue<'ctx>,
        param_index: u32,
        target_ty: CgTy,
        missing_kind: &'static str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.cg_value_from_llvm_param_impl(at, llvm_fun, param_index, target_ty, missing_kind)
    }

    fn bind_ordinary_param_local(
        &mut self,
        binding: OrdinaryParamLocalBinding<'ctx, '_>,
    ) -> Result<(), LlvmEmitError> {
        self.bind_ordinary_param_local_impl(binding)
    }

    fn materialize_deferred_cg_value_for_call_arg(
        &mut self,
        at: crate::span::Span,
        name: &str,
        value: DeferredCgValue<'ctx>,
    ) -> Result<(CgValue<'ctx>, Vec<DeferredGcSensitiveSpill<'ctx>>), LlvmEmitError> {
        self.materialize_deferred_cg_value_for_call_arg_impl(at, name, value)
    }

    fn deferred_gc_spill_slot_for_call_arg(
        &mut self,
        at: crate::span::Span,
        name: &str,
        value: DeferredCgValue<'ctx>,
    ) -> Result<(PointerValue<'ctx>, Vec<DeferredGcSensitiveSpill<'ctx>>), LlvmEmitError> {
        self.deferred_gc_spill_slot_for_call_arg_impl(at, name, value)
    }

    fn release_evaluated_call_arg_roots(&mut self, args: &[EvaluatedCallArg<'ctx>]) {
        self.release_evaluated_call_arg_roots_impl(args)
    }

    fn as_llvm_arg_value(
        &self,
        span: crate::span::Span,
        param_ty: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<inkwell::values::BasicMetadataValueEnum<'ctx>, LlvmEmitError> {
        self.as_llvm_arg_value_impl(span, param_ty, value)
    }

    #[allow(dead_code)]
    fn codegen_fun_params(
        &mut self,
        fun: &hir::FunDecl,
        llvm_fun: FunctionValue<'ctx>,
        param_offset: u32,
    ) -> Result<(), LlvmEmitError> {
        for (idx, param) in fun.params.iter().enumerate() {
            self.bind_ordinary_param_local(OrdinaryParamLocalBinding {
                at: param.span,
                llvm_fun,
                param_index: idx as u32 + param_offset,
                name: &param.name,
                id: param.id,
                ty_id: param.ty,
                call_may_suspend: self.local_call_may_suspend_from_hir_ty(Some(param.ty)),
                missing_kind: "missing llvm param",
            })?;
        }
        Ok(())
    }

    fn default_value(
        &mut self,
        at: crate::span::Span,
        ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match ty {
            CgTy::Unit => Ok(CgValue::unit()),
            // T1612: Nothing/Never has no runtime value.
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                let llvm_ty = self.llvm_basic_type_of(at, ty)?;
                let raw = self.zero_initializer_for_basic_type(llvm_ty);
                self.cg_value_from_loaded(at, ty, raw)
            }
        }
    }

    fn declare_libc_exit(&self) -> FunctionValue<'ctx> {
        if let Some(f) = self.module.get_function("exit") {
            return f;
        }
        let fn_ty = self
            .context
            .void_type()
            .fn_type(&[self.context.i32_type().into()], false);
        self.declare_runtime_or_native_import_function("exit", fn_ty)
    }

    fn declare_libc_malloc(&self) -> FunctionValue<'ctx> {
        if let Some(f) = self.module.get_function("malloc") {
            return f;
        }

        // `void* malloc(size_t size)`：这里用 `i64` 作为 size（host 64-bit 场景；32-bit 下会被 truncate）。
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let size_ty = self.context.i64_type();
        let fn_ty = i8_ptr_ty.fn_type(&[size_ty.into()], false);
        self.declare_runtime_or_native_import_function("malloc", fn_ty)
    }

    fn emit_exit_with_code(
        &mut self,
        at: crate::span::Span,
        code: i32,
    ) -> Result<(), LlvmEmitError> {
        let exit = self.declare_libc_exit();
        let code_i32 = self.context.i32_type().const_int(code as u64, false);
        let _ = self.builder.build_call(exit, &[code_i32.into()], "exit")?;
        self.builder.build_unreachable()?;
        let _ = at;
        Ok(())
    }

    /// T0141: Codegen an early `return` from inside a nested block or loop.
    /// Stores the return value into the function-level return alloca and branches to the return BB.
    pub(super) fn codegen_early_return(
        &mut self,
        span: crate::span::Span,
        value: Option<&hir::Expr>,
    ) -> Result<(), LlvmEmitError> {
        let return_ctx =
            self.function_cx
                .return_context
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "return outside function with return context",
                    at: span.into(),
                })?;
        let declared_return_cg = self.function_cx.current_fun_return_ty.unwrap_or(CgTy::Unit);

        match value {
            Some(expr) => {
                let v = self.codegen_expr_in_expected_context(expr, Some(declared_return_cg))?;
                if let Some(alloca) = return_ctx.return_alloca {
                    let coerced = self.coerce_value(expr.span, v, declared_return_cg)?;
                    if let Some(raw) = coerced.value {
                        self.builder.build_store(alloca, raw)?;
                    }
                }
            }
            None => {
                // `return` without value — for Unit functions, no store needed.
            }
        }

        self.builder
            .build_unconditional_branch(return_ctx.return_bb)?;
        Ok(())
    }

    fn emit_return(
        &mut self,
        span: crate::span::Span,
        declared_return_ty: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        match declared_return_ty {
            CgTy::Unit => {
                self.builder.build_return(None)?;
                Ok(())
            }
            // T1612: A function declared as returning Nothing never returns normally.
            // Emit `unreachable` instead of a return instruction.
            CgTy::Never => {
                self.builder.build_unreachable()?;
                Ok(())
            }
            CgTy::Bool
            | CgTy::Float64
            | CgTy::Float32
            | CgTy::Int(_)
            | CgTy::String
            | CgTy::Ref
            | CgTy::Tuple(_)
            | CgTy::Struct(_)
            | CgTy::Enum(_) => {
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: if self.function_cx.current_sret_return_ptr.is_some() {
                            "sret return value"
                        } else {
                            "return value"
                        },
                        at: span.into(),
                    });
                };
                if let Some(sret_ptr) = self.function_cx.current_sret_return_ptr
                    && matches!(
                        declared_return_ty,
                        CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_)
                    )
                {
                    let _ = self.builder.build_store(sret_ptr, raw)?;
                    self.builder.build_return(None)?;
                } else {
                    self.builder.build_return(Some(&raw))?;
                }
                Ok(())
            }
        }
    }

    fn codegen_literal(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
        lit: &hir::LiteralKind,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match lit {
            hir::LiteralKind::Unit => Ok(CgValue::unit()),
            hir::LiteralKind::Bool(v) => Ok(CgValue::bool(
                self.context.bool_type().const_int(*v as u64, false),
            )),
            hir::LiteralKind::Char(value) => Ok(CgValue::int(
                self.context.i32_type().const_int(*value as u64, false),
                IntTy {
                    bits: 32,
                    signed: false,
                },
            )),
            hir::LiteralKind::Int => {
                let Some(CgTy::Int(int_ty)) = self.cg_ty_of(ty) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "int literal type",
                        at: span.into(),
                    });
                };
                let value = self.int_literal_bits_for_ty(span, int_ty)?;
                Ok(CgValue::int(
                    self.int_type(int_ty).const_int(value, false),
                    int_ty,
                ))
            }
            hir::LiteralKind::Float64(value) => Ok(CgValue::float(
                self.context.f64_type().const_float(*value),
                CgTy::Float64,
            )),
            hir::LiteralKind::Float32(value) => Ok(CgValue::float(
                self.context.f32_type().const_float(f64::from(*value)),
                CgTy::Float32,
            )),
            hir::LiteralKind::String => self.codegen_string_literal(span),
            hir::LiteralKind::SynthInt(value) => {
                // Synthesized integer literal from compiler desugaring (T0110).
                let int_ty = IntTy {
                    bits: 64,
                    signed: true,
                };
                Ok(CgValue::int(
                    self.int_type(int_ty).const_int(*value as u64, false),
                    int_ty,
                ))
            }
        }
    }

    /// Emit LLVM IR for a string literal by parsing the current source text on demand.
    fn codegen_string_literal(
        &mut self,
        span: crate::span::Span,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let bytes = self.parse_current_string_literal_bytes(span)?;
        self.codegen_string_literal_from_bytes(span, &bytes)
    }

    fn codegen_string_literal_from_text(
        &mut self,
        span: crate::span::Span,
        text: &str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_string_literal_from_bytes(span, text.as_bytes())
    }

    /// Emit LLVM IR for a string literal from already parsed bytes.
    fn codegen_string_literal_from_bytes(
        &mut self,
        span: crate::span::Span,
        bytes: &[u8],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 1) 分配一个 GC-managed `ScoopString` 对象：
        //    - LLVM 侧类型为 `ScoopString addrspace(1)*`
        //    - 分配通过 `scoop_alloc_typed(desc, sizeof(ScoopString))`（runtime 写入对象头 type_desc）
        let scoop_str_ty = self.llvm_scoop_string_type();
        let obj_size = self.target_data.get_store_size(&scoop_str_ty);
        let size_v = self.context.i64_type().const_int(obj_size, false);

        let str_desc = self.get_or_create_string_type_desc_global(span)?;
        let str_desc_i8 = self.builder.build_pointer_cast(
            str_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "str_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_alloc,
            &[str_desc_i8.into(), size_v.into()],
            "rt_alloc_string_lit",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(raw_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return type",
                at: span.into(),
            });
        };

        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let str_ptr = self
            .builder
            .build_pointer_cast(raw_ptr, str_ptr_ty, "str_obj_ptr")?;

        // 2) 写入 `{ len, data }`。
        let len_ptr = self
            .builder
            .build_struct_gep(scoop_str_ty, str_ptr, 1, "str_len_gep")?;
        let data_ptr = self
            .builder
            .build_struct_gep(scoop_str_ty, str_ptr, 2, "str_data_gep")?;

        let len = self.context.i64_type().const_int(bytes.len() as u64, false);
        let _ = self.builder.build_store(len_ptr, len)?;

        // 空串：保持 `data = NULL`（与 runtime 侧空串约定一致）。
        if bytes.is_empty() {
            let i8_ptr_ty = self.llvm_i8_ptr_type();
            let _ = self.builder.build_store(data_ptr, i8_ptr_ty.const_null())?;
        } else {
            // 把字节序列落到一个只读全局常量：`[N x i8] @__scoop_str_data_*`
            let data_gv = self.get_or_create_global_bytes(span, bytes);
            let i8_ptr_ty = self.llvm_i8_ptr_type();
            let data_i8_ptr = self.builder.build_pointer_cast(
                data_gv.as_pointer_value(),
                i8_ptr_ty,
                "str_data_ptr",
            )?;
            let _ = self.builder.build_store(data_ptr, data_i8_ptr)?;
        }

        Ok(CgValue {
            ty: CgTy::String,
            value: Some(str_ptr.into()),
        })
    }

    fn load_scoop_string_len_and_data(
        &mut self,
        str_obj_ptr: PointerValue<'ctx>,
    ) -> Result<(IntValue<'ctx>, PointerValue<'ctx>), LlvmEmitError> {
        let scoop_str_ty = self.llvm_scoop_string_type();
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        let len_ptr =
            self.builder
                .build_struct_gep(scoop_str_ty, str_obj_ptr, 1, "str_len_gep_interp")?;
        let data_ptr =
            self.builder
                .build_struct_gep(scoop_str_ty, str_obj_ptr, 2, "str_data_gep_interp")?;

        let len = self
            .builder
            .build_load(i64_ty, len_ptr, "str_len_interp")?
            .into_int_value();
        let data = self
            .builder
            .build_load(i8_ptr_ty, data_ptr, "str_data_interp")?
            .into_pointer_value();

        Ok((len, data))
    }

    fn codegen_interpolated_string(
        &mut self,
        span: crate::span::Span,
        raw: bool,
        parts: &[hir::InterpolatedStringPart],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 当前阶段的落点：把 f-string 分片后"拼接"为一段连续 UTF-8 字节序列，
        // 返回一个 GC-managed `ScoopString` 对象（addrspace(1)），其 `data` 指向 `malloc` 的 bytes buffer。
        //
        // 约束（与 TODO T0823 对齐）：
        // - 目前支持 `{Bool}` / `{Char}` / `{Int}` / `{String}` / `{Float}`；
        // - 先不支持 format spec / locale；
        // - 当前阶段不接入 type descriptor/release：`data` 的释放留给后续任务补齐（T1507/T1514）。

        #[derive(Clone, Copy)]
        struct Segment<'ctx> {
            ptr: PointerValue<'ctx>,
            len: IntValue<'ctx>,
        }

        let i64_ty = self.context.i64_type();
        let i8_ty = self.context.i8_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let scoop_str_ty = self.llvm_scoop_string_type();

        // 1) 先做一遍：收集所有片段的 (ptr, len)，并计算总长度（运行期）。
        let mut segments: Vec<Segment<'ctx>> = Vec::new();
        let mut total_len = i64_ty.const_zero();

        for part in parts {
            match part {
                hir::InterpolatedStringPart::Text { span: text_span } => {
                    let text = self.current_source_slice(*text_span)?;
                    let bytes = parse_f_string_text_bytes(raw, text).map_err(|_| {
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "invalid interpolated string text",
                            at: (*text_span).into(),
                        }
                    })?;

                    let gv = self.get_or_create_global_bytes(*text_span, &bytes);
                    let ptr = self.builder.build_pointer_cast(
                        gv.as_pointer_value(),
                        i8_ptr_ty,
                        "fstr_text_ptr",
                    )?;
                    let len = i64_ty.const_int(bytes.len() as u64, false);

                    segments.push(Segment { ptr, len });
                    total_len = self
                        .builder
                        .build_int_add(total_len, len, "fstr_total_len")?;
                }
                hir::InterpolatedStringPart::Expr { expr } => {
                    if self.expr_is_builtin_char(expr) {
                        let str_v = self.codegen_char_method_to_string(expr.span, expr)?;
                        let Some(raw) = str_v.value else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "string interpolation char expr value",
                                at: expr.span.into(),
                            });
                        };
                        let BasicValueEnum::PointerValue(str_obj_ptr) = raw else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "string interpolation char expr type",
                                at: expr.span.into(),
                            });
                        };

                        let (len, data) = self.load_scoop_string_len_and_data(str_obj_ptr)?;

                        segments.push(Segment { ptr: data, len });
                        total_len = self
                            .builder
                            .build_int_add(total_len, len, "fstr_total_len")?;
                        continue;
                    }

                    let v = self.codegen_expr(expr)?;

                    match v.ty {
                        CgTy::String => {
                            let coerced = self.coerce_value(expr.span, v, CgTy::String)?;
                            let Some(raw) = coerced.value else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation expr value",
                                    at: expr.span.into(),
                                });
                            };
                            let BasicValueEnum::PointerValue(str_obj_ptr) = raw else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation expr type",
                                    at: expr.span.into(),
                                });
                            };

                            let (len, data) = self.load_scoop_string_len_and_data(str_obj_ptr)?;

                            segments.push(Segment { ptr: data, len });
                            total_len =
                                self.builder
                                    .build_int_add(total_len, len, "fstr_total_len")?;
                        }
                        CgTy::Bool => {
                            let Some(raw) = v.value else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation bool expr value",
                                    at: expr.span.into(),
                                });
                            };
                            let BasicValueEnum::IntValue(bool_val) = raw else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation bool expr type",
                                    at: expr.span.into(),
                                });
                            };

                            let bool_as_i64 = self.builder.build_int_z_extend(
                                bool_val,
                                i64_ty,
                                "fstr_bool_zext",
                            )?;
                            let rt_bool = self.declare_runtime_bool_to_string();
                            let call = self.build_call_preserving_gc_local_roots(
                                expr.span,
                                rt_bool,
                                &[bool_as_i64.into()],
                                "rt_bool_to_string_for_fstr",
                            )?;
                            let ret = call.try_as_basic_value().basic().ok_or(
                                LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation bool return value",
                                    at: expr.span.into(),
                                },
                            )?;
                            let BasicValueEnum::PointerValue(str_obj_ptr) = ret else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation bool return type",
                                    at: expr.span.into(),
                                });
                            };

                            let (len, data) = self.load_scoop_string_len_and_data(str_obj_ptr)?;

                            segments.push(Segment { ptr: data, len });
                            total_len =
                                self.builder
                                    .build_int_add(total_len, len, "fstr_total_len")?;
                        }
                        CgTy::Float64 | CgTy::Float32 => {
                            let str_v =
                                self.codegen_float_to_string_value(expr.span, expr.span, v)?;
                            let Some(raw) = str_v.value else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation float expr value",
                                    at: expr.span.into(),
                                });
                            };
                            let BasicValueEnum::PointerValue(str_obj_ptr) = raw else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation float expr type",
                                    at: expr.span.into(),
                                });
                            };

                            let (len, data) = self.load_scoop_string_len_and_data(str_obj_ptr)?;

                            segments.push(Segment { ptr: data, len });
                            total_len =
                                self.builder
                                    .build_int_add(total_len, len, "fstr_total_len")?;
                        }
                        CgTy::Int(from_ty) => {
                            if from_ty.bits > 64 {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "integer width for string interpolation",
                                    at: expr.span.into(),
                                });
                            }

                            let (raw_int, _) =
                                v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                                    kind: "integer interpolation expr value",
                                    at: expr.span.into(),
                                })?;

                            // 先把整数提升/截断到 i64/u64，再调用 runtime 格式化到临时 buffer。
                            let to_ty = IntTy {
                                bits: 64,
                                signed: from_ty.signed,
                            };
                            let int64 = self.cast_int(raw_int, from_ty, to_ty)?;

                            // i64 最长：`-9223372036854775808`（20 字符）；
                            // 这里给更宽松的 cap，避免后续扩展时踩坑。
                            let cap = i64_ty.const_int(64, false);
                            let buf =
                                self.builder
                                    .build_array_alloca(i8_ty, cap, "fstr_int_buf")?;

                            let fmt_name = if from_ty.signed {
                                "scoop_format_i64"
                            } else {
                                "scoop_format_u64"
                            };
                            let fmt_fun = self.declare_runtime_format_int(fmt_name);
                            let call_site = self.builder.build_call(
                                fmt_fun,
                                &[int64.into(), buf.into(), cap.into()],
                                "fstr_fmt_int",
                            )?;
                            let len = call_site
                                .try_as_basic_value()
                                .basic()
                                .ok_or(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation int length",
                                    at: expr.span.into(),
                                })?
                                .into_int_value();

                            segments.push(Segment { ptr: buf, len });
                            total_len =
                                self.builder
                                    .build_int_add(total_len, len, "fstr_total_len")?;
                        }
                        _ => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "string interpolation expr type",
                                at: expr.span.into(),
                            });
                        }
                    }
                }
            }
        }

        // 2) 为拼接结果分配 heap buffer（malloc），并按顺序 memcpy 各段。
        let is_zero = self.builder.build_int_compare(
            IntPredicate::EQ,
            total_len,
            i64_ty.const_zero(),
            "fstr_total_is_zero",
        )?;

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        let malloc_bb = self.context.append_basic_block(func, "fstr_malloc");
        let done_bb = self.context.append_basic_block(func, "fstr_done");

        self.builder
            .build_conditional_branch(is_zero, done_bb, malloc_bb)?;

        // --- malloc + memcpy ---
        self.builder.position_at_end(malloc_bb);
        let malloc = self.declare_libc_malloc();
        let call = self
            .builder
            .build_call(malloc, &[total_len.into()], "fstr_malloc")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "malloc return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(buf) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "malloc return type",
                at: span.into(),
            });
        };

        let mut cursor = i64_ty.const_zero();
        for (idx, seg) in segments.iter().enumerate() {
            let dst = unsafe {
                self.builder.build_in_bounds_gep(
                    i8_ty,
                    buf,
                    &[cursor],
                    &format!("fstr_dst_{idx}"),
                )?
            };
            let _ = self.builder.build_memcpy(dst, 1, seg.ptr, 1, seg.len)?;
            cursor = self.builder.build_int_add(cursor, seg.len, "fstr_cursor")?;
        }

        self.builder.build_unconditional_branch(done_bb)?;

        // --- done ---
        self.builder.position_at_end(done_bb);
        let buf_phi = self.builder.build_phi(i8_ptr_ty, "fstr_buf")?;
        let buf_null: BasicValueEnum<'ctx> = i8_ptr_ty.const_null().into();
        let buf_value: BasicValueEnum<'ctx> = buf.into();
        buf_phi.add_incoming(&[(&buf_null, insert_block), (&buf_value, malloc_bb)]);
        let buf_ptr = buf_phi.as_basic_value().into_pointer_value();

        // 3) 分配并初始化 `ScoopString` 对象（GC-managed）。
        let obj_size = self.target_data.get_store_size(&scoop_str_ty);
        let size_v = i64_ty.const_int(obj_size, false);

        let str_desc = self.get_or_create_string_type_desc_global(span)?;
        let str_desc_i8 = self.builder.build_pointer_cast(
            str_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "fstr_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_alloc,
            &[str_desc_i8.into(), size_v.into()],
            "rt_alloc_fstr",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(raw_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return type",
                at: span.into(),
            });
        };

        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let str_ptr = self
            .builder
            .build_pointer_cast(raw_ptr, str_ptr_ty, "fstr_obj_ptr")?;

        let len_ptr = self
            .builder
            .build_struct_gep(scoop_str_ty, str_ptr, 1, "fstr_len_gep")?;
        let data_ptr = self
            .builder
            .build_struct_gep(scoop_str_ty, str_ptr, 2, "fstr_data_gep")?;

        let _ = self.builder.build_store(len_ptr, total_len)?;
        let _ = self.builder.build_store(data_ptr, buf_ptr)?;

        Ok(CgValue {
            ty: CgTy::String,
            value: Some(str_ptr.into()),
        })
    }

    fn codegen_var_ref(
        &mut self,
        span: crate::span::Span,
        v: &hir::ValueRef,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match v {
            hir::ValueRef::TopLevel { fqn, .. } => self.codegen_top_level_value_ref(span, fqn),
            hir::ValueRef::Local { id, .. } => {
                let local = self.function_cx.env.get(*id).ok_or_else(|| {
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "unknown local value",
                        at: span.into(),
                    }
                })?;
                let local_ptr = self.local_ptr_for_use(span, local, "load_local_slot")?;

                match local.ty {
                    CgTy::Unit => Ok(CgValue::unit()),
                    CgTy::Never => Ok(CgValue::never()),
                    CgTy::Bool => {
                        let raw = self
                            .builder
                            .build_load(
                                self.llvm_basic_type_of(span, local.ty)?,
                                local_ptr,
                                "load_bool",
                            )?
                            .into_int_value();
                        Ok(CgValue::bool(raw))
                    }
                    CgTy::Float64 | CgTy::Float32 => {
                        let raw = self
                            .builder
                            .build_load(
                                self.llvm_basic_type_of(span, local.ty)?,
                                local_ptr,
                                "load_float",
                            )?
                            .into_float_value();
                        Ok(CgValue::float(raw, local.ty))
                    }
                    CgTy::Int(int_ty) => {
                        let raw = self
                            .builder
                            .build_load(
                                self.llvm_basic_type_of(span, local.ty)?,
                                local_ptr,
                                "load_int",
                            )?
                            .into_int_value();
                        Ok(CgValue::int(raw, int_ty))
                    }
                    CgTy::String => {
                        let raw = self
                            .builder
                            .build_load(
                                self.llvm_basic_type_of(span, local.ty)?,
                                local_ptr,
                                "load_str",
                            )?
                            .into_pointer_value();
                        Ok(CgValue {
                            ty: CgTy::String,
                            value: Some(raw.into()),
                        })
                    }
                    CgTy::Ref => {
                        let raw = self
                            .builder
                            .build_load(
                                self.llvm_basic_type_of(span, local.ty)?,
                                local_ptr,
                                "load_ref",
                            )?
                            .into_pointer_value();
                        Ok(CgValue {
                            ty: CgTy::Ref,
                            value: Some(raw.into()),
                        })
                    }
                    CgTy::Tuple(_) => {
                        let raw = self.builder.build_load(
                            self.llvm_basic_type_of(span, local.ty)?,
                            local_ptr,
                            "load_tuple",
                        )?;
                        Ok(CgValue {
                            ty: local.ty,
                            value: Some(raw),
                        })
                    }
                    CgTy::Struct(_) => {
                        let raw = self.builder.build_load(
                            self.llvm_basic_type_of(span, local.ty)?,
                            local_ptr,
                            "load_struct",
                        )?;
                        Ok(CgValue {
                            ty: local.ty,
                            value: Some(raw),
                        })
                    }
                    CgTy::Enum(_) => {
                        let raw = self.builder.build_load(
                            self.llvm_basic_type_of(span, local.ty)?,
                            local_ptr,
                            "load_enum",
                        )?;
                        Ok(CgValue {
                            ty: local.ty,
                            value: Some(raw),
                        })
                    }
                }
            }
        }
    }

    fn codegen_struct_lit(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
        fields: &[hir::StructLitField],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let Some(CgTy::Struct(struct_ty)) = self.cg_ty_of(ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "struct literal type",
                at: span.into(),
            });
        };

        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(struct_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "struct literal type",
                at: span.into(),
            });
        };

        let layout_key = self.nominal_layout_key(nominal);
        let layout =
            self.struct_layouts
                .get(&layout_key)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "struct literal layout",
                    at: span.into(),
                })?;

        let llvm_struct_ty = self.llvm_struct_type(span, struct_ty)?;
        let mut deferred_fields: Vec<(u32, String, crate::span::Span, DeferredCgValue<'ctx>)> =
            Vec::with_capacity(layout.fields.len());

        for (idx, field) in layout.fields.iter().enumerate() {
            let Some(init) = fields.iter().find(|f| f.name == field.name) else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "struct literal missing field",
                    at: span.into(),
                });
            };

            let field_cg =
                self.cg_ty_of_layout_field(init.span, field.ty, field.ty_fqn.as_deref())?;

            // 重要：struct 字段 initializer 需要以字段类型作为 expected context。
            //
            // 例如：`Wrap { e: B(7) }` 中的 `B(7)` 是 enum variant ctor call：
            // - 若缺少 expected enum type，后端无法决定该 ctor 对应哪个 enum 的表示；
            // - 这里把 `field_cg` 作为 expected 传入，可与 `val x: E = B(7)` 的路径保持一致。
            let init_v = self.codegen_expr_in_expected_context(&init.value, Some(field_cg))?;
            let coerced = if field_cg == CgTy::Unit {
                CgValue::unit()
            } else if init_v.ty != field_cg {
                self.coerce_value(init.value.span, init_v, field_cg)?
            } else {
                init_v
            };

            let deferred = self.defer_gc_sensitive_cg_value(
                init.value.span,
                &format!("struct_field_{idx}"),
                coerced,
            )?;

            // T0119: For `@CLayout(packed = N)` with N > 1, use the remapped LLVM element index.
            let llvm_idx = self
                .shared_caches
                .pack_field_indices
                .borrow()
                .get(&layout_key)
                .map_or(idx as u32, |indices| indices[idx]);
            deferred_fields.push((llvm_idx, field.name.clone(), init.value.span, deferred));
        }

        let mut agg: AggregateValueEnum<'ctx> = llvm_struct_ty.get_undef().into();
        for (idx, (llvm_idx, field_name, field_span, deferred)) in
            deferred_fields.into_iter().enumerate()
        {
            let materialized = self.materialize_deferred_cg_value(
                field_span,
                &format!("struct_field_reload_{idx}"),
                deferred,
            )?;
            let raw = match materialized.ty {
                CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
                _ => materialized
                    .value
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "struct field value",
                        at: field_span.into(),
                    })?,
            };

            let name = format!("insert_{field_name}");
            agg = self.builder.build_insert_value(agg, raw, llvm_idx, &name)?;
        }

        Ok(CgValue {
            ty: CgTy::Struct(struct_ty),
            value: Some(agg.as_basic_value_enum()),
        })
    }

    fn codegen_tuple_lit(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
        elements: &[hir::Expr],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let Some(CgTy::Tuple(tuple_ty)) = self.cg_ty_of(ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple literal type",
                at: span.into(),
            });
        };

        let TypeKind::Value(ValueTypeKind::Tuple(element_tys)) = self.types.kind(tuple_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple literal type",
                at: span.into(),
            });
        };

        if element_tys.len() != elements.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple literal arity mismatch",
                at: span.into(),
            });
        }

        let llvm_tuple_ty = self.llvm_tuple_type(span, tuple_ty)?;
        let mut deferred_elements: Vec<(usize, crate::span::Span, DeferredCgValue<'ctx>)> =
            Vec::with_capacity(elements.len());

        for (idx, (elem_expr, elem_ty)) in elements.iter().zip(element_tys.iter()).enumerate() {
            let elem_cg = self
                .cg_ty_of(*elem_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "tuple element type",
                    at: elem_expr.span.into(),
                })?;

            // tuple 元素 initializer 也需要带 expected context：
            // 否则 `({ 11 }, 4)` 这类包含 closure literal 的 tuple 会在元素 codegen 时
            // 落回“无 expected function type”的通用 `expression kind` unsupported。
            let elem_v = self.codegen_expr_in_expected_context(elem_expr, Some(elem_cg))?;
            let coerced = self.coerce_value(elem_expr.span, elem_v, elem_cg)?;
            let deferred = self.defer_gc_sensitive_cg_value(
                elem_expr.span,
                &format!("tuple_elem_{idx}"),
                coerced,
            )?;
            deferred_elements.push((idx, elem_expr.span, deferred));
        }

        let mut agg: AggregateValueEnum<'ctx> = llvm_tuple_ty.get_undef().into();
        for (idx, elem_span, deferred) in deferred_elements {
            let materialized = self.materialize_deferred_cg_value(
                elem_span,
                &format!("tuple_elem_reload_{idx}"),
                deferred,
            )?;
            let raw: BasicValueEnum<'ctx> = match materialized.ty {
                CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
                _ => materialized
                    .value
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "tuple element value",
                        at: elem_span.into(),
                    })?,
            };

            let name = format!("insert_elem_{idx}");
            agg = self
                .builder
                .build_insert_value(agg, raw, idx as u32, &name)?;
        }

        Ok(CgValue {
            ty: CgTy::Tuple(tuple_ty),
            value: Some(agg.as_basic_value_enum()),
        })
    }

    fn codegen_member_access(
        &mut self,
        _span: crate::span::Span,
        receiver: &hir::Expr,
        member: &hir::MemberAccess,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match member.resolved.as_ref() {
            Some(hir::MemberRef::Value { fqn, .. }) => {
                // T1311：`TypeName.NestedObject` / `Obj.NestedObject` 的"object 值"访问。
                if self.object_inits.contains_key(fqn) {
                    return self.codegen_object_value_access(member.span, fqn);
                }

                // T0828：`object` / `companion object` 静态成员访问（backing field 读取）。
                if self.lookup_object_property_by_fqn(fqn).is_some() {
                    return self.codegen_object_property_access(member.span, fqn);
                }

                // `EnumName.Variant`（unit variant）：`RuntimeError.NullAssertionFailed` 等。
                if let Some(v) =
                    self.try_codegen_qualified_enum_unit_variant_value(member.span, fqn)?
                {
                    return Ok(v);
                }

                // 优先使用“当前表达式语境下最精确的 receiver 类型”：
                // - smart-cast / branch narrowing 会把 `receiver.ty` 收窄到比声明更具体的类型；
                // - 普通局部变量若仍只有 `Any` / `Param`，再回退到 env 里保存的原始 `hir_ty`。
                let receiver_hir_ty = self
                    .resolve_expr_concrete_type(receiver)
                    .unwrap_or(receiver.ty);

                // T1312：class 实例字段访问（`this.x` / `obj.x`）。
                if let Some((class, field_idx, field_cg)) =
                    self.lookup_class_field_by_fqn(fqn, member.span, Some(receiver_hir_ty))?
                {
                    if field_cg == CgTy::Unit {
                        return Ok(CgValue::unit());
                    }

                    let recv = self.codegen_expr_in_expected_context(receiver, Some(CgTy::Ref))?;
                    let recv = self.coerce_value(receiver.span, recv, CgTy::Ref)?;
                    let Some(raw) = recv.value else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "class field receiver value",
                            at: receiver.span.into(),
                        });
                    };
                    let BasicValueEnum::PointerValue(obj_ptr) = raw else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "class field receiver type",
                            at: receiver.span.into(),
                        });
                    };

                    let field_ptr =
                        self.codegen_class_field_ptr(member.span, &class, obj_ptr, field_idx)?;
                    let llvm_ty = self.llvm_basic_type_of(member.span, field_cg)?;
                    let loaded = self
                        .builder
                        .build_load(llvm_ty, field_ptr, "load_class_field")?;
                    return self.cg_value_from_loaded(member.span, field_cg, loaded);
                }

                // 优先路径：`localStruct.field` —— 用 GEP 从 alloca slot 取字段（更贴近后续可变字段语义）。
                if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &receiver.kind
                    && let Some(local) = self.function_cx.env.get(*id)
                    && let CgTy::Struct(struct_ty) = local.ty
                {
                    let (field_idx, field_ty) =
                        self.lookup_struct_field(struct_ty, fqn, member.span)?;
                    if field_ty == CgTy::Unit {
                        return Ok(CgValue::unit());
                    }

                    let local_ptr = self.local_ptr_for_use(member.span, local, "field_base_ptr")?;
                    let llvm_struct_ty = self.llvm_struct_type(member.span, struct_ty)?;
                    let field_ptr = self.builder.build_struct_gep(
                        llvm_struct_ty,
                        local_ptr,
                        field_idx,
                        "field_gep",
                    )?;
                    let llvm_field_ty = self.llvm_basic_type_of(member.span, field_ty)?;
                    let loaded = self
                        .builder
                        .build_load(llvm_field_ty, field_ptr, "load_field")?;
                    // `@CLayout(packed = N)`：字段地址可能是非自然对齐的，需要把 load
                    // alignment 降到 `min(field_natural_align, N)` 以避免 UB。
                    if let Some(pack_n) = self.struct_clayout(struct_ty).and_then(|c| c.packed)
                        && let Some(inst) = loaded.as_instruction_value()
                    {
                        let natural = self.target_data.get_abi_alignment(&llvm_field_ty);
                        let effective = std::cmp::min(natural, pack_n);
                        inst.set_alignment(effective)?;
                    }
                    return self.cg_value_from_loaded(member.span, field_ty, loaded);
                }

                // fallback：先把 receiver 降到值，再用 extractvalue 取字段。
                let recv = self.codegen_expr(receiver)?;
                let CgTy::Struct(struct_ty) = recv.ty else {
                    tracing::warn!(
                        "codegen_member_access_expr: unsupported struct receiver for member {} (resolved={:?}) -> {:?}",
                        member.name,
                        member.resolved,
                        recv.ty
                    );
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "member access receiver type",
                        at: receiver.span.into(),
                    });
                };
                let (field_idx, field_ty) =
                    self.lookup_struct_field(struct_ty, fqn, member.span)?;
                if field_ty == CgTy::Unit {
                    return Ok(CgValue::unit());
                }

                let raw = recv.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "member access receiver value",
                    at: receiver.span.into(),
                })?;
                let struct_v = raw.into_struct_value();
                let extracted =
                    self.builder
                        .build_extract_value(struct_v, field_idx, "extract_field")?;
                return self.cg_value_from_loaded(member.span, field_ty, extracted);
            }
            Some(_) => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "member access target",
                    at: member.span.into(),
                });
            }
            None => {}
        }

        // tuple 元素访问（spec §2.3.3）：`t._0` / `t._1` / ...
        let Some(elem_idx) = parse_tuple_member_index(&member.name) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "member access target",
                at: member.span.into(),
            });
        };

        // 优先路径：`localTuple._0` —— 用 GEP 从 alloca slot 取元素。
        if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &receiver.kind
            && let Some(local) = self.function_cx.env.get(*id)
            && let CgTy::Tuple(tuple_ty) = local.ty
        {
            let elem_ty = self.lookup_tuple_element(tuple_ty, elem_idx, member.span)?;
            if elem_ty == CgTy::Unit {
                return Ok(CgValue::unit());
            }

            let local_ptr = self.local_ptr_for_use(member.span, local, "tuple_base_ptr")?;
            let llvm_tuple_ty = self.llvm_tuple_type(member.span, tuple_ty)?;
            let elem_ptr = self.builder.build_struct_gep(
                llvm_tuple_ty,
                local_ptr,
                elem_idx,
                "tuple_elem_gep",
            )?;
            let llvm_elem_ty = self.llvm_basic_type_of(member.span, elem_ty)?;
            let loaded = self
                .builder
                .build_load(llvm_elem_ty, elem_ptr, "load_tuple_elem")?;
            return self.cg_value_from_loaded(member.span, elem_ty, loaded);
        }

        // fallback：先把 receiver 降到值，再用 extractvalue 取元素。
        let recv = self.codegen_expr(receiver)?;
        let CgTy::Tuple(tuple_ty) = recv.ty else {
            tracing::warn!(
                "codegen_member_access_expr: unsupported tuple receiver for member {} -> {:?}",
                member.name,
                recv.ty
            );
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "member access receiver type",
                at: receiver.span.into(),
            });
        };

        let elem_ty = self.lookup_tuple_element(tuple_ty, elem_idx, member.span)?;
        if elem_ty == CgTy::Unit {
            return Ok(CgValue::unit());
        }

        let raw = recv.value.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "member access receiver value",
            at: receiver.span.into(),
        })?;
        let tuple_v = raw.into_struct_value();
        let extracted =
            self.builder
                .build_extract_value(tuple_v, elem_idx, "extract_tuple_elem")?;
        self.cg_value_from_loaded(member.span, elem_ty, extracted)
    }

    fn cg_value_from_loaded(
        &self,
        _at: crate::span::Span,
        ty: CgTy,
        raw: BasicValueEnum<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        Ok(match ty {
            CgTy::Unit => CgValue::unit(),
            CgTy::Bool => CgValue::bool(raw.into_int_value()),
            CgTy::Float64 | CgTy::Float32 => CgValue::float(raw.into_float_value(), ty),
            CgTy::Int(int_ty) => CgValue::int(raw.into_int_value(), int_ty),
            CgTy::String => CgValue {
                ty: CgTy::String,
                value: Some(raw.into_pointer_value().into()),
            },
            CgTy::Ref => CgValue {
                ty: CgTy::Ref,
                value: Some(raw.into_pointer_value().into()),
            },
            CgTy::Tuple(tuple_ty) => CgValue {
                ty: CgTy::Tuple(tuple_ty),
                value: Some(raw),
            },
            CgTy::Struct(struct_ty) => CgValue {
                ty: CgTy::Struct(struct_ty),
                value: Some(raw),
            },
            CgTy::Enum(enum_ty) => CgValue {
                ty: CgTy::Enum(enum_ty),
                value: Some(raw),
            },
            CgTy::Never => CgValue::never(),
        })
    }

    fn codegen_unary(
        &mut self,
        span: crate::span::Span,
        result_ty: TypeId,
        op: ast::UnaryOp,
        expr: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match op {
            ast::UnaryOp::Not => {
                let v = self.codegen_expr(expr)?.as_bool().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "unary ! operand",
                        at: span.into(),
                    },
                )?;
                let out = self.builder.build_not(v, "not")?;
                Ok(CgValue::bool(out))
            }
            ast::UnaryOp::Neg => {
                if matches!(expr.kind, hir::ExprKind::Literal(hir::LiteralKind::Int))
                    && let Some(CgTy::Int(int_ty)) = self.cg_ty_of(result_ty)
                {
                    let bits = self.negated_int_literal_bits_for_ty(span, expr.span, int_ty)?;
                    return Ok(CgValue::int(
                        self.int_type(int_ty).const_int(bits, false),
                        int_ty,
                    ));
                }

                let value = self.codegen_expr(expr)?;
                match value.ty {
                    CgTy::Int(ty) => {
                        let (v, _) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "unary - operand",
                            at: span.into(),
                        })?;
                        let out = self.builder.build_int_neg(v, "neg")?;
                        Ok(CgValue::int(out, ty))
                    }
                    CgTy::Float64 | CgTy::Float32 => {
                        let (v, ty) =
                            value.as_float().ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "unary - operand",
                                at: span.into(),
                            })?;
                        let out = self.builder.build_float_neg(v, "fneg")?;
                        Ok(CgValue::float(out, ty))
                    }
                    _ => Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "unary - operand",
                        at: span.into(),
                    }),
                }
            }
            ast::UnaryOp::BitNot => {
                let (v, ty) = self.codegen_expr(expr)?.as_int().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "unary ~ operand",
                        at: span.into(),
                    },
                )?;
                let out = self.builder.build_not(v, "bitnot")?;
                Ok(CgValue::int(out, ty))
            }
        }
    }

    /// Map a binary operator to its operator overload method name (Spec B.8).
    fn operator_overload_method_name(op: ast::BinaryOp) -> Option<&'static str> {
        match op {
            ast::BinaryOp::Add => Some("plus"),
            ast::BinaryOp::Sub => Some("minus"),
            ast::BinaryOp::Mul => Some("times"),
            ast::BinaryOp::Div => Some("div"),
            ast::BinaryOp::Rem => Some("rem"),
            ast::BinaryOp::BitAnd => Some("and"),
            ast::BinaryOp::BitOr => Some("or"),
            ast::BinaryOp::BitXor => Some("xor"),
            ast::BinaryOp::Shl => Some("shl"),
            ast::BinaryOp::Shr => Some("shr"),
            _ => None,
        }
    }

    /// Try to dispatch a binary operator to a user-defined method on a struct type.
    /// Returns `Some(result)` if the LHS is a struct with the corresponding operator method,
    /// `None` if the LHS is not a struct type (caller should use builtin integer path).
    /// Resolve the effective CgTy for an expression, preferring concrete type
    /// sources over the often-widened HIR `expr.ty`.
    fn resolve_expr_cg_ty(&self, expr: &hir::Expr) -> Option<CgTy> {
        // Locals keep their exact lowered codegen type in the environment, so
        // prefer that before reconstructing from a TypeId.
        if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &expr.kind
            && let Some(local) = self.function_cx.env.get(*id)
        {
            return Some(local.ty);
        }

        // HIR lowering writes all VarRef expressions as `Any`, including
        // top-level refs. Reuse the broader concrete-type resolver so builtin
        // call sites like `Continuation.resume(payload)` do not regress when
        // the payload comes from a top-level typed binding.
        if let Some(concrete_ty) = self.resolve_expr_concrete_type(expr) {
            return self.cg_ty_of(concrete_ty);
        }

        self.cg_ty_of(expr.ty)
    }

    fn expr_uses_float_codegen(&self, expr: &hir::Expr) -> bool {
        matches!(
            self.resolve_expr_cg_ty(expr),
            Some(CgTy::Float64 | CgTy::Float32)
        ) || matches!(
            expr.kind,
            hir::ExprKind::Literal(hir::LiteralKind::Float64(_))
                | hir::ExprKind::Literal(hir::LiteralKind::Float32(_))
        )
    }

    fn is_unsuffixed_float64_literal(expr: &hir::Expr) -> bool {
        matches!(
            expr.kind,
            hir::ExprKind::Literal(hir::LiteralKind::Float64(_))
        )
    }

    fn unify_float_cg_types(
        &self,
        lhs: &hir::Expr,
        lhs_ty: CgTy,
        rhs: &hir::Expr,
        rhs_ty: CgTy,
    ) -> Option<CgTy> {
        match (lhs_ty, rhs_ty) {
            (CgTy::Float64, CgTy::Float64) => Some(CgTy::Float64),
            (CgTy::Float32, CgTy::Float32) => Some(CgTy::Float32),
            (CgTy::Float64, CgTy::Float32) if Self::is_unsuffixed_float64_literal(lhs) => {
                Some(CgTy::Float32)
            }
            (CgTy::Float32, CgTy::Float64) if Self::is_unsuffixed_float64_literal(rhs) => {
                Some(CgTy::Float32)
            }
            _ => None,
        }
    }

    fn try_codegen_operator_overload(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        // Check if LHS has a struct type, resolving through local env if needed.
        let Some(CgTy::Struct(lhs_type_id)) = self.resolve_expr_cg_ty(lhs) else {
            return Ok(None);
        };

        let method = match Self::operator_overload_method_name(op) {
            Some(m) => m,
            None => return Ok(None),
        };

        // Get the struct FQN from TypeId.
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(lhs_type_id) else {
            return Ok(None);
        };
        let struct_fqn = nominal.fqn.clone();
        let method_fqn = format!("{struct_fqn}.{method}");

        // Look up the method in fun_index.
        let sig_fun = match self.fun_index.get(method_fqn.as_str()) {
            Some(f) => *f,
            None => return Ok(None),
        };

        // Generate the call: StructType.method(lhs, rhs)
        let result = self.codegen_operator_overload_call(span, &method_fqn, sig_fun, lhs, rhs)?;
        Ok(Some(result))
    }

    /// Try to dispatch a comparison operator to a `compareTo` method on a struct type.
    /// `compareTo(other) -> Int`, then compare the result with 0.
    fn try_codegen_compare_to_overload(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let Some(CgTy::Struct(lhs_type_id)) = self.resolve_expr_cg_ty(lhs) else {
            return Ok(None);
        };

        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(lhs_type_id) else {
            return Ok(None);
        };
        let struct_fqn = nominal.fqn.clone();
        let method_fqn = format!("{struct_fqn}.compareTo");

        let sig_fun = match self.fun_index.get(method_fqn.as_str()) {
            Some(f) => *f,
            None => return Ok(None),
        };

        // Call compareTo: returns Int
        let cmp_result =
            self.codegen_operator_overload_call(span, &method_fqn, sig_fun, lhs, rhs)?;
        let (cmp_int, _) = cmp_result
            .as_int()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "compareTo return type (expected Int)",
                at: span.into(),
            })?;

        // Compare result with 0: result < 0 for Lt, result <= 0 for Le, etc.
        let zero = self.context.i64_type().const_zero();
        let pred = match op {
            ast::BinaryOp::Lt => IntPredicate::SLT,
            ast::BinaryOp::Le => IntPredicate::SLE,
            ast::BinaryOp::Gt => IntPredicate::SGT,
            ast::BinaryOp::Ge => IntPredicate::SGE,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "compareTo comparison op",
                    at: span.into(),
                });
            }
        };
        let result = self
            .builder
            .build_int_compare(pred, cmp_int, zero, "cmp_to")?;
        Ok(Some(CgValue::bool(result)))
    }

    /// Generate a call to a struct's operator overload method.
    /// The method has signature: `fun StructType.method(this: StructType, rhs: RhsType): RetType`
    fn codegen_operator_overload_call(
        &mut self,
        span: crate::span::Span,
        method_fqn: &str,
        sig_fun: &hir::FunDecl,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if sig_fun.params.len() != 2 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "operator overload arity (expected 2)",
                at: span.into(),
            });
        }
        let call_args = [
            hir::CallArg::Positional(lhs.clone()),
            hir::CallArg::Positional(rhs.clone()),
        ];
        self.codegen_top_level_fun_call(span, span, method_fqn, &call_args)
    }

    fn codegen_binary(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match op {
            ast::BinaryOp::Add
            | ast::BinaryOp::Sub
            | ast::BinaryOp::Mul
            | ast::BinaryOp::Div
            | ast::BinaryOp::Rem => {
                // T0111: try operator overload dispatch for user-defined types first.
                if let Some(result) = self.try_codegen_operator_overload(span, op, lhs, rhs)? {
                    return Ok(result);
                }
                if self.expr_uses_float_codegen(lhs) || self.expr_uses_float_codegen(rhs) {
                    return self.codegen_float_binary_same_type(span, op, lhs, rhs);
                }
                self.codegen_int_binary_same_type(span, op, lhs, rhs)
            }
            ast::BinaryOp::BitAnd | ast::BinaryOp::BitXor | ast::BinaryOp::BitOr => {
                // T0111: try operator overload dispatch for user-defined types first.
                if let Some(result) = self.try_codegen_operator_overload(span, op, lhs, rhs)? {
                    return Ok(result);
                }
                self.codegen_int_binary_same_type(span, op, lhs, rhs)
            }

            ast::BinaryOp::Shl | ast::BinaryOp::Shr => {
                if let Some(result) = self.try_codegen_operator_overload(span, op, lhs, rhs)? {
                    return Ok(result);
                }
                self.codegen_shift(span, op, lhs, rhs)
            }

            ast::BinaryOp::Lt | ast::BinaryOp::Le | ast::BinaryOp::Gt | ast::BinaryOp::Ge => {
                // T0111: try compareTo overload for user-defined types first.
                if let Some(result) = self.try_codegen_compare_to_overload(span, op, lhs, rhs)? {
                    return Ok(result);
                }
                if self.expr_uses_float_codegen(lhs) || self.expr_uses_float_codegen(rhs) {
                    return self.codegen_float_compare(span, op, lhs, rhs);
                }
                self.codegen_int_compare(span, op, lhs, rhs)
            }

            ast::BinaryOp::Eq | ast::BinaryOp::Ne => self.codegen_equality(span, op, lhs, rhs),

            ast::BinaryOp::LogAnd | ast::BinaryOp::LogOr => {
                self.codegen_bool_logic(span, op, lhs, rhs)
            }

            ast::BinaryOp::RangeInclusive => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "range operator",
                at: span.into(),
            }),

            ast::BinaryOp::Elvis => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "elvis operator",
                at: span.into(),
            }),
        }
    }

    fn codegen_type_check_expr(
        &mut self,
        span: crate::span::Span,
        op: ast::TypeCheckOp,
        expr: &hir::Expr,
        target_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 当前阶段只实现 ref→ref 的运行期检查（T1509b）：
        // - class：沿 type descriptor parent 链查找；
        // - interface：扫描 itable 是否包含 interface_id。
        //
        // 说明：typecheck 阶段对 `is/!is` 的静态约束仍偏弱（只保证 type lowering），
        // 因此 codegen 侧需要做"不可支持场景"的防御式报错，避免 silent miscompile。
        let v = self.codegen_expr(expr)?;
        let v = match v.ty {
            CgTy::Ref => v,
            CgTy::String => self.coerce_value(expr.span, v, CgTy::Ref)?,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "type check operand (ref)",
                    at: span.into(),
                });
            }
        };
        let Some(raw) = v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "type check operand value",
                at: span.into(),
            });
        };
        let BasicValueEnum::PointerValue(obj_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "type check operand type",
                at: span.into(),
            });
        };

        let is_ok = self.codegen_ref_is_instance_of(span, obj_ptr, target_ty)?;
        let out = match op {
            ast::TypeCheckOp::Is => is_ok,
            ast::TypeCheckOp::NotIs => self.builder.build_not(is_ok, "typecheck_not")?,
        };
        Ok(CgValue::bool(out))
    }

    fn codegen_cast_expr(
        &mut self,
        span: crate::span::Span,
        op: ast::CastOp,
        expr: &hir::Expr,
        target_ty: TypeId,
        out_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match op {
            ast::CastOp::As => self.codegen_cast_as_expr(span, expr, target_ty),
            ast::CastOp::AsQ => self.codegen_cast_asq_expr(span, expr, target_ty, out_ty),
        }
    }

    fn codegen_cast_as_expr(
        &mut self,
        span: crate::span::Span,
        expr: &hir::Expr,
        target_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let target_cg = self
            .cg_ty_of(target_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "cast target type",
                at: span.into(),
            })?;
        let target_ptr_ty = match target_cg {
            CgTy::Ref => self.llvm_gc_i8_ptr_type(),
            CgTy::String => self.llvm_scoop_string_ptr_type(),
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "cast target (ref)",
                    at: span.into(),
                });
            }
        };

        let v = self.codegen_expr(expr)?;
        let v = match v.ty {
            CgTy::Ref => v,
            CgTy::String => self.coerce_value(expr.span, v, CgTy::Ref)?,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "cast operand (ref)",
                    at: span.into(),
                });
            }
        };
        let Some(raw) = v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "cast operand value",
                at: span.into(),
            });
        };
        let BasicValueEnum::PointerValue(obj_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "cast operand type",
                at: span.into(),
            });
        };

        // 运行期检查：为避免在 obj=NULL 时解引用对象头，先对 NULL 做 fail 处理。
        let is_ok = self.codegen_ref_is_instance_of(span, obj_ptr, target_ty)?;

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        let ok_bb = self.context.append_basic_block(func, "cast_ok");
        let fail_bb = self.context.append_basic_block(func, "cast_fail");
        let merge_bb = self.context.append_basic_block(func, "cast_merge");
        self.builder
            .build_conditional_branch(is_ok, ok_bb, fail_bb)?;

        // --- ok ---
        self.builder.position_at_end(ok_bb);
        let casted_ptr = self
            .builder
            .build_pointer_cast(obj_ptr, target_ptr_ty, "cast_ptr")?;
        self.builder.build_unconditional_branch(merge_bb)?;

        // --- fail ---
        self.builder.position_at_end(fail_bb);
        self.emit_raise_runtime_error_variant(span, "ClassCastFailed")?;
        let fail_incoming = if self.ordinary_effect_propagation_enabled() {
            self.emit_ordinary_non_resuming_effect_exit(span, "cast_raise_effect")?;
            self.builder.build_unreachable()?;
            None
        } else {
            let dead_bb =
                self.builder
                    .get_insert_block()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "builder has no insert block",
                        at: span.into(),
                    })?;
            let default_ptr = target_ptr_ty.const_null();
            self.builder.build_unconditional_branch(merge_bb)?;
            Some((default_ptr, dead_bb))
        };

        // --- merge ---
        self.builder.position_at_end(merge_bb);
        let phi = self.builder.build_phi(target_ptr_ty, "cast_value")?;
        if let Some((default_ptr, dead_bb)) = fail_incoming {
            phi.add_incoming(&[(&casted_ptr, ok_bb), (&default_ptr, dead_bb)]);
        } else {
            phi.add_incoming(&[(&casted_ptr, ok_bb)]);
        }
        let out_ptr = phi.as_basic_value().into_pointer_value();

        Ok(CgValue {
            ty: target_cg,
            value: Some(out_ptr.into()),
        })
    }

    fn codegen_cast_asq_expr(
        &mut self,
        span: crate::span::Span,
        expr: &hir::Expr,
        target_ty: TypeId,
        out_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // `as?` 的结果类型应为 `Option<target_ty>`（或等价 nullable sugar）。
        let out_cg = self
            .cg_ty_of(out_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "cast result type",
                at: span.into(),
            })?;
        let CgTy::Enum(option_ty) = out_cg else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "as? result type (Option<T>)",
                at: span.into(),
            });
        };

        let target_cg = self
            .cg_ty_of(target_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "cast target type",
                at: span.into(),
            })?;
        let target_ptr_ty = match target_cg {
            CgTy::Ref => self.llvm_gc_i8_ptr_type(),
            CgTy::String => self.llvm_scoop_string_ptr_type(),
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "as? target (ref)",
                    at: span.into(),
                });
            }
        };

        let v = self.codegen_expr(expr)?;
        let v = match v.ty {
            CgTy::Ref => v,
            CgTy::String => self.coerce_value(expr.span, v, CgTy::Ref)?,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "as? operand (ref)",
                    at: span.into(),
                });
            }
        };
        let Some(raw) = v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "as? operand value",
                at: span.into(),
            });
        };
        let BasicValueEnum::PointerValue(obj_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "as? operand type",
                at: span.into(),
            });
        };

        let is_ok = self.codegen_ref_is_instance_of(span, obj_ptr, target_ty)?;

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        let ok_bb = self.context.append_basic_block(func, "asq_ok");
        let fail_bb = self.context.append_basic_block(func, "asq_fail");
        let merge_bb = self.context.append_basic_block(func, "asq_merge");
        self.builder
            .build_conditional_branch(is_ok, ok_bb, fail_bb)?;

        // --- ok：Some(casted) ---
        self.builder.position_at_end(ok_bb);
        let casted_ptr = self
            .builder
            .build_pointer_cast(obj_ptr, target_ptr_ty, "asq_cast_ptr")?;
        let casted = CgValue {
            ty: target_cg,
            value: Some(casted_ptr.into()),
        };
        let payload = self.coerce_enum_payload(span, casted, target_cg)?;
        let some_v = self.build_enum_value(span, option_ty, 0, payload)?;
        let some_raw = some_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "as? Some value",
            at: span.into(),
        })?;
        self.builder.build_unconditional_branch(merge_bb)?;

        // --- fail：None ---
        self.builder.position_at_end(fail_bb);
        let none_v = self.build_enum_value(span, option_ty, 1, CgEnumPayload::default())?;
        let none_raw = none_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "as? None value",
            at: span.into(),
        })?;
        self.builder.build_unconditional_branch(merge_bb)?;

        // --- merge ---
        self.builder.position_at_end(merge_bb);
        let llvm_option_ty = self.llvm_enum_value_type(span, option_ty)?;
        let phi = self.builder.build_phi(llvm_option_ty, "asq_value")?;
        phi.add_incoming(&[(&some_raw, ok_bb), (&none_raw, fail_bb)]);
        let out_raw = phi.as_basic_value();

        Ok(CgValue {
            ty: CgTy::Enum(option_ty),
            value: Some(out_raw),
        })
    }

    /// 运行期类型检查：判断 `obj` 是否为 `target_ty` 的实例。
    ///
    /// 约定（v0，T1509b）：
    /// - 若 `obj == NULL`：返回 false（避免解引用 NULL）；
    /// - `Any`：只要非 NULL 即为 true（不依赖 type_desc）；
    /// - class：沿 `type_desc.parent_type_desc` 向上查找；
    /// - interface：扫描 itable entries 的 runtime target match 集。
    fn codegen_ref_is_instance_of(
        &mut self,
        at: crate::span::Span,
        obj: PointerValue<'ctx>,
        target_ty: TypeId,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let obj_is_null = self.builder.build_is_null(obj, "isa_obj_is_null")?;

        // fast path：`x is Any` 只需要判空。
        if matches!(self.types.kind(target_ty), TypeKind::Ref(RefTypeKind::Any)) {
            return Ok(self.builder.build_not(obj_is_null, "isa_any_nonnull")?);
        }

        // 对其它 target：obj 为 NULL 时直接 false，避免解引用对象头。
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: at.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: at.into(),
            })?;

        let null_bb = self.context.append_basic_block(func, "isa_obj_null");
        let nonnull_bb = self.context.append_basic_block(func, "isa_obj_nonnull");
        let done_bb = self.context.append_basic_block(func, "isa_done");
        self.builder
            .build_conditional_branch(obj_is_null, null_bb, nonnull_bb)?;

        // null -> done(false)
        self.builder.position_at_end(null_bb);
        self.builder.build_unconditional_branch(done_bb)?;

        // nonnull -> 计算真实检查 -> done
        self.builder.position_at_end(nonnull_bb);
        let inner_ok = self.codegen_ref_is_instance_of_nonnull(at, obj, target_ty)?;
        let after_check_bb =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: at.into(),
                })?;
        self.builder.build_unconditional_branch(done_bb)?;

        // done：phi 合并
        self.builder.position_at_end(done_bb);
        let phi = self
            .builder
            .build_phi(self.context.bool_type(), "isa_result")?;
        phi.add_incoming(&[
            (&self.context.bool_type().const_int(0, false), null_bb),
            (&inner_ok, after_check_bb),
        ]);
        Ok(phi.as_basic_value().into_int_value())
    }

    fn codegen_ref_is_instance_of_nonnull(
        &mut self,
        at: crate::span::Span,
        obj: PointerValue<'ctx>,
        target_ty: TypeId,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match self.types.kind(target_ty) {
            TypeKind::Ref(RefTypeKind::Any) => Ok(self.context.bool_type().const_int(1, false)),
            TypeKind::Ref(RefTypeKind::String) => {
                let desc = self.get_or_create_string_type_desc_global(at)?;
                let target_i8 = desc.as_pointer_value().const_cast(self.llvm_i8_ptr_type());
                self.codegen_type_desc_chain_contains_target(at, obj, target_i8)
            }
            TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
                // interface：用 itable 中预计算的 runtime target match 集判断是否可赋值到目标实例。
                if self.interfaces.contains_key(&nominal.fqn) {
                    let target_type_id = stable_hash64(
                        StableHashScope::RttiV0,
                        &self.types.display(target_ty).to_string(),
                    );
                    return self.codegen_itable_contains_runtime_type_id(at, obj, target_type_id);
                }

                // class：沿 parent 链查找。
                let class_lookup_key = if nominal.args.is_empty() {
                    self.class_inits
                        .contains_key(&nominal.fqn)
                        .then(|| nominal.fqn.clone())
                } else {
                    let mangled = self.nominal_layout_key(nominal);
                    self.class_inits.contains_key(&mangled).then_some(mangled)
                };
                if let Some(class_fqn) = class_lookup_key {
                    let desc = self.get_or_create_class_type_desc_global(at, &class_fqn)?;
                    let target_i8 = desc.as_pointer_value().const_cast(self.llvm_i8_ptr_type());
                    return self.codegen_type_desc_chain_contains_target(at, obj, target_i8);
                }

                if self.object_inits.contains_key(&nominal.fqn) {
                    let desc =
                        self.get_or_create_object_singleton_type_desc_global(at, &nominal.fqn)?;
                    let target_i8 = desc.as_pointer_value().const_cast(self.llvm_i8_ptr_type());
                    return self.codegen_type_desc_chain_contains_target(at, obj, target_i8);
                }

                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "type check target (nominal ref)",
                    at: at.into(),
                })
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "type check target type",
                at: at.into(),
            }),
        }
    }

    /// `class` 类型判断：检查 `obj.header.type_desc` 的 parent 链是否包含 `target_desc_i8`。
    fn codegen_type_desc_chain_contains_target(
        &mut self,
        at: crate::span::Span,
        obj: PointerValue<'ctx>,
        target_desc_i8: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        // 读取 `header.type_desc`（i8*）。
        let header_ty = self.llvm_gc_object_header_type();
        let header_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let header_ptr = self
            .builder
            .build_pointer_cast(obj, header_ptr_ty, "isa_hdr_ptr")?;
        let type_desc_ptr =
            self.builder
                .build_struct_gep(header_ty, header_ptr, 1, "isa_type_desc_gep")?;
        let type_desc_i8 = self
            .builder
            .build_load(i8_ptr_ty, type_desc_ptr, "isa_type_desc")?
            .into_pointer_value();

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: at.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: at.into(),
            })?;

        // while (cur != NULL) { if (cur == target) return true; cur = cur.parent; } return false
        let loop_bb = self.context.append_basic_block(func, "isa_loop");
        let check_bb = self.context.append_basic_block(func, "isa_check");
        let advance_bb = self.context.append_basic_block(func, "isa_advance");
        let hit_bb = self.context.append_basic_block(func, "isa_hit");
        let done_bb = self.context.append_basic_block(func, "isa_done");

        self.builder.build_unconditional_branch(loop_bb)?;
        self.builder.position_at_end(loop_bb);

        let cur_phi = self.builder.build_phi(i8_ptr_ty, "isa_cur")?;
        cur_phi.add_incoming(&[(&type_desc_i8, insert_block)]);
        let cur_i8 = cur_phi.as_basic_value().into_pointer_value();

        let cur_is_null = self.builder.build_is_null(cur_i8, "isa_cur_is_null")?;
        self.builder
            .build_conditional_branch(cur_is_null, done_bb, check_bb)?;

        // check：cur == target ?
        self.builder.position_at_end(check_bb);
        let word_ty = self.int_type(IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        });
        let cur_int = self
            .builder
            .build_ptr_to_int(cur_i8, word_ty, "isa_cur_int")?;
        let target_int =
            self.builder
                .build_ptr_to_int(target_desc_i8, word_ty, "isa_target_int")?;
        let eq = self
            .builder
            .build_int_compare(IntPredicate::EQ, cur_int, target_int, "isa_eq")?;
        self.builder
            .build_conditional_branch(eq, hit_bb, advance_bb)?;

        // advance：cur = cur.parent
        self.builder.position_at_end(advance_bb);
        let desc_ty = self.llvm_scoop_type_descriptor_type();
        let desc_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let cur_desc = self
            .builder
            .build_pointer_cast(cur_i8, desc_ptr_ty, "isa_desc")?;
        let parent_ptr = self
            .builder
            .build_struct_gep(desc_ty, cur_desc, 11, "isa_parent_gep")?;
        let parent_desc = self
            .builder
            .build_load(desc_ptr_ty, parent_ptr, "isa_parent")?
            .into_pointer_value();
        let parent_i8 = self
            .builder
            .build_pointer_cast(parent_desc, i8_ptr_ty, "isa_parent_i8")?;
        cur_phi.add_incoming(&[(&parent_i8, advance_bb)]);
        self.builder.build_unconditional_branch(loop_bb)?;

        // hit：return true
        self.builder.position_at_end(hit_bb);
        self.builder.build_unconditional_branch(done_bb)?;

        // done：phi 合并 true/false
        self.builder.position_at_end(done_bb);
        let phi = self
            .builder
            .build_phi(self.context.bool_type(), "isa_found")?;
        phi.add_incoming(&[
            (&self.context.bool_type().const_int(0, false), loop_bb),
            (&self.context.bool_type().const_int(1, false), hit_bb),
        ]);
        Ok(phi.as_basic_value().into_int_value())
    }

    /// `interface` 类型判断：扫描 `obj.type_desc.itable` 中各 entry 的 runtime match set，
    /// 只要其中任意一个 target type id 与 `target_type_id` 相等，就判定为 true。
    fn codegen_itable_contains_runtime_type_id(
        &mut self,
        at: crate::span::Span,
        obj: PointerValue<'ctx>,
        target_type_id: u64,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        // obj 指向对象头起始地址：先把它 cast 为 `ScoopGcObjectHeader*`。
        let header_ty = self.llvm_gc_object_header_type();
        let header_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let header_ptr =
            self.builder
                .build_pointer_cast(obj, header_ptr_ty, "isa_iface_hdr_ptr")?;

        // header.type_desc : i8*
        let type_desc_ptr =
            self.builder
                .build_struct_gep(header_ty, header_ptr, 1, "isa_iface_type_desc_gep")?;
        let type_desc_i8 = self
            .builder
            .build_load(i8_ptr_ty, type_desc_ptr, "isa_iface_load_type_desc")?
            .into_pointer_value();

        // type_desc.itable : i8*
        let desc_ty = self.llvm_scoop_type_descriptor_type();
        let desc_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let desc_ptr =
            self.builder
                .build_pointer_cast(type_desc_i8, desc_ptr_ty, "isa_iface_type_desc")?;
        let itable_field_ptr =
            self.builder
                .build_struct_gep(desc_ty, desc_ptr, 12, "isa_iface_itable_gep")?;
        let itable_i8 = self
            .builder
            .build_load(i8_ptr_ty, itable_field_ptr, "isa_iface_load_itable")?
            .into_pointer_value();

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: at.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: at.into(),
            })?;

        // itable == NULL -> false
        let itable_is_null = self
            .builder
            .build_is_null(itable_i8, "isa_iface_itable_is_null")?;
        let null_bb = self
            .context
            .append_basic_block(func, "isa_iface_itable_null");
        let lookup_bb = self
            .context
            .append_basic_block(func, "isa_iface_itable_lookup");
        let done_bb = self
            .context
            .append_basic_block(func, "isa_iface_itable_done");
        self.builder
            .build_conditional_branch(itable_is_null, null_bb, lookup_bb)?;

        self.builder.position_at_end(null_bb);
        self.builder.build_unconditional_branch(done_bb)?;

        // lookup：扫描 entries[idx].runtime_match_type_ids
        self.builder.position_at_end(lookup_bb);
        let itable_ty = self.llvm_scoop_itable_type();
        let itable_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let itable_ptr =
            self.builder
                .build_pointer_cast(itable_i8, itable_ptr_ty, "isa_iface_itable_ptr")?;

        let len_ptr =
            self.builder
                .build_struct_gep(itable_ty, itable_ptr, 0, "isa_iface_len_gep")?;
        let len_i32 = self
            .builder
            .build_load(i32_ty, len_ptr, "isa_iface_len")?
            .into_int_value();

        let entry_ty = self.llvm_scoop_itable_entry_type();
        let entries_field_ptr =
            self.builder
                .build_struct_gep(itable_ty, itable_ptr, 2, "isa_iface_entries_gep")?;
        let entry_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let entries_base = self.builder.build_pointer_cast(
            entries_field_ptr,
            entry_ptr_ty,
            "isa_iface_entries",
        )?;

        let loop_bb = self.context.append_basic_block(func, "isa_iface_loop");
        let body_bb = self.context.append_basic_block(func, "isa_iface_body");
        let hit_bb = self.context.append_basic_block(func, "isa_iface_hit");
        let miss_bb = self.context.append_basic_block(func, "isa_iface_miss");

        self.builder.build_unconditional_branch(loop_bb)?;
        self.builder.position_at_end(loop_bb);

        let idx_phi = self.builder.build_phi(i32_ty, "isa_iface_idx")?;
        idx_phi.add_incoming(&[(&i32_ty.const_zero(), lookup_bb)]);
        let idx_i32 = idx_phi.as_basic_value().into_int_value();

        let cond = self.builder.build_int_compare(
            IntPredicate::ULT,
            idx_i32,
            len_i32,
            "isa_iface_idx_lt_len",
        )?;
        self.builder
            .build_conditional_branch(cond, body_bb, done_bb)?;

        // body：线性扫描当前 entry 的 runtime_match_type_ids。
        self.builder.position_at_end(body_bb);
        let entry_ptr = unsafe {
            self.builder.build_in_bounds_gep(
                entry_ty,
                entries_base,
                &[idx_i32],
                "isa_iface_entry_ptr",
            )?
        };
        let match_len_ptr =
            self.builder
                .build_struct_gep(entry_ty, entry_ptr, 1, "isa_iface_match_len_gep")?;
        let match_len_i32 = self
            .builder
            .build_load(i32_ty, match_len_ptr, "isa_iface_match_len")?
            .into_int_value();
        let match_ids_ptr =
            self.builder
                .build_struct_gep(entry_ty, entry_ptr, 3, "isa_iface_match_ids_gep")?;
        let match_ids_i8 = self
            .builder
            .build_load(i8_ptr_ty, match_ids_ptr, "isa_iface_match_ids")?
            .into_pointer_value();

        let entry_match_ids_null = self
            .builder
            .build_is_null(match_ids_i8, "isa_iface_match_ids_is_null")?;
        let entry_match_len_zero = self.builder.build_int_compare(
            IntPredicate::EQ,
            match_len_i32,
            i32_ty.const_zero(),
            "isa_iface_match_len_is_zero",
        )?;
        let entry_match_empty = self.builder.build_or(
            entry_match_ids_null,
            entry_match_len_zero,
            "isa_iface_match_empty",
        )?;
        let entry_match_lookup_bb = self
            .context
            .append_basic_block(func, "isa_iface_match_lookup");
        self.builder
            .build_conditional_branch(entry_match_empty, miss_bb, entry_match_lookup_bb)?;

        self.builder.position_at_end(entry_match_lookup_bb);
        let match_ids_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let match_ids_base = self.builder.build_pointer_cast(
            match_ids_i8,
            match_ids_ptr_ty,
            "isa_iface_match_ids_base",
        )?;
        let match_loop_bb = self
            .context
            .append_basic_block(func, "isa_iface_match_loop");
        let match_body_bb = self
            .context
            .append_basic_block(func, "isa_iface_match_body");
        let match_done_miss_bb = self
            .context
            .append_basic_block(func, "isa_iface_match_done_miss");
        self.builder.build_unconditional_branch(match_loop_bb)?;

        self.builder.position_at_end(match_loop_bb);
        let match_idx_phi = self.builder.build_phi(i32_ty, "isa_iface_match_idx")?;
        match_idx_phi.add_incoming(&[(&i32_ty.const_zero(), entry_match_lookup_bb)]);
        let match_idx_i32 = match_idx_phi.as_basic_value().into_int_value();
        let match_cond = self.builder.build_int_compare(
            IntPredicate::ULT,
            match_idx_i32,
            match_len_i32,
            "isa_iface_match_idx_lt_len",
        )?;
        self.builder
            .build_conditional_branch(match_cond, match_body_bb, match_done_miss_bb)?;

        self.builder.position_at_end(match_body_bb);
        let match_slot_ptr = unsafe {
            self.builder.build_in_bounds_gep(
                i64_ty,
                match_ids_base,
                &[match_idx_i32],
                "isa_iface_match_slot_ptr",
            )?
        };
        let match_id_i64 = self
            .builder
            .build_load(i64_ty, match_slot_ptr, "isa_iface_match_id")?
            .into_int_value();
        let target_id = i64_ty.const_int(target_type_id, false);
        let ok = self.builder.build_int_compare(
            IntPredicate::EQ,
            match_id_i64,
            target_id,
            "isa_iface_match_id_eq",
        )?;
        let match_next_bb = self
            .context
            .append_basic_block(func, "isa_iface_match_next");
        self.builder
            .build_conditional_branch(ok, hit_bb, match_next_bb)?;

        self.builder.position_at_end(match_next_bb);
        let match_next = self.builder.build_int_add(
            match_idx_i32,
            i32_ty.const_int(1, false),
            "isa_iface_match_idx_next",
        )?;
        match_idx_phi.add_incoming(&[(&match_next, match_next_bb)]);
        self.builder.build_unconditional_branch(match_loop_bb)?;

        self.builder.position_at_end(match_done_miss_bb);
        self.builder.build_unconditional_branch(miss_bb)?;

        // miss：idx++ 继续 loop
        self.builder.position_at_end(miss_bb);
        let next = self.builder.build_int_add(
            idx_i32,
            i32_ty.const_int(1, false),
            "isa_iface_idx_next",
        )?;
        idx_phi.add_incoming(&[(&next, miss_bb)]);
        self.builder.build_unconditional_branch(loop_bb)?;

        // hit：直接 done
        self.builder.position_at_end(hit_bb);
        self.builder.build_unconditional_branch(done_bb)?;

        // done：phi 合并 false/true
        self.builder.position_at_end(done_bb);
        let phi = self
            .builder
            .build_phi(self.context.bool_type(), "isa_iface_found")?;
        phi.add_incoming(&[
            (&self.context.bool_type().const_int(0, false), null_bb),
            (&self.context.bool_type().const_int(0, false), loop_bb),
            (&self.context.bool_type().const_int(1, false), hit_bb),
        ]);
        Ok(phi.as_basic_value().into_int_value())
    }

    fn codegen_float_binary_same_type(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (l_raw, l_ty) =
            self.codegen_expr(lhs)?
                .as_float()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "float binary op lhs",
                    at: span.into(),
                })?;
        let (r_raw, r_ty) =
            self.codegen_expr(rhs)?
                .as_float()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "float binary op rhs",
                    at: span.into(),
                })?;

        let out_ty = self.unify_float_cg_types(lhs, l_ty, rhs, r_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "float binary op type",
                at: span.into(),
            },
        )?;

        let l = self.cast_float(l_raw, l_ty, out_ty)?;
        let r = self.cast_float(r_raw, r_ty, out_ty)?;

        let out = match op {
            ast::BinaryOp::Add => self.builder.build_float_add(l, r, "fadd")?,
            ast::BinaryOp::Sub => self.builder.build_float_sub(l, r, "fsub")?,
            ast::BinaryOp::Mul => self.builder.build_float_mul(l, r, "fmul")?,
            ast::BinaryOp::Div => self.builder.build_float_div(l, r, "fdiv")?,
            ast::BinaryOp::Rem => self.builder.build_float_rem(l, r, "frem")?,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "float binary op",
                    at: span.into(),
                });
            }
        };

        Ok(CgValue::float(out, out_ty))
    }

    fn codegen_int_binary_same_type(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let lhs_is_lit = matches!(lhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));
        let rhs_is_lit = matches!(rhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));

        let (l_raw, l_ty) =
            self.codegen_expr(lhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "integer binary op lhs",
                    at: span.into(),
                })?;
        let (r_raw, r_ty) =
            self.codegen_expr(rhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "integer binary op rhs",
                    at: span.into(),
                })?;

        let out_ty = unify_int_types(lhs_is_lit, l_ty, rhs_is_lit, r_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "integer binary op type",
                at: span.into(),
            },
        )?;

        let l = self.cast_int(l_raw, l_ty, out_ty)?;
        let r = self.cast_int(r_raw, r_ty, out_ty)?;

        let out = match op {
            ast::BinaryOp::Add => self.builder.build_int_add(l, r, "add")?,
            ast::BinaryOp::Sub => self.builder.build_int_sub(l, r, "sub")?,
            ast::BinaryOp::Mul => self.builder.build_int_mul(l, r, "mul")?,
            ast::BinaryOp::Div => {
                if out_ty.signed {
                    self.builder.build_int_signed_div(l, r, "sdiv")?
                } else {
                    self.builder.build_int_unsigned_div(l, r, "udiv")?
                }
            }
            ast::BinaryOp::Rem => {
                if out_ty.signed {
                    self.builder.build_int_signed_rem(l, r, "srem")?
                } else {
                    self.builder.build_int_unsigned_rem(l, r, "urem")?
                }
            }
            ast::BinaryOp::BitAnd => self.builder.build_and(l, r, "and")?,
            ast::BinaryOp::BitXor => self.builder.build_xor(l, r, "xor")?,
            ast::BinaryOp::BitOr => self.builder.build_or(l, r, "or")?,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "integer binary op",
                    at: span.into(),
                });
            }
        };

        Ok(CgValue::int(out, out_ty))
    }

    fn codegen_shift(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (lhs_value, lhs_ty) =
            self.codegen_expr(lhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "shift lhs type",
                    at: span.into(),
                })?;

        let rhs_value =
            self.codegen_expr(rhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "shift rhs type",
                    at: span.into(),
                })?;

        let shift_count = self.mask_shift_count(lhs_ty, rhs_value.0)?;

        let out = match op {
            ast::BinaryOp::Shl => self
                .builder
                .build_left_shift(lhs_value, shift_count, "shl")?,
            ast::BinaryOp::Shr => {
                self.builder
                    .build_right_shift(lhs_value, shift_count, lhs_ty.signed, "shr")?
            }
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "shift operator",
                    at: span.into(),
                });
            }
        };

        Ok(CgValue::int(out, lhs_ty))
    }

    fn mask_shift_count(
        &mut self,
        lhs_ty: IntTy,
        rhs: IntValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let lhs_bits = lhs_ty.bits;
        let lhs_int = self.int_type(lhs_ty);

        // 1) 截断为 lhs 的位宽（只取低位，后续再 mask）。
        let rhs_trunc = self
            .builder
            .build_int_truncate(rhs, lhs_int, "shift_rhs_trunc")?;

        // 2) mask：shiftCount & (bitWidth - 1)，避免 LLVM 对"超范围 shift"的 UB。
        let mask = lhs_int.const_int((lhs_bits.saturating_sub(1)) as u64, false);
        Ok(self.builder.build_and(rhs_trunc, mask, "shift_masked")?)
    }

    fn codegen_int_compare(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let lhs_is_lit = matches!(lhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));
        let rhs_is_lit = matches!(rhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));

        let (l_raw, l_ty) =
            self.codegen_expr(lhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "comparison lhs",
                    at: span.into(),
                })?;
        let (r_raw, r_ty) =
            self.codegen_expr(rhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "comparison rhs",
                    at: span.into(),
                })?;

        let int_ty = unify_int_types(lhs_is_lit, l_ty, rhs_is_lit, r_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "comparison operand type",
                at: span.into(),
            },
        )?;

        let l = self.cast_int(l_raw, l_ty, int_ty)?;
        let r = self.cast_int(r_raw, r_ty, int_ty)?;

        let pred = match (op, int_ty.signed) {
            (ast::BinaryOp::Lt, true) => IntPredicate::SLT,
            (ast::BinaryOp::Lt, false) => IntPredicate::ULT,
            (ast::BinaryOp::Le, true) => IntPredicate::SLE,
            (ast::BinaryOp::Le, false) => IntPredicate::ULE,
            (ast::BinaryOp::Gt, true) => IntPredicate::SGT,
            (ast::BinaryOp::Gt, false) => IntPredicate::UGT,
            (ast::BinaryOp::Ge, true) => IntPredicate::SGE,
            (ast::BinaryOp::Ge, false) => IntPredicate::UGE,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "comparison operator",
                    at: span.into(),
                });
            }
        };

        Ok(CgValue::bool(
            self.builder.build_int_compare(pred, l, r, "icmp")?,
        ))
    }

    fn codegen_float_compare(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (l_raw, l_ty) =
            self.codegen_expr(lhs)?
                .as_float()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "float comparison lhs",
                    at: span.into(),
                })?;
        let (r_raw, r_ty) =
            self.codegen_expr(rhs)?
                .as_float()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "float comparison rhs",
                    at: span.into(),
                })?;

        let float_ty = self.unify_float_cg_types(lhs, l_ty, rhs, r_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "float comparison operand type",
                at: span.into(),
            },
        )?;

        let l = self.cast_float(l_raw, l_ty, float_ty)?;
        let r = self.cast_float(r_raw, r_ty, float_ty)?;

        let pred = match op {
            ast::BinaryOp::Lt => FloatPredicate::OLT,
            ast::BinaryOp::Le => FloatPredicate::OLE,
            ast::BinaryOp::Gt => FloatPredicate::OGT,
            ast::BinaryOp::Ge => FloatPredicate::OGE,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "float comparison operator",
                    at: span.into(),
                });
            }
        };

        Ok(CgValue::bool(
            self.builder.build_float_compare(pred, l, r, "fcmp")?,
        ))
    }

    fn codegen_equality(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let lhs_is_lit = matches!(lhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));
        let rhs_is_lit = matches!(rhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));

        let lhs_v = self.codegen_expr(lhs)?;
        if lhs_v.ty == CgTy::String {
            let deferred_lhs =
                self.defer_gc_sensitive_cg_value(lhs.span, "string_eq_lhs", lhs_v)?;
            let rhs_v = self.codegen_expr(rhs)?;
            let lhs_v =
                self.materialize_deferred_cg_value(lhs.span, "string_eq_lhs_reload", deferred_lhs)?;

            if matches!((lhs_v.ty, rhs_v.ty), (CgTy::String, CgTy::String)) {
                let BasicValueEnum::PointerValue(l) =
                    lhs_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "equality lhs string value",
                        at: span.into(),
                    })?
                else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "equality lhs string type",
                        at: span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(r) =
                    rhs_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "equality rhs string value",
                        at: span.into(),
                    })?
                else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "equality rhs string type",
                        at: span.into(),
                    });
                };
                let fn_val = self.declare_runtime_string_equals();
                let call = self
                    .builder
                    .build_call(fn_val, &[l.into(), r.into()], "str_eq")?;
                let raw_result = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "String equals return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(eq_i64) = raw_result else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String equals return type",
                        at: span.into(),
                    });
                };
                let is_eq = self.builder.build_int_compare(
                    IntPredicate::NE,
                    eq_i64,
                    self.context.i64_type().const_zero(),
                    "str_eq_bool",
                )?;
                let result = match op {
                    ast::BinaryOp::Eq => is_eq,
                    ast::BinaryOp::Ne => self.builder.build_not(is_eq, "str_ne_bool")?,
                    _ => unreachable!("filtered by caller"),
                };
                return Ok(CgValue::bool(result));
            }

            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "string equality operand type",
                at: span.into(),
            });
        }
        let rhs_v = self.codegen_expr(rhs)?;

        // Bool == Bool
        if matches!((lhs_v.ty, rhs_v.ty), (CgTy::Bool, CgTy::Bool)) {
            let l = lhs_v.as_bool().unwrap();
            let r = rhs_v.as_bool().unwrap();
            let pred = match op {
                ast::BinaryOp::Eq => IntPredicate::EQ,
                ast::BinaryOp::Ne => IntPredicate::NE,
                _ => unreachable!("filtered by caller"),
            };
            return Ok(CgValue::bool(self.builder.build_int_compare(
                pred,
                l,
                r,
                "icmp_bool",
            )?));
        }

        // T0107: String == String — call scoop_string_equals(a, b) -> i64 (1=equal, 0=not)
        if matches!((lhs_v.ty, rhs_v.ty), (CgTy::String, CgTy::String)) {
            let BasicValueEnum::PointerValue(l) =
                lhs_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "equality lhs string value",
                    at: span.into(),
                })?
            else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "equality lhs string type",
                    at: span.into(),
                });
            };
            let BasicValueEnum::PointerValue(r) =
                rhs_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "equality rhs string value",
                    at: span.into(),
                })?
            else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "equality rhs string type",
                    at: span.into(),
                });
            };
            let fn_val = self.declare_runtime_string_equals();
            let call = self
                .builder
                .build_call(fn_val, &[l.into(), r.into()], "str_eq")?;
            let raw_result =
                call.try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "String equals return value",
                        at: span.into(),
                    })?;
            let BasicValueEnum::IntValue(eq_i64) = raw_result else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "String equals return type",
                    at: span.into(),
                });
            };
            let is_eq = self.builder.build_int_compare(
                IntPredicate::NE,
                eq_i64,
                self.context.i64_type().const_zero(),
                "str_eq_bool",
            )?;
            let result = match op {
                ast::BinaryOp::Eq => is_eq,
                ast::BinaryOp::Ne => self.builder.build_not(is_eq, "str_ne_bool")?,
                _ => unreachable!("filtered by caller"),
            };
            return Ok(CgValue::bool(result));
        }

        if let (Some((l_raw, l_ty)), Some((r_raw, r_ty))) = (lhs_v.as_float(), rhs_v.as_float()) {
            let float_ty = self.unify_float_cg_types(lhs, l_ty, rhs, r_ty).ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "equality float operand type",
                    at: span.into(),
                },
            )?;
            let l = self.cast_float(l_raw, l_ty, float_ty)?;
            let r = self.cast_float(r_raw, r_ty, float_ty)?;
            let pred = match op {
                ast::BinaryOp::Eq => FloatPredicate::OEQ,
                ast::BinaryOp::Ne => FloatPredicate::UNE,
                _ => unreachable!("filtered by caller"),
            };
            return Ok(CgValue::bool(
                self.builder.build_float_compare(pred, l, r, "fcmp_eq")?,
            ));
        }

        // Int == Int（含 int literal 吸收）
        let Some((l_raw, l_ty)) = lhs_v.as_int() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "equality lhs",
                at: span.into(),
            });
        };
        let Some((r_raw, r_ty)) = rhs_v.as_int() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "equality rhs",
                at: span.into(),
            });
        };

        let int_ty = unify_int_types(lhs_is_lit, l_ty, rhs_is_lit, r_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "equality operand type",
                at: span.into(),
            },
        )?;

        let l = self.cast_int(l_raw, l_ty, int_ty)?;
        let r = self.cast_int(r_raw, r_ty, int_ty)?;

        let pred = match op {
            ast::BinaryOp::Eq => IntPredicate::EQ,
            ast::BinaryOp::Ne => IntPredicate::NE,
            _ => unreachable!("filtered by caller"),
        };
        Ok(CgValue::bool(
            self.builder.build_int_compare(pred, l, r, "icmp_eq")?,
        ))
    }

    fn codegen_bool_logic(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let l = self
            .codegen_expr(lhs)?
            .as_bool()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "bool operator lhs",
                at: span.into(),
            })?;
        let r = self
            .codegen_expr(rhs)?
            .as_bool()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "bool operator rhs",
                at: span.into(),
            })?;

        let out = match op {
            ast::BinaryOp::LogAnd => self.builder.build_and(l, r, "and")?,
            ast::BinaryOp::LogOr => self.builder.build_or(l, r, "or")?,
            _ => unreachable!("filtered by caller"),
        };
        Ok(CgValue::bool(out))
    }

    fn coerce_value(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
        target: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match (value.ty, target) {
            // T1612: Nothing (bottom type) coerces to any target type.
            // This only occurs on unreachable paths (after Raise.raise, etc.),
            // so we return a phantom default value of the target type.
            (CgTy::Never, _) => self.default_value(at, target),
            (CgTy::Unit, CgTy::Unit) => Ok(CgValue::unit()),
            (CgTy::Unit, CgTy::Ref) => {
                // early stage：允许把 `Unit` 装箱到 `Any`。
                //
                // 说明：
                // - 当前阶段有一部分"语句位置"的表达式仍会被类型系统视为 `Any`（例如某些 `block/when`），
                //   因此后端需要支持 `Unit -> Any` 的值提升；
                // - v0 阶段 runtime type descriptor 仍是占位（NULL），这里只保证"可执行/可回归"。
                let boxed = self.codegen_box_unit_to_ref(at)?;
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(boxed.into()),
                })
            }
            (CgTy::Bool, CgTy::Bool) => Ok(value),
            (CgTy::Bool, CgTy::Int(int_ty)) => {
                let v = value.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "bool value",
                    at: at.into(),
                })?;
                let out =
                    self.builder
                        .build_int_z_extend(v, self.int_type(int_ty), "bool_to_int")?;
                Ok(CgValue::int(out, int_ty))
            }
            (CgTy::Bool, CgTy::Ref) => {
                // early stage：允许把 `Bool` 装箱到 `Any`（与 `Int -> Any` 一致）。
                //
                // 注意：
                // - 当前阶段 runtime type descriptor 仍是占位（NULL），因此这里只保证"可执行/可回归"，
                //   不承诺后续 runtime type casts 的可观察语义；
                // - 为复用现有 box 形态，这里把 `Bool` 扩展为 word-sized 无符号整数后按 int box 存储。
                let v = value.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "bool value",
                    at: at.into(),
                })?;
                let word = IntTy {
                    bits: self.host.word_bit_width(),
                    signed: false,
                };
                let widened =
                    self.builder
                        .build_int_z_extend(v, self.int_type(word), "box_bool_to_word")?;
                let boxed = self.codegen_box_int_to_ref(at, widened, word)?;
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(boxed.into()),
                })
            }
            (CgTy::Float64, CgTy::Float64) | (CgTy::Float32, CgTy::Float32) => Ok(value),
            (CgTy::Float64, CgTy::Float32) | (CgTy::Float32, CgTy::Float64) => {
                let (v, from) = value.as_float().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "float value",
                    at: at.into(),
                })?;
                let out = self.cast_float(v, from, target)?;
                Ok(CgValue::float(out, target))
            }
            (CgTy::Int(from), CgTy::Int(to)) => {
                if let Some(bits) = self.int_literal_bits_from_source_span_if_present(at, to)? {
                    return Ok(CgValue::int(self.int_type(to).const_int(bits, false), to));
                }
                let (v, _) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "int value",
                    at: at.into(),
                })?;
                let out = self.cast_int(v, from, to)?;
                Ok(CgValue::int(out, to))
            }
            (CgTy::String, CgTy::String) => Ok(value),
            (CgTy::String, CgTy::Ref) => {
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "string -> ref coercion value",
                        at: at.into(),
                    });
                };
                let BasicValueEnum::PointerValue(ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "string -> ref coercion type",
                        at: at.into(),
                    });
                };

                let casted = self.builder.build_pointer_cast(
                    ptr,
                    self.llvm_gc_i8_ptr_type(),
                    "str_to_ref",
                )?;
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(casted.into()),
                })
            }
            (CgTy::Ref, CgTy::Ref) => Ok(value),
            (CgTy::Int(_), CgTy::Ref) => {
                // T0817：值类型装箱到 `Any`（当前阶段先只支持整数族）。
                let (raw_int, from_ty) =
                    value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "int value",
                        at: at.into(),
                    })?;
                let boxed = self.codegen_box_int_to_ref(at, raw_int, from_ty)?;
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(boxed.into()),
                })
            }
            (CgTy::Enum(enum_ty), CgTy::Ref) => {
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum -> ref coercion value",
                        at: at.into(),
                    });
                };
                let boxed = self.codegen_box_enum_to_ref(at, enum_ty, raw)?;
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(boxed.into()),
                })
            }
            (CgTy::Tuple(from), CgTy::Tuple(to)) if from == to => Ok(value),
            (CgTy::Struct(from), CgTy::Struct(to)) if from == to => Ok(value),
            (CgTy::Enum(from), CgTy::Enum(to)) if from == to => Ok(value),
            (CgTy::String, CgTy::Enum(target_enum))
            | (CgTy::Ref, CgTy::Enum(target_enum))
            | (CgTy::Enum(_), CgTy::Enum(target_enum)) => {
                if let Some(coerced) =
                    self.try_coerce_pointer_like_to_option_enum(at, value, target_enum)?
                {
                    Ok(coerced)
                } else {
                    Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "value coercion pointer-like enum",
                        at: at.into(),
                    })
                }
            }
            (from, to) => Err(LlvmEmitError::Frontend {
                message: format!("unsupported value coercion from {from:?} to {to:?}"),
            }),
        }
    }

    fn try_coerce_pointer_like_to_option_enum(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
        target_enum: TypeId,
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Option(target_inner)) = self.types.kind(target_enum)
        else {
            return Ok(None);
        };

        if !matches!(
            self.cg_enum_layout(at, target_enum)?.repr,
            CgEnumRepr::Niche {
                storage: NicheStorage::Pointer,
                ..
            }
        ) {
            return Ok(None);
        }

        let target_inner_cg =
            self.cg_ty_of(*target_inner)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "Option<T> inner type",
                    at: at.into(),
                })?;

        let Some(raw) = value.value else {
            return Ok(None);
        };
        let BasicValueEnum::PointerValue(ptr) = raw else {
            return Ok(None);
        };

        match (value.ty, target_inner_cg) {
            (CgTy::Ref, CgTy::Ref) | (CgTy::String, CgTy::String) | (CgTy::String, CgTy::Ref) => {}
            (CgTy::Enum(source_enum), CgTy::Ref | CgTy::String)
                if matches!(
                    self.cg_enum_layout(at, source_enum)?.repr,
                    CgEnumRepr::Niche {
                        storage: NicheStorage::Pointer,
                        ..
                    }
                ) => {}
            _ => return Ok(None),
        }

        let target_llvm_ty = self.llvm_basic_type_of(at, CgTy::Enum(target_enum))?;
        let BasicTypeEnum::PointerType(ptr_ty) = target_llvm_ty else {
            return Ok(None);
        };

        let casted = self
            .builder
            .build_pointer_cast(ptr, ptr_ty, "option_ptr_coerce")?;
        Ok(Some(CgValue {
            ty: CgTy::Enum(target_enum),
            value: Some(casted.into()),
        }))
    }

    fn codegen_box_unit_to_ref(
        &mut self,
        at: crate::span::Span,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        // 约定（early stage）：
        // - box 对象布局：`{ header: ScoopGcObjectHeader }`（无 payload）
        // - 对象头字段由 runtime 的 `scoop_alloc` 初始化（与 `codegen_box_int_to_ref` 一致）。
        let boxed_ty = self.llvm_boxed_unit_type();
        let obj_size_bytes = self.target_data.get_store_size(&boxed_ty);

        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);

        let desc = self.get_or_create_boxed_unit_type_desc_global(at)?;
        let desc_i8 = self.builder.build_pointer_cast(
            desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "boxed_unit_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            at,
            rt_alloc,
            &[desc_i8.into(), size_v.into()],
            "rt_alloc_box_unit",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return value",
                at: at.into(),
            })?;

        let BasicValueEnum::PointerValue(raw_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return type",
                at: at.into(),
            });
        };

        Ok(raw_ptr)
    }

    fn codegen_box_int_to_ref(
        &mut self,
        at: crate::span::Span,
        value: IntValue<'ctx>,
        value_ty: IntTy,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        // 约定（early stage）：
        // - box 对象布局：`{ header: ScoopGcObjectHeader, payload: <int> }`（TODO T0908）
        // - 当前阶段由 runtime 的 `scoop_alloc_typed` 初始化对象头字段：
        //   - `next = NULL`
        //   - `type_desc = <boxed-int type desc>`
        //   - `size_bytes = alloc_size`
        //   - `flags/mark = 0`
        //
        // 注意：这里不尝试做"复用 box 类型"或 cache；LLVM named struct 会在 module 内复用。
        let target = self.target_layout();
        let payload_size = u64::from(value_ty.bits).div_ceil(8);
        let payload_align = payload_size.clamp(1, target.pointer_align.max(1));

        // 对象头布局与 C runtime 对齐（见 `runtime/c/scoop_gc.h` 的 static asserts）。
        //
        // `ScoopGcObjectHeader` 字段：
        // - next: void*
        // - type_desc: void*
        // - size_bytes: u64
        // - flags: u32
        // - mark: u32
        let header_size = 2 * target.pointer_size + 16;
        let header_align = target.pointer_align.max(8).max(1);
        let payload_offset = align_to(header_size, payload_align);
        let obj_align = header_align.max(payload_align);
        let total_size = align_to(payload_offset.saturating_add(payload_size), obj_align);

        let size_v = self.context.i64_type().const_int(total_size, false);

        let desc = self.get_or_create_boxed_int_type_desc_global(at, value_ty)?;
        let desc_i8 = self.builder.build_pointer_cast(
            desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "boxed_int_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            at,
            rt_alloc,
            &[desc_i8.into(), size_v.into()],
            "rt_alloc_box",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return value",
                at: at.into(),
            })?;

        let BasicValueEnum::PointerValue(raw_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return type",
                at: at.into(),
            });
        };

        // 写入 payload（对象头由 runtime 初始化）。
        let boxed_ty = self.llvm_boxed_int_type(value_ty);
        let boxed_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let boxed_ptr = self
            .builder
            .build_pointer_cast(raw_ptr, boxed_ptr_ty, "boxed_int_ptr")?;

        let payload_ptr =
            self.builder
                .build_struct_gep(boxed_ty, boxed_ptr, 1, "boxed_payload_gep")?;
        let _ = self.builder.build_store(payload_ptr, value)?;

        Ok(raw_ptr)
    }

    fn codegen_box_enum_to_ref(
        &mut self,
        at: crate::span::Span,
        enum_ty: TypeId,
        value: BasicValueEnum<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let payload_ty = self.llvm_basic_type_of(at, CgTy::Enum(enum_ty))?;
        let object_ty = self.llvm_boxed_enum_type(enum_ty, payload_ty);
        let object_size = self.target_data.get_store_size(&object_ty);
        let size_v = self.context.i64_type().const_int(object_size, false);
        let desc = self.get_or_create_boxed_enum_type_desc_global(at, enum_ty, object_ty)?;
        let desc_i8 = self.builder.build_pointer_cast(
            desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "boxed_enum_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            at,
            rt_alloc,
            &[desc_i8.into(), size_v.into()],
            "rt_alloc_box_enum",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed enum box return value",
                at: at.into(),
            })?;
        let BasicValueEnum::PointerValue(raw_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed enum box return type",
                at: at.into(),
            });
        };
        let obj_ptr = self.builder.build_pointer_cast(
            raw_ptr,
            self.llvm_ptr_type(self.gc_address_space()),
            "boxed_enum_ptr",
        )?;
        let payload_ptr =
            self.builder
                .build_struct_gep(object_ty, obj_ptr, 1, "boxed_enum_payload_gep")?;
        let _ = self.builder.build_store(payload_ptr, value)?;
        Ok(raw_ptr)
    }

    fn llvm_boxed_enum_type(
        &self,
        enum_ty: TypeId,
        payload_ty: BasicTypeEnum<'ctx>,
    ) -> StructType<'ctx> {
        let name = format!(
            "scoop.runtime.BoxedEnum__{}",
            sanitize_llvm_ident(&self.types.display(enum_ty).to_string()),
        );
        if let Some(existing) = self.context.get_struct_type(&name) {
            return existing;
        }
        let ty = self.context.opaque_struct_type(&name);
        let header_ty = self.llvm_gc_object_header_type();
        ty.set_body(&[header_ty.into(), payload_ty], false);
        ty
    }

    fn get_or_create_boxed_enum_type_desc_global(
        &mut self,
        at: crate::span::Span,
        enum_ty: TypeId,
        object_ty: StructType<'ctx>,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let name = sanitize_llvm_ident(&self.types.display(enum_ty).to_string());
        let global_name = format!("__scoop_type_desc_runtime__boxed_enum__{name}");
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(existing);
        }
        let trace_start_offset_bytes = self
            .target_data
            .offset_of_element(&object_ty, 1)
            .unwrap_or(0);
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &global_name,
            canonical_name: &format!("scoop.runtime.BoxedEnum<{}>", self.types.display(enum_ty)),
            obj_ty: object_ty,
            trace_start_offset_bytes,
            parent: None,
            itable: None,
            vtable: None,
        })
    }

    fn cast_int(
        &mut self,
        value: IntValue<'ctx>,
        from: IntTy,
        to: IntTy,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        if from.bits == to.bits {
            return Ok(value);
        }

        let to_ty = self.int_type(to);
        if to.bits > from.bits {
            if from.signed {
                Ok(self.builder.build_int_s_extend(value, to_ty, "sext")?)
            } else {
                Ok(self.builder.build_int_z_extend(value, to_ty, "zext")?)
            }
        } else {
            Ok(self.builder.build_int_truncate(value, to_ty, "trunc")?)
        }
    }

    fn cast_float(
        &mut self,
        value: FloatValue<'ctx>,
        from: CgTy,
        to: CgTy,
    ) -> Result<FloatValue<'ctx>, LlvmEmitError> {
        match (from, to) {
            (CgTy::Float64, CgTy::Float64) | (CgTy::Float32, CgTy::Float32) => Ok(value),
            (CgTy::Float32, CgTy::Float64) => {
                Ok(self
                    .builder
                    .build_float_ext(value, self.context.f64_type(), "fpext")?)
            }
            (CgTy::Float64, CgTy::Float32) => {
                Ok(self
                    .builder
                    .build_float_trunc(value, self.context.f32_type(), "fptrunc")?)
            }
            _ => unreachable!("cast_float only accepts Float64/Float32"),
        }
    }

    fn int_type(&self, ty: IntTy) -> IntType<'ctx> {
        self.context.custom_width_int_type(ty.bits)
    }

    fn get_or_create_global_bytes(
        &self,
        span: crate::span::Span,
        bytes: &[u8],
    ) -> GlobalValue<'ctx> {
        let name = format!("__scoop_str_data_{}_{}", span.start, span.end);
        if let Some(existing) = self.module.get_global(&name) {
            return existing;
        }

        let arr_ty = self.context.i8_type().array_type(bytes.len() as u32);
        let gv = self.module.add_global(arr_ty, None, &name);
        let init = self.context.const_string(bytes, false);
        gv.set_initializer(&init);
        gv.set_constant(true);
        gv
    }

    fn create_entry_alloca(
        &mut self,
        at: crate::span::Span,
        name: &str,
        ty: CgTy,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let alloca_ty = self.llvm_basic_type_of(at, ty)?;
        let ptr = self.create_entry_alloca_raw(at, name, alloca_ty)?;
        self.apply_alloca_alignment_for_ty(at, ptr, ty)?;
        Ok(ptr)
    }

    fn create_entry_alloca_raw(
        &mut self,
        at: crate::span::Span,
        name: &str,
        alloca_ty: BasicTypeEnum<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let frame_slots = self.reserve_explicit_frame_leaf_slots_for_storage_type(at, alloca_ty)?;
        let alloca_builder = self.context.create_builder();
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: at.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: at.into(),
            })?;
        let entry = func
            .get_first_basic_block()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "function has no entry block",
                at: at.into(),
            })?;

        match entry.get_first_instruction() {
            Some(inst) => alloca_builder.position_before(&inst),
            None => alloca_builder.position_at_end(entry),
        }

        let slot = alloca_builder.build_alloca(alloca_ty, name)?;
        self.record_explicit_frame_slot_mirrors(slot, frame_slots);
        Ok(slot)
    }

    fn create_entry_scratch_alloca_raw(
        &self,
        at: crate::span::Span,
        name: &str,
        alloca_ty: BasicTypeEnum<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let alloca_builder = self.context.create_builder();
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: at.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: at.into(),
            })?;
        let entry = func
            .get_first_basic_block()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "function has no entry block",
                at: at.into(),
            })?;

        match entry.get_first_instruction() {
            Some(inst) => alloca_builder.position_before(&inst),
            None => alloca_builder.position_at_end(entry),
        }

        Ok(alloca_builder.build_alloca(alloca_ty, name)?)
    }

    fn apply_alloca_alignment_for_ty(
        &self,
        at: crate::span::Span,
        ptr: PointerValue<'ctx>,
        ty: CgTy,
    ) -> Result<(), LlvmEmitError> {
        // `@CLayout(aligned = N)`：显式对齐仅对 struct 有意义，其它类型保持默认 ABI 对齐。
        let CgTy::Struct(struct_ty) = ty else {
            return Ok(());
        };
        let Some(aligned) = self.struct_clayout(struct_ty).and_then(|c| c.aligned) else {
            return Ok(());
        };

        let inst = ptr
            .as_instruction_value()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "alloca instruction value",
                at: at.into(),
            })?;
        inst.set_alignment(aligned)?;
        Ok(())
    }
}

fn is_direct_hir_closure_carrier_alias(callable_fqn: &str) -> bool {
    callable_fqn
        .strip_prefix("scoop.lambda$")
        .is_some_and(|suffix| !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()))
}

fn string_literal_parse_reason(err: StringLiteralParseError) -> &'static str {
    match err {
        StringLiteralParseError::Invalid => "包含无效引号、转义或 Unicode 码点",
        StringLiteralParseError::Interpolated => "插值字符串当前阶段不能直接按普通字符串解析",
        StringLiteralParseError::InvalidUtf8 => "解码后的字节不是有效 UTF-8",
    }
}

#[allow(dead_code)]
fn top_level_callee_resume_entry_fn_name(fun_fqn: &str) -> String {
    format!("__scoop_callee_resume__{fun_fqn}")
}

fn top_level_immutable_value_init_fn_name(value_fqn: &str) -> String {
    format!("__scoop_top_level_val_init__{value_fqn}")
}

fn refactor_hidden_top_level_immutable_value_init_bridge_fn_name(value_fqn: &str) -> String {
    format!("__scoop_refactor_hidden_top_level_init_bridge__{value_fqn}")
}

fn top_level_immutable_value_guard_global_name(value_fqn: &str) -> String {
    format!("__scoop_top_level_val_guard__{value_fqn}")
}

fn top_level_immutable_value_global_name(value_fqn: &str) -> String {
    format!("__scoop_top_level_val__{value_fqn}")
}

fn top_level_var_global_name(var_fqn: &str) -> String {
    format!("__scoop_top_level_var__{var_fqn}")
}

fn pointer_value_key<'ctx>(ptr: PointerValue<'ctx>) -> usize {
    ptr.as_value_ref() as usize
}

fn align_to(value: u64, align: u64) -> u64 {
    if align <= 1 {
        return value;
    }
    let mask = align - 1;
    (value + mask) & !mask
}

fn largest_two_sizes(layouts: &[TypeLayout]) -> (u64, u64) {
    let mut max = 0u64;
    let mut second = 0u64;
    for l in layouts {
        let s = l.size;
        if s >= max {
            second = max;
            max = s;
            continue;
        }
        if s > second {
            second = s;
        }
    }
    (max, second)
}

fn unify_int_types(
    lhs_is_lit: bool,
    lhs_ty: IntTy,
    rhs_is_lit: bool,
    rhs_ty: IntTy,
) -> Option<IntTy> {
    if lhs_ty == rhs_ty {
        return Some(lhs_ty);
    }
    if lhs_is_lit {
        return Some(rhs_ty);
    }
    if rhs_is_lit {
        return Some(lhs_ty);
    }
    None
}

fn parse_tuple_member_index(text: &str) -> Option<u32> {
    let digits = text.strip_prefix('_')?;
    if digits.is_empty() {
        return None;
    }
    if !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u32>().ok()
}

fn sanitize_llvm_ident(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() { "_".to_string() } else { out }
}

fn explicit_root_frame_type_name(function_symbol: &str) -> String {
    format!(
        "scoop.runtime.ScoopExplicitRootFrame${}",
        sanitize_llvm_ident(function_symbol)
    )
}

fn explicit_root_frame_offsets_global_name(function_symbol: &str) -> String {
    format!(
        "__scoop_explicit_root_offsets__{}",
        sanitize_llvm_ident(function_symbol)
    )
}

fn explicit_root_frame_desc_global_name(function_symbol: &str) -> String {
    format!(
        "__scoop_explicit_root_desc__{}",
        sanitize_llvm_ident(function_symbol)
    )
}

fn mask_to_bits(value: u128, bits: u32) -> u128 {
    if bits == 0 {
        return 0;
    }
    if bits >= 128 {
        return value;
    }
    let mask = (1u128 << bits) - 1;
    value & mask
}

fn checked_positive_int_literal_bits(value: u128, int_ty: IntTy) -> Option<u128> {
    let max = if int_ty.signed {
        signed_int_max(int_ty.bits)
    } else {
        unsigned_int_max(int_ty.bits)
    };
    (value <= max).then_some(value)
}

fn checked_negated_int_literal_bits(value: u128, int_ty: IntTy) -> Option<u128> {
    if !int_ty.signed {
        return None;
    }

    let min_abs = signed_int_min_abs(int_ty.bits);
    if value > min_abs {
        return None;
    }

    Some(mask_to_bits(0u128.wrapping_sub(value), int_ty.bits))
}

fn unsigned_int_max(bits: u32) -> u128 {
    if bits == 0 {
        return 0;
    }
    if bits >= 128 {
        return u128::MAX;
    }
    (1u128 << bits) - 1
}

fn signed_int_max(bits: u32) -> u128 {
    if bits <= 1 {
        return 0;
    }
    if bits >= 128 {
        return i128::MAX as u128;
    }
    (1u128 << (bits - 1)) - 1
}

fn signed_int_min_abs(bits: u32) -> u128 {
    if bits == 0 {
        return 0;
    }
    if bits >= 128 {
        return 1u128 << 127;
    }
    1u128 << (bits - 1)
}

fn source_text_int_literal_body(text: &str) -> Option<(bool, &str)> {
    let (negative, body) = if let Some(rest) = text.strip_prefix('-') {
        (true, rest)
    } else {
        (false, text)
    };

    if body.is_empty() || !body.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        return None;
    }

    if let Some(rest) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        if rest
            .bytes()
            .all(|b| char::from(b).is_ascii_hexdigit() || b == b'_')
        {
            return Some((negative, body));
        }
        return None;
    }

    if let Some(rest) = body.strip_prefix("0b").or_else(|| body.strip_prefix("0B")) {
        if rest.bytes().all(|b| matches!(b, b'0' | b'1' | b'_')) {
            return Some((negative, body));
        }
        return None;
    }

    if body
        .bytes()
        .all(|b| char::from(b).is_ascii_digit() || b == b'_')
    {
        return Some((negative, body));
    }

    None
}

fn parse_f_string_text_bytes(raw: bool, text: &str) -> Result<Vec<u8>, StringLiteralParseError> {
    // f-string 的 Text 片段来自 parser 拆分后的"内容区间 slice"，不包含包裹引号。
    // 这里需要补齐两类语义：
    // - `{{` / `}}`：字面量大括号（spec §8.2）；
    // - 非 raw f-string：支持最小转义（与普通字符串一致）。
    if raw {
        let undoubled = undouble_braces(text);
        return Ok(undoubled.into_bytes());
    }

    // 非 raw：先在源码层"去双大括号"，并避免把 `\u{...}` 的 `{}` 当作候选；
    // 再复用普通字符串的转义解析。
    let undoubled = undouble_braces_preserving_escapes(text);
    parse_normal_string_bytes(&undoubled)
}

fn undouble_braces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' if matches!(chars.peek(), Some('{')) => {
                let _ = chars.next();
                out.push('{');
            }
            '}' if matches!(chars.peek(), Some('}')) => {
                let _ = chars.next();
                out.push('}');
            }
            _ => out.push(ch),
        }
    }
    out
}

fn undouble_braces_preserving_escapes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            // 转义序列中的 `{`/`}` 不参与 `{{`/`}}` 的消解。
            out.push('\\');
            let Some(next) = chars.next() else {
                break;
            };
            out.push(next);

            // `\u{...}`：把整个 `{...}` 视为转义语法的一部分，原样拷贝。
            if next == 'u' && matches!(chars.peek(), Some('{')) {
                out.push(chars.next().expect("peek 已保证存在"));
                for c in chars.by_ref() {
                    out.push(c);
                    if c == '}' {
                        break;
                    }
                }
            }
            continue;
        }

        match ch {
            '{' if matches!(chars.peek(), Some('{')) => {
                let _ = chars.next();
                out.push('{');
            }
            '}' if matches!(chars.peek(), Some('}')) => {
                let _ = chars.next();
                out.push('}');
            }
            _ => out.push(ch),
        }
    }

    out
}
