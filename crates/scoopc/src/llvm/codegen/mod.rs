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
//! `T5000c` 已将 `HirFacts` / `EffectAnalysisCtx` / `ExprFactResolver` 这类 shared facts
//! 抽离到 backend 外的共享层；这里当前只消费这些 backend-agnostic 输入，并继续朝
//! “只做 backend lowering”的边界收口。后续 `T5000d+` 将让 early MIR / summary 直接复用
//! 同一层共享事实，而不是回到 LLVM 现场拼装分析输入。

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::path::{Path, PathBuf};
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
use crate::cone::SourceConeInfo;
use crate::effect::state_machine::CalleeSuspendPlan;
use crate::expr_facts::{ExprFactResolver, HirFactResolver};
use crate::hir;
use crate::llvm::target::HostTargetInfo;
use crate::source::{SourceFile, SourceId, SourceMap};
use crate::stable_id::{
    AbiMangler, CanonicalTextKey, PrivateSymbolMangler, StableCanonicalKey, StableClosureKey,
    StableConeKey, StableDefKey, StableDefNamespace, StableTypeParamKey,
    canonical_callable_signature_key, canonical_record, canonical_type_text,
    stable_rtti_derived_type_key, stable_rtti_type_id, stable_rtti_type_id_for_type,
};
use crate::syntax::int_literal::{parse_int_literal, parse_int_literal_checked};
use crate::syntax::string_literal::{StringLiteralParseError, parse_string_literal_bytes};
use crate::ty::layout::{NicheStorage, TypeLayout};
use crate::ty::{
    BuiltinTypes, NominalType, RefTypeKind, TypeId, TypeKind, TypeParamType, TypeStore,
    ValueTypeKind,
};
use scoopc_hir_facts::{HirFacts, declarations::NominalKind as HirFactNominalKind};
use scoopc_lir_facts::{
    LirExternGlobalLinkage, LirFacts, LirGlobalRootFacts, LirGlobalRootKey, LirGlobalRootKind,
    LirGlobalStoragePolicy,
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

#[derive(Clone, Copy)]
struct InterfaceItableSlotLookup<'ctx> {
    fn_i8: PointerValue<'ctx>,
    receiver_type_id: inkwell::values::IntValue<'ctx>,
}

#[derive(Clone)]
struct InterfaceValueReceiverCase {
    receiver_type_id: u64,
    source_ty: TypeId,
    impl_fqn: String,
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
struct NativeParamAbi<'ctx> {
    llvm_param_ty: BasicMetadataTypeEnum<'ctx>,
}

#[derive(Clone, Copy)]
struct NativeReturnAbi<'ctx> {
    cg_ty: CgTy,
    llvm_return_ty: Option<BasicTypeEnum<'ctx>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NativeAggregateReturnMode {
    TargetAbiDirect,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NativeBoundaryMode {
    EnterLeaveNative,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NativeEffectBoundaryPolicy {
    PlainNativeLeaf,
}

#[derive(Clone)]
struct NativeCallableAbi<'ctx> {
    param_abis: Vec<NativeParamAbi<'ctx>>,
    return_abi: NativeReturnAbi<'ctx>,
    fn_ty: FunctionType<'ctx>,
    aggregate_return_mode: NativeAggregateReturnMode,
    call_convention: u32,
    boundary_mode: NativeBoundaryMode,
    gc_leaf_function: bool,
    effect_boundary_policy: NativeEffectBoundaryPolicy,
}

#[derive(Clone, Copy)]
enum NativeCallableOrigin<'a> {
    DirectExtern { callable_fqn: &'a str },
    FunPtr,
}

#[derive(Clone, Copy)]
enum NativeCallableTarget<'ctx> {
    Direct(FunctionValue<'ctx>),
    Indirect {
        fn_ty: FunctionType<'ctx>,
        ptr: PointerValue<'ctx>,
        call_name: &'static str,
    },
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

#[derive(Clone, Copy)]
struct FunPtrCallSpec<'a> {
    span: crate::span::Span,
    callee_span: crate::span::Span,
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
    exported_abi_symbols: RefCell<HashMap<String, ExportedAbiSymbolReservation>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExportedAbiSymbolReservation {
    canonical_key: String,
    owner_label: String,
}

fn reserve_exported_abi_symbol_in_registry(
    registry: &RefCell<HashMap<String, ExportedAbiSymbolReservation>>,
    symbol: &str,
    canonical_key: String,
    owner_label: String,
) -> Result<(), String> {
    let mut registry = registry.borrow_mut();
    if let Some(existing) = registry.get(symbol) {
        if existing.canonical_key != canonical_key {
            return Err(format!(
                "exported ABI symbol collision: `{symbol}` already belongs to {} (canonical key `{}`), but {} tried to reuse it with canonical key `{canonical_key}`",
                existing.owner_label, existing.canonical_key, owner_label,
            ));
        }
        return Ok(());
    }
    registry.insert(
        symbol.to_string(),
        ExportedAbiSymbolReservation {
            canonical_key,
            owner_label,
        },
    );
    Ok(())
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
    stable_cone_key: &'a StableConeKey,
    source_cones: &'a HashMap<PathBuf, SourceConeInfo>,
    stable_type_param_keys: &'a HashMap<TypeParamType, StableTypeParamKey>,
    types: &'a TypeStore,
    struct_layouts: &'a hir::StructLayoutIndex,
    enum_layouts: &'a hir::EnumLayoutIndex,
    top_level_vars: &'a hir::TopLevelVarIndex,
    top_level_immutable_values: &'a hir::TopLevelImmutableValueIndex,
    top_level_fun_call_sites: &'a hir::TopLevelFunCallSiteIndex,
    extern_funs: &'a hir::ExternFunIndex,
    native_callable_funs: &'a hir::NativeCallableFunIndex,
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
    /// LIR-owned backend-neutral contracts for global init/storage and callable ABI.
    published_lir_facts: &'a LirFacts,
    /// HIR barrier 发布的 declaration/entity facts。
    hir_facts: Rc<HirFacts>,
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
    current_stable_owner_key: Option<StableDefKey>,
    current_stable_closure_path_prefix: Option<String>,
    next_stable_child_closure_index: usize,
    stable_closure_paths: HashMap<hir::ClosureId, String>,
    loop_context_stack: Vec<LoopContext<'ctx>>,
    return_context: Option<ReturnContext<'ctx>>,
    current_sret_return_ptr: Option<PointerValue<'ctx>>,
    current_effect_ctx_ref: Option<PointerValue<'ctx>>,
    current_incoming_resume_token_ref: Option<PointerValue<'ctx>>,
    current_effect_outcome_ptr: Option<PointerValue<'ctx>>,
    local_effect_escape_targets: Vec<inkwell::basic_block::BasicBlock<'ctx>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConeInitRootKind {
    TopLevelImmutableValue,
    TopLevelVar,
}

#[derive(Debug, Clone)]
pub(super) struct ConeInitRoot {
    kind: ConeInitRootKind,
    fqn: String,
    storage: Option<LirGlobalStoragePolicy>,
}

#[derive(Debug, Clone)]
pub(super) struct ConeInitRoutinePlan {
    function_name: String,
    roots: Vec<ConeInitRoot>,
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
    hir_facts: &HirFacts,
) -> HashMap<String, Vec<TypeId>> {
    let mut by_effect_fqn: HashMap<String, Vec<TypeId>> = HashMap::new();
    let effect_fqns = hir_facts
        .declarations
        .nominals
        .iter()
        .filter(|nominal| nominal.kind == HirFactNominalKind::Effect)
        .map(|nominal| nominal.identity.display_name.as_str())
        .collect::<HashSet<_>>();

    for type_id in types.iter_ids() {
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = types.kind(type_id) else {
            continue;
        };
        if !effect_fqns.contains(nominal.fqn.as_str()) {
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
    pub(super) stable_cone_key: &'a StableConeKey,
    pub(super) source_cones: &'a HashMap<PathBuf, SourceConeInfo>,
    pub(super) stable_type_param_keys: &'a HashMap<TypeParamType, StableTypeParamKey>,
    pub(super) types: &'a TypeStore,
    pub(super) struct_layouts: &'a hir::StructLayoutIndex,
    pub(super) enum_layouts: &'a hir::EnumLayoutIndex,
    pub(super) top_level_vars: &'a hir::TopLevelVarIndex,
    pub(super) top_level_immutable_values: &'a hir::TopLevelImmutableValueIndex,
    pub(super) top_level_fun_call_sites: &'a hir::TopLevelFunCallSiteIndex,
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
    pub(super) native_callable_funs: &'a hir::NativeCallableFunIndex,
    pub(super) fun_index: &'a HashMap<String, &'a hir::FunDecl>,
    pub(super) materialized_pass_view: Option<crate::mir::MaterializedMirPassView<'a>>,
    pub(super) published_late_lowered_program:
        Option<&'a crate::effect_lowered::LateLoweredProgram>,
    pub(super) published_lir_facts: &'a LirFacts,
    pub(super) hir_facts: Rc<HirFacts>,
    pub(super) effect_op_tags: Rc<RefCell<EffectOpTagState>>,
}

pub(super) struct TypeDescriptorSpec<'ctx, 'a> {
    pub(super) at: crate::span::Span,
    pub(super) global_name: &'a str,
    pub(super) type_id_key: &'a str,
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
            stable_cone_key,
            source_cones,
            stable_type_param_keys,
            types,
            struct_layouts,
            enum_layouts,
            top_level_vars,
            top_level_immutable_values,
            top_level_fun_call_sites,
            native_callable_funs,
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
            published_lir_facts,
            hir_facts,
            effect_op_tags,
        } = inputs;
        let known_effect_instances_by_effect_fqn =
            collect_known_effect_instance_types_by_effect_fqn(types, hir_facts.as_ref());
        Self {
            context,
            module,
            builder,
            target_data,
            host,
            source_map,
            entry_source_id,
            stable_cone_key,
            source_cones,
            stable_type_param_keys,
            types,
            struct_layouts,
            enum_layouts,
            top_level_vars,
            top_level_immutable_values,
            top_level_fun_call_sites,
            extern_funs,
            native_callable_funs,
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
            published_lir_facts,
            hir_facts,
            shared_caches: SharedCodegenCaches::default(),
            effect_op_tags,
            known_effect_instances_by_effect_fqn,
        }
    }

    pub(super) fn cone_init_routine_plans(&self) -> Vec<ConeInitRoutinePlan> {
        let global_init = &self.published_lir_facts.global_init;
        global_init
            .final_entry_order
            .routines
            .iter()
            .map(|routine_key| {
                let routine = global_init.cone_init_routines.get(routine_key).unwrap_or_else(|| {
                    panic!(
                        "cone_init_routine_plans: LIR facts verifier accepted missing cone init routine {}",
                        routine_key.as_u32()
                    )
                });
                let roots = routine
                    .roots
                    .iter()
                    .map(|root_key| {
                        let root = global_init.roots.get(root_key).unwrap_or_else(|| {
                            panic!(
                                "cone_init_routine_plans: LIR facts verifier accepted missing global root `{}`",
                                root_key.as_str()
                            )
                        });
                        let kind = match root.kind {
                            LirGlobalRootKind::TopLevelImmutableVal => {
                                ConeInitRootKind::TopLevelImmutableValue
                            }
                            LirGlobalRootKind::TopLevelMutableVar => ConeInitRootKind::TopLevelVar,
                            LirGlobalRootKind::ObjectSingleton | LirGlobalRootKind::ExternGlobal => {
                                panic!(
                                    "cone_init_routine_plans: LIR facts verifier accepted non-eager global root `{}` in cone init routine",
                                    root_key.as_str()
                                )
                            }
                        };
                        ConeInitRoot {
                            kind,
                            fqn: root_key.as_str().to_string(),
                            storage: root.storage,
                        }
                    })
                    .collect();
                ConeInitRoutinePlan {
                    function_name: private_cone_init_fn_name(&routine.cone),
                    roots,
                }
            })
            .collect()
    }

    pub(super) fn thread_local_init_routine_plans(&self) -> Vec<ConeInitRoutinePlan> {
        self.cone_init_routine_plans()
            .into_iter()
            .filter_map(|plan| {
                let roots = plan
                    .roots
                    .into_iter()
                    .filter(|root| {
                        root.kind == ConeInitRootKind::TopLevelVar
                            && root.storage == Some(LirGlobalStoragePolicy::ThreadLocal)
                    })
                    .collect::<Vec<_>>();
                (!roots.is_empty()).then(|| ConeInitRoutinePlan {
                    function_name: private_thread_local_cone_init_fn_name(&plan.function_name),
                    roots,
                })
            })
            .collect()
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

    pub(super) fn stable_type_param_resolver(&self) -> &HashMap<TypeParamType, StableTypeParamKey> {
        self.stable_type_param_keys
    }
}

impl<'a, 'ctx> Deref for MainCodegen<'a, 'ctx> {
    type Target = CompilationUnitCodegenCx<'a, 'ctx>;

    fn deref(&self) -> &Self::Target {
        self.shared
    }
}

mod main;

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

pub(crate) fn private_closure_body_fn_name(closure_key: &StableClosureKey) -> String {
    PrivateSymbolMangler.mangle("closure_body", closure_key)
}

pub(crate) fn private_closure_resume_fn_name(closure_key: &StableClosureKey) -> String {
    PrivateSymbolMangler.mangle("closure_resume", closure_key)
}

pub(crate) fn private_closure_env_type_name(closure_key: &StableClosureKey) -> String {
    PrivateSymbolMangler.mangle("closure_env", closure_key)
}

pub(crate) fn private_closure_env_type_desc_name(closure_key: &StableClosureKey) -> String {
    PrivateSymbolMangler.mangle("closure_env_type_desc", closure_key)
}

fn private_top_level_immutable_value_init_fn_name(stable_key: &StableDefKey) -> String {
    PrivateSymbolMangler.mangle("top_level_val_init", stable_key)
}

fn private_top_level_immutable_value_init_bridge_fn_name(stable_key: &StableDefKey) -> String {
    PrivateSymbolMangler.mangle("hidden_top_level_init_bridge", stable_key)
}

fn private_top_level_immutable_value_guard_global_name(stable_key: &StableDefKey) -> String {
    PrivateSymbolMangler.mangle("top_level_val_guard", stable_key)
}

fn private_top_level_immutable_value_global_name(stable_key: &StableDefKey) -> String {
    PrivateSymbolMangler.mangle("top_level_val", stable_key)
}

fn private_top_level_var_global_name(stable_key: &StableDefKey) -> String {
    PrivateSymbolMangler.mangle("top_level_var", stable_key)
}

fn private_cone_init_fn_name(stable_key: &StableConeKey) -> String {
    let readable = sanitize_llvm_ident(stable_key.name());
    let hash = PrivateSymbolMangler.hash_suffix("cone_init", stable_key);
    format!("__scoop_priv0__cone_init__{readable}__h{hash}")
}

fn private_thread_local_cone_init_fn_name(cone_init_name: &str) -> String {
    format!("{cone_init_name}__thread_local")
}

fn pointer_value_key<'ctx>(ptr: PointerValue<'ctx>) -> usize {
    ptr.as_value_ref() as usize
}

// Walk HIR in lexical order so materialized-MIR closure helpers can recover the
// same `$lambdaN.$lambdaM` path without reusing `ClosureId` or old symbol text.
fn callable_export_readable_path(owner_path: &str) -> &str {
    let base = owner_path
        .rsplit_once("::<")
        .map(|(base, _)| base)
        .unwrap_or(owner_path);
    base.split_once("$overload$")
        .map(|(base, _)| base)
        .unwrap_or(base)
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

#[cfg(test)]
mod exported_abi_symbol_registry_tests {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::{ExportedAbiSymbolReservation, reserve_exported_abi_symbol_in_registry};
    use crate::stable_id::{StableCanonicalKey, StableConeKey, StableDefKey, StableDefNamespace};

    #[test]
    fn exported_abi_symbol_registry_allows_authoritative_reuse_from_multiple_paths() {
        let registry = RefCell::new(HashMap::<String, ExportedAbiSymbolReservation>::new());
        let key = StableDefKey::new(
            StableConeKey::new("sample", "0.1.0"),
            StableDefNamespace::Fun,
            "sample.helper",
            "non_generic_callable",
            Some("sig$arity0".to_string()),
        );

        reserve_exported_abi_symbol_in_registry(
            &registry,
            "__scoop_abi0_fun__sample_helper__hdeadbeef",
            key.canonical_text(),
            "HIR declaration path".to_string(),
        )
        .expect("初次注册应成功");
        reserve_exported_abi_symbol_in_registry(
            &registry,
            "__scoop_abi0_fun__sample_helper__hdeadbeef",
            key.canonical_text(),
            "materialized declaration path".to_string(),
        )
        .expect("同一 authoritative key 的重复声明路径应允许复用");
    }

    #[test]
    fn exported_abi_symbol_registry_rejects_conflicting_canonical_owners() {
        let registry = RefCell::new(HashMap::<String, ExportedAbiSymbolReservation>::new());
        let left = StableDefKey::new(
            StableConeKey::new("sample", "0.1.0"),
            StableDefNamespace::Fun,
            "sample.left",
            "non_generic_callable",
            Some("sig$arity0".to_string()),
        );
        let right = StableDefKey::new(
            StableConeKey::new("sample", "0.1.0"),
            StableDefNamespace::Fun,
            "sample.right",
            "non_generic_callable",
            Some("sig$arity0".to_string()),
        );

        reserve_exported_abi_symbol_in_registry(
            &registry,
            "__scoop_abi0_fun__sample_collision__hdeadbeef",
            left.canonical_text(),
            "source callable `sample.left`".to_string(),
        )
        .expect("初次注册应成功");
        let err = reserve_exported_abi_symbol_in_registry(
            &registry,
            "__scoop_abi0_fun__sample_collision__hdeadbeef",
            right.canonical_text(),
            "source callable `sample.right`".to_string(),
        )
        .expect_err("不同 canonical key 复用同一 ABI symbol 应显式失败");
        assert!(
            err.contains("collision") && err.contains("sample.right"),
            "冲突报错应说明发生了 exported ABI symbol collision，实际: {err}"
        );
    }
}
