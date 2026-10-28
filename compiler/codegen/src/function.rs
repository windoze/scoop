use super::*;

mod abi_parts;
mod call;
mod gc;
mod instruction;
mod interface_values;
mod local_storage;
mod memory;
mod roots;
mod thread_address;

/// Per-function emission state: everything instruction translation
/// needs, bundled to keep signatures small.
struct FnEmitter<'a, 'ctx> {
    profile: ValidatedBackendProfile,
    context: &'ctx Context,
    managed_address_space: ManagedAddressSpace,
    optimize_allocations: bool,
    optimize_reference_writes: bool,
    direct_array_regions: bool,
    allocation_initialization_limit: u64,
    reuse_thread_addresses: bool,
    llvm: &'a LlvmModule<'ctx>,
    builder: &'a inkwell::builder::Builder<'ctx>,
    function: &'a Function,
    llvm_function: inkwell::values::FunctionValue<'ctx>,
    current_block: scoop_lir::BlockId,
    current_instruction: usize,
    /// Entry block; enum temporaries are alloca'd here (see
    /// `entry_alloca`).
    entry_block: inkwell::basic_block::BasicBlock<'ctx>,
    /// Every LLVM basic block of the function, indexed by `BlockId`
    /// (invoke targets).
    llvm_blocks: &'a [inkwell::basic_block::BasicBlock<'ctx>],
    functions: &'a [Function],
    structs: &'a StructDefs,
    enums: &'a EnumDefs,
    extern_functions: &'a ExternFunctions,
    external_callables: &'a Arena<scoop_lir::ExternalCallable>,
    native_globals: &'a Arena<NativeGlobal>,
    native_global_bridges: &'a scoop_lir::NativeGlobalBridges,
    callback_bridges: &'a Arena<scoop_lir::CallbackBridge>,
    foreign_callback_families: &'a Arena<scoop_lir::ForeignCallbackFamily>,
    foreign_callback_bridges: &'a Arena<scoop_lir::ForeignCallbackBridge>,
    globals_arena: &'a Arena<Global>,
    globals: &'a BTreeMap<scoop_lir::GlobalId, GlobalValue<'ctx>>,
    initialization_units: &'a BTreeMap<scoop_lir::InitializationUnitId, GlobalValue<'ctx>>,
    /// Complete array metadata and descriptor globals, indexed directly by
    /// `ArrayTypeId`.
    arrays: &'a Arena<ArrayType>,
    array_tds: &'a BTreeMap<ArrayTypeId, GlobalValue<'ctx>>,
    type_descriptors: &'a Arena<scoop_lir::TypeDescriptor>,
    type_tds: &'a BTreeMap<scoop_lir::TypeDescriptorId, GlobalValue<'ctx>>,
    external_type_tds: &'a BTreeMap<scoop_lir::ExternalTypeDescriptorId, GlobalValue<'ctx>>,
    root_scans: Vec<PointerValue<'ctx>>,
    runtime_scans: crate::callable_runtime_scans::CallableRuntimeScanEmitter<'a, 'ctx>,
    target_data: &'a inkwell::targets::TargetData,
    /// Hidden result pointer for a physically indirect aggregate return.
    return_slot: Option<PointerValue<'ctx>>,
    allocas: HashMap<scoop_lir::LocalId, local_storage::LocalAllocation<'ctx>>,
    temps: HashMap<TempId, BasicValueEnum<'ctx>>,
    /// Canonical addressable storage for every parameter/temporary named by
    /// a complete LIR root plan. Locals reuse their ordinary alloca. All
    /// post-safepoint uses reload from this storage, so values remain valid
    /// across arbitrary CFG joins instead of relying on a block-global SSA
    /// cache entry.
    root_storage: HashMap<scoop_lir::CallerRootSource, RootStorage<'ctx>>,
    /// Fresh interface snapshots belong only to one straight-line segment.
    interface_values: interface_values::StoredInterfaces<'ctx>,
    /// Sources used by each unwind successor, transposed from the complete
    /// per-invoke edge flags before LLVM block emission.
    unwind_root_sources: HashMap<scoop_lir::BlockId, Vec<scoop_lir::CallerRootSource>>,
    /// Every landingpad reached by an invoke has one dynamic compiler frame;
    /// NoGc invokes publish an empty one so cleanup remains predecessor-free.
    compiler_unwind_blocks: HashSet<scoop_lir::BlockId>,
    compiler_invoke_index: u32,
    allocation_index: u32,
    /// The attached thread address stays stable throughout this invocation.
    cached_poll_state: Option<PointerValue<'ctx>>,
    /// Lazily initialized AS0 context address; its mutable fields are not cached.
    allocation_context_slot: Option<PointerValue<'ctx>>,
    /// Native address of the existing Context root; never caches its value.
    task_context_slot_cache: Option<PointerValue<'ctx>>,
    /// Collected only when allocation initialization can be optimized.
    allocation_addressed_locals: Option<HashSet<scoop_lir::LocalId>>,
    /// Nursery membership along one uninterrupted initialization path.
    young_allocation: Option<gc::YoungAllocation<'ctx>>,
    /// One ordinary field-write region, never retained across control flow or
    /// operations that can change the reference or publish/collect heap state.
    field_barrier: Option<gc::FieldBarrierCache<'ctx>>,
    /// Checked array-size failures share one block per callable-owned message.
    array_size_trap_blocks: HashMap<scoop_lir::GlobalId, inkwell::basic_block::BasicBlock<'ctx>>,
}

struct NativeTransition<'ctx> {
    frame: PointerValue<'ctx>,
    transition: PointerValue<'ctx>,
}

#[derive(Clone, Copy)]
struct RootStorage<'ctx> {
    pointer: PointerValue<'ctx>,
    ty: BasicTypeEnum<'ctx>,
}

struct ReloadedRoot<'ctx> {
    storage: RootStorage<'ctx>,
    value: BasicValueEnum<'ctx>,
}

impl FnEmitter<'_, '_> {
    fn safepoint_id(&self, site: scoop_lir::SafepointSiteRef) -> scoop_lir::SafepointId {
        self.function.safepoints[site].runtime_id()
    }
}

struct CompilerRootFrame<'ctx> {
    pointer: PointerValue<'ctx>,
}

struct StatepointLiveLeaf<'ctx> {
    storage: PointerValue<'ctx>,
    value: PointerValue<'ctx>,
    source: scoop_lir::CallerRootSource,
    byte_offset: u64,
    restore: bool,
}

struct MaterializedStatepointLive<'ctx> {
    arguments: HashMap<scoop_lir::CallerRootSource, BasicValueEnum<'ctx>>,
    leaves: Vec<StatepointLiveLeaf<'ctx>>,
}

impl<'ctx> MaterializedStatepointLive<'ctx> {
    fn interface_object(&self, value: Value) -> Option<PointerValue<'ctx>> {
        let source = match value {
            Value::Param(index) => scoop_lir::CallerRootSource::Param(index),
            Value::Local(id) => scoop_lir::CallerRootSource::Local(id),
            Value::Temp(id) => scoop_lir::CallerRootSource::Temp(id),
            _ => return None,
        };
        self.leaves
            .iter()
            .find(|leaf| leaf.source == source && leaf.byte_offset == 0)
            .map(|leaf| leaf.value)
    }
}

#[derive(Clone, Copy)]
enum NativeTransitionKind {
    Safe,
    Borrowed,
}

enum CallProtocol<'a> {
    Managed {
        safepoint: scoop_lir::SafepointId,
        live: &'a scoop_lir::StatepointLiveSet,
    },
    ManagedInvoke {
        safepoint: scoop_lir::SafepointId,
        roots: &'a scoop_lir::ExceptionalRootSet,
    },
    NoGc,
    NativeGcLeaf,
    ReleaseNativeLeaf,
    NativeSafe {
        safepoint: scoop_lir::SafepointId,
        roots: &'a scoop_lir::NativeSafeRootSet,
    },
    NativeBorrowed {
        safepoint: scoop_lir::SafepointId,
        roots: &'a scoop_lir::NativeBorrowedRootSet,
        result: scoop_lir::NativeBorrowedResultPublication<'a>,
    },
}

impl CallProtocol<'_> {
    fn safepoint(&self) -> Option<scoop_lir::SafepointId> {
        match self {
            Self::Managed { safepoint, .. }
            | Self::ManagedInvoke { safepoint, .. }
            | Self::NativeSafe { safepoint, .. }
            | Self::NativeBorrowed { safepoint, .. } => Some(*safepoint),
            Self::NoGc | Self::NativeGcLeaf | Self::ReleaseNativeLeaf => None,
        }
    }
}

enum TypedCallResult<'a> {
    Void,
    ElidedZst {
        out: TempId,
        value: &'a scoop_lir::AbiZst,
    },
    Direct {
        out: TempId,
        value: &'a scoop_lir::AbiDirectValue,
    },
    Indirect {
        storage: scoop_lir::LocalId,
        value: &'a scoop_lir::AbiValue,
        convention: scoop_lir::IndirectResultConvention,
    },
}

fn result_scan<'a>(result: &TypedCallResult<'a>) -> &'a RefScan {
    match result {
        TypedCallResult::Void | TypedCallResult::ElidedZst { .. } => &RefScan::None,
        TypedCallResult::Direct { value, .. } => value.scan(),
        TypedCallResult::Indirect { value, .. } => value.scan(),
    }
}

pub(super) fn emit_function<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    builder: &inkwell::builder::Builder<'ctx>,
    module_ctx: &ModuleCtx<'_, 'ctx>,
    function: &Function,
    surface: &scoop_lir::ObjectSymbolSurfaceV1,
    runtime_scan_plan: &scoop_lir::StrongCallableRuntimeScanPlanV1,
) -> Result<(), CodegenError> {
    // Pre-declared in the first pass of the selected object member.
    let llvm_function = llvm
        .get_function(function.symbol())
        .expect("function declared in the first pass");
    let debug_locations = module_ctx.debug_info.and_then(|debug| {
        crate::debug_info::attach_function(context, debug, function, llvm_function)
    });
    if let Some((_, unknown)) = debug_locations {
        builder.set_current_debug_location(unknown);
    }

    // A function containing a landing pad needs Scoop's closed-profile
    // personality (M25, runtime spec 5). Setting it also makes LLVM emit the
    // unwind table entry; the runtime-private record remains opaque here.
    let has_landing_pad = function.blocks.iter().any(|(_, block)| {
        block.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::LandingPad { .. } | Instruction::CleanupPad { .. }
            )
        })
    });
    if has_landing_pad {
        let personality_symbol = scoop_lir::TargetEhSupportV1::ScoopPersonality
            .logical_symbol_for_target(module_ctx.profile.lir_target_profile());
        let personality = llvm.get_function(personality_symbol).unwrap_or_else(|| {
            llvm.add_function(
                personality_symbol,
                context.i32_type().fn_type(&[], true),
                None,
            )
        });
        llvm_function.set_personality_function(personality);
    }

    // All blocks up front so terminators can reference them in any order.
    let mut blocks = vec![None; function.blocks.len()];
    let entry_index = arena_index(function.entry);
    blocks[entry_index] =
        Some(context.append_basic_block(llvm_function, &function.blocks[function.entry].name));
    for (id, block) in function.blocks.iter() {
        let index = arena_index(id);
        if index != entry_index {
            blocks[index] = Some(context.append_basic_block(llvm_function, &block.name));
        }
    }
    let blocks: Vec<_> = blocks
        .into_iter()
        .map(|block| block.expect("every LIR block is created"))
        .collect();

    let return_slot = match function.signature.result() {
        scoop_lir::AbiReturn::Indirect(_) => Some(
            llvm_function
                .get_nth_param(0)
                .expect("indirect-result function has its sret parameter")
                .into_pointer_value(),
        ),
        scoop_lir::AbiReturn::UnitVoid
        | scoop_lir::AbiReturn::ElidedZst(_)
        | scoop_lir::AbiReturn::Direct(_) => None,
    };
    let (unwind_root_sources, compiler_unwind_blocks) = compiler_unwind_plan(function);
    let mut runtime_scans = crate::callable_runtime_scans::CallableRuntimeScanEmitter::new(
        context,
        llvm,
        surface,
        runtime_scan_plan,
    );
    let mut root_scans = Vec::with_capacity(function.call_targets.root_scans.len());
    for (_, scan) in function.call_targets.root_scans.iter() {
        root_scans.push(
            runtime_scans
                .emit(scan)?
                .unwrap_or_else(|| crate::callable_runtime_scans::null_runtime_scan(context)),
        );
    }

    let mut emitter = FnEmitter {
        profile: module_ctx.profile,
        context,
        managed_address_space: module_ctx.managed_address_space,
        optimize_allocations: module_ctx.optimize_allocations,
        optimize_reference_writes: module_ctx.optimize_reference_writes,
        direct_array_regions: module_ctx.direct_array_regions,
        allocation_initialization_limit: module_ctx.allocation_initialization_limit,
        reuse_thread_addresses: module_ctx.reuse_thread_addresses,
        llvm,
        builder,
        function,
        llvm_function,
        current_block: function.entry,
        current_instruction: 0,
        entry_block: blocks[arena_index(function.entry)],
        llvm_blocks: &blocks,
        functions: module_ctx.functions,
        structs: module_ctx.structs,
        enums: module_ctx.enums,
        extern_functions: module_ctx.extern_functions,
        external_callables: module_ctx.external_callables,
        native_globals: module_ctx.native_globals,
        native_global_bridges: module_ctx.native_global_bridges,
        callback_bridges: module_ctx.callback_bridges,
        foreign_callback_families: module_ctx.foreign_callback_families,
        foreign_callback_bridges: module_ctx.foreign_callback_bridges,
        globals_arena: module_ctx.globals_arena,
        globals: module_ctx.globals,
        initialization_units: module_ctx.initialization_units,
        arrays: module_ctx.arrays,
        array_tds: module_ctx.array_tds,
        type_descriptors: module_ctx.type_descriptors,
        type_tds: module_ctx.type_tds,
        external_type_tds: module_ctx.external_type_tds,
        root_scans,
        runtime_scans,
        target_data: module_ctx.target_data,
        return_slot,
        allocas: HashMap::with_capacity(function.locals.len()),
        temps: HashMap::new(),
        root_storage: HashMap::new(),
        interface_values: interface_values::StoredInterfaces::new(
            module_ctx.reuse_interface_values,
        ),
        unwind_root_sources,
        compiler_unwind_blocks,
        compiler_invoke_index: 0,
        allocation_index: 0,
        cached_poll_state: None,
        allocation_context_slot: None,
        task_context_slot_cache: None,
        allocation_addressed_locals: None,
        young_allocation: None,
        field_barrier: None,
        array_size_trap_blocks: HashMap::new(),
    };

    // All locals are stack slots allocated at the top of the entry block;
    // LLVM's mem2reg promotes them. Entry is empty at this point, so
    // positioning at its end places the allocas before every instruction.
    builder.position_at_end(blocks[arena_index(function.entry)]);
    emitter.allocate_locals()?;
    emitter.prepare_root_storage()?;
    let mut initialization_successors = None;
    let mut young_successors = HashMap::new();
    for (block_id, block) in function.blocks.iter() {
        emitter.current_block = block_id;
        emitter.field_barrier = None;
        emitter.young_allocation = young_successors.remove(&block_id);
        emitter.interface_values.clear();
        builder.position_at_end(blocks[arena_index(block_id)]);
        // Invoke is an LLVM terminator even though it
        // are LIR instructions: one must be the last instruction of its
        // block, and the block's LIR terminator must be the redundant
        // `Br` to the invoke's normal target (kept so dumps stay
        // readable); it is not emitted. A LandingPad instruction must be
        // the first of its block (LLVM requires the landingpad first).
        let mut invoke_terminated = false;
        for (index, instruction) in block.instructions.iter().enumerate() {
            if let Some((entry, unknown)) = debug_locations {
                builder.set_current_debug_location(if block_id == function.entry && index == 0 {
                    entry
                } else {
                    unknown
                });
            }
            emitter.current_instruction = index;
            let is_invoke = matches!(instruction, Instruction::Invoke { .. });
            if is_invoke && index + 1 != block.instructions.len() {
                return Err(CodegenError(format!(
                    "invoke @{}: must be the last instruction of block {}",
                    function.symbol(),
                    block.name
                )));
            }
            if matches!(
                instruction,
                Instruction::LandingPad { .. } | Instruction::CleanupPad { .. }
            ) && index != 0
            {
                return Err(CodegenError(format!(
                    "landing pad @{}: must be the first instruction of block {}",
                    function.symbol(),
                    block.name
                )));
            }
            emitter.instruction(instruction)?;
            if !is_invoke {
                for temp in instruction_temp_defs(instruction).into_iter().flatten() {
                    emitter.sync_root_temp(temp)?;
                }
            }
            invoke_terminated = is_invoke;
        }
        if invoke_terminated {
            let normal = match block.instructions.last() {
                Some(Instruction::Invoke { site }) => site.normal(),
                _ => continue,
            };
            match &block.terminator {
                Terminator::Br(target) if *target == normal => {}
                _ => {
                    return Err(CodegenError(format!(
                        "invoke block @{}:{}: terminator must be `br` to the invoke's normal target",
                        function.symbol(),
                        block.name
                    )));
                }
            }
            continue;
        }
        if let Some((entry, unknown)) = debug_locations {
            builder.set_current_debug_location(
                if block_id == function.entry && block.instructions.is_empty() {
                    entry
                } else {
                    unknown
                },
            );
        }
        match &block.terminator {
            Terminator::Br(target) => {
                builder
                    .build_unconditional_branch(blocks[arena_index(*target)])
                    .map_err(|e| CodegenError(format!("br @{}: {e}", block.name)))?;
                if let Some(young) = emitter.young_allocation.take()
                    && arena_index(*target) > arena_index(block_id)
                    && initialization_successors
                        .get_or_insert_with(|| gc::single_predecessor_blocks(function))
                        .contains(target)
                {
                    young_successors.insert(*target, young);
                }
            }
            Terminator::CondBr {
                cond,
                then_block,
                else_block,
            } => {
                // LLVM integer values share one wrapper, so preserve the
                // logical LIR condition type before materializing it.
                let cond_ty = function.value_ty(module_ctx.globals_arena, *cond);
                if cond_ty != LirType::I1 {
                    return Err(CodegenError(format!(
                        "cbr @{} has condition type {}, expected i1",
                        block.name,
                        cond_ty.dump()
                    )));
                }
                let cond = emitter.value(*cond)?.into_int_value();
                builder
                    .build_conditional_branch(
                        cond,
                        blocks[arena_index(*then_block)],
                        blocks[arena_index(*else_block)],
                    )
                    .map_err(|e| CodegenError(format!("cbr @{}: {e}", block.name)))?;
            }
            Terminator::Return { value } => {
                let logical_result_ty = function.signature.result().logical_storage_type();
                match (value, logical_result_ty) {
                    (Some(value), Some(expected)) => {
                        let actual = function.value_ty(module_ctx.globals_arena, *value);
                        if &actual != expected {
                            return Err(CodegenError(format!(
                                "ret @{} has value type {}, but function returns {}",
                                function.symbol(),
                                actual.dump(),
                                expected.dump()
                            )));
                        }
                    }
                    (None, Some(expected)) => {
                        return Err(CodegenError(format!(
                            "ret @{} has no value, but function returns {}",
                            function.symbol(),
                            expected.dump()
                        )));
                    }
                    (Some(_), None) => {
                        return Err(CodegenError(format!(
                            "ret @{} has a value, but function returns void",
                            function.symbol()
                        )));
                    }
                    (None, None) => {}
                }

                match function.signature.result() {
                    scoop_lir::AbiReturn::UnitVoid | scoop_lir::AbiReturn::ElidedZst(_) => {
                        builder.build_return(None).map_err(|e| {
                            CodegenError(format!("ret @{}: {e}", function.symbol()))
                        })?;
                    }
                    scoop_lir::AbiReturn::Direct(plan) => {
                        let value = emitter.value(value.expect("direct result was validated"))?;
                        let value = emitter.encode_direct_result(plan, value)?;
                        builder.build_return(Some(&value)).map_err(|e| {
                            CodegenError(format!("ret @{}: {e}", function.symbol()))
                        })?;
                    }
                    scoop_lir::AbiReturn::Indirect(_) => {
                        let value = emitter.value(value.expect("indirect result was validated"))?;
                        let slot = emitter
                            .return_slot
                            .expect("indirect-result function has its sret parameter");
                        builder.build_store(slot, value).map_err(|e| {
                            CodegenError(format!("ret slot @{}: {e}", function.symbol()))
                        })?;
                        builder.build_return(None).map_err(|e| {
                            CodegenError(format!("ret @{}: {e}", function.symbol()))
                        })?;
                    }
                }
            }
            Terminator::Resume { exception } => {
                let exception_ty = function.value_ty(module_ctx.globals_arena, *exception);
                if exception_ty != LirType::ExceptionRecord {
                    return Err(CodegenError(format!(
                        "resume @{} requires exception_record, got {}",
                        function.symbol(),
                        exception_ty.dump()
                    )));
                }
                let exception = emitter.value(*exception)?;
                builder
                    .build_resume(exception)
                    .map_err(|e| CodegenError(format!("resume @{}: {e}", function.symbol())))?;
            }
            Terminator::Unreachable => {
                builder.build_unreachable().map_err(|e| {
                    CodegenError(format!("unreachable @{}: {e}", function.symbol()))
                })?;
            }
        }
    }
    builder.unset_current_debug_location();
    emitter.runtime_scans.finish()
}
