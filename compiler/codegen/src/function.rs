use super::*;

mod call;
mod instruction;
mod memory;
mod roots;

/// Per-function emission state: everything instruction translation
/// needs, bundled to keep signatures small.
struct FnEmitter<'a, 'ctx> {
    context: &'ctx Context,
    managed_address_space: ManagedAddressSpace,
    llvm: &'a LlvmModule<'ctx>,
    builder: &'a inkwell::builder::Builder<'ctx>,
    function: &'a Function,
    llvm_function: inkwell::values::FunctionValue<'ctx>,
    current_block: scoop_lir::BlockId,
    /// Entry block; enum temporaries are alloca'd here (see
    /// `entry_alloca`).
    entry_block: inkwell::basic_block::BasicBlock<'ctx>,
    /// Every LLVM basic block of the function, indexed by `BlockId`
    /// (invoke targets).
    llvm_blocks: &'a [inkwell::basic_block::BasicBlock<'ctx>],
    functions: &'a [Function],
    structs: &'a Arena<StructDef>,
    enums: &'a Arena<EnumDef>,
    extern_functions: &'a ExternFunctions,
    native_globals: &'a Arena<NativeGlobal>,
    native_global_bridges: &'a scoop_lir::NativeGlobalBridges,
    foreign_callback_bridges: &'a Arena<scoop_lir::ForeignCallbackBridge>,
    globals_arena: &'a Arena<Global>,
    globals: &'a [Option<GlobalValue<'ctx>>],
    /// Complete array metadata and descriptor globals, indexed directly by
    /// `ArrayTypeId`.
    arrays: &'a Arena<ArrayType>,
    array_tds: &'a [GlobalValue<'ctx>],
    type_tds: &'a [GlobalValue<'ctx>],
    external_type_tds: &'a [GlobalValue<'ctx>],
    root_scans: Vec<PointerValue<'ctx>>,
    target_data: &'a inkwell::targets::TargetData,
    /// Hidden result pointer for a physically indirect aggregate return.
    return_slot: Option<PointerValue<'ctx>>,
    /// LIR parameters start after the hidden result pointer when present.
    param_offset: u32,
    allocas: Vec<PointerValue<'ctx>>,
    temps: HashMap<TempId, BasicValueEnum<'ctx>>,
    /// Canonical addressable storage for every parameter/temporary named by
    /// a complete LIR root plan. Locals reuse their ordinary alloca. All
    /// post-safepoint uses reload from this storage, so values remain valid
    /// across arbitrary CFG joins instead of relying on a block-global SSA
    /// cache entry.
    root_storage: HashMap<scoop_lir::CallerRootSource, RootStorage<'ctx>>,
    /// Sources used by each unwind successor, transposed from the complete
    /// per-invoke edge flags before LLVM block emission.
    unwind_root_sources: HashMap<scoop_lir::BlockId, Vec<scoop_lir::CallerRootSource>>,
    /// Every landingpad reached by an invoke has one dynamic compiler frame;
    /// NoGc invokes publish an empty one so cleanup remains predecessor-free.
    compiler_unwind_blocks: HashSet<scoop_lir::BlockId>,
    native_call_index: u32,
    compiler_invoke_index: u32,
    allocation_index: u32,
    /// Lazily-created shared bounds-check trap block of this function
    /// (one per function, reused by every ArrayGet / ArraySet) and the
    /// module-level "array index out of bounds" message global it
    /// references (`Some` whenever the module uses arrays).
    bounds_trap_block: Option<inkwell::basic_block::BasicBlock<'ctx>>,
    bounds_message: Option<GlobalValue<'ctx>>,
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

struct CompilerRootFrame<'ctx> {
    pointer: PointerValue<'ctx>,
}

struct StatepointLiveLeaf<'ctx> {
    storage: PointerValue<'ctx>,
    value: PointerValue<'ctx>,
}

struct MaterializedStatepointLive<'ctx> {
    arguments: HashMap<scoop_lir::CallerRootSource, BasicValueEnum<'ctx>>,
    leaves: Vec<StatepointLiveLeaf<'ctx>>,
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
    NativeSafe {
        safepoint: scoop_lir::SafepointId,
        roots: &'a scoop_lir::NativeSafeRootSet,
    },
    NativeBorrowed {
        safepoint: scoop_lir::SafepointId,
        roots: &'a scoop_lir::NativeBorrowedRootSet,
    },
}

impl CallProtocol<'_> {
    fn safepoint(&self) -> Option<scoop_lir::SafepointId> {
        match self {
            Self::Managed { safepoint, .. }
            | Self::ManagedInvoke { safepoint, .. }
            | Self::NativeSafe { safepoint, .. }
            | Self::NativeBorrowed { safepoint, .. } => Some(*safepoint),
            Self::NoGc => None,
        }
    }
}

enum TypedCallResult<'a> {
    Void,
    Direct {
        out: TempId,
        ty: &'a LirType,
        scan: &'a RefScan,
    },
    Indirect {
        storage: scoop_lir::LocalId,
        ty: &'a LirType,
        scan: &'a RefScan,
    },
}

fn result_scan<'a>(result: &TypedCallResult<'a>) -> &'a RefScan {
    match result {
        TypedCallResult::Void => &RefScan::None,
        TypedCallResult::Direct { scan, .. } | TypedCallResult::Indirect { scan, .. } => scan,
    }
}

pub(super) fn emit_function<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    builder: &inkwell::builder::Builder<'ctx>,
    module_ctx: &ModuleCtx<'_, 'ctx>,
    function: &Function,
) -> Result<(), CodegenError> {
    // Pre-declared in the first pass (see `emit_object`).
    let llvm_function = llvm
        .get_function(&function.symbol)
        .expect("function declared in the first pass");

    // A function containing a landing pad needs a personality function
    // (M8, runtime spec 5): `scoop_eh_personality`, declared with the
    // same variadic prototype LLVM uses for `__gxx_personality_v0`.
    // Setting it also makes LLVM emit the unwind table entry.
    let has_landing_pad = function.blocks.iter().any(|(_, block)| {
        block.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::LandingPad { .. } | Instruction::CleanupPad { .. }
            )
        })
    });
    if has_landing_pad {
        let personality = llvm
            .get_function("scoop_eh_personality")
            .unwrap_or_else(|| {
                llvm.add_function(
                    "scoop_eh_personality",
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

    let param_offset = u32::from(uses_return_slot(module_ctx.enums, &function.return_ty));
    let return_slot = (param_offset != 0).then(|| {
        llvm_function
            .get_nth_param(0)
            .expect("return-slot function has its hidden parameter")
            .into_pointer_value()
    });
    let (unwind_root_sources, compiler_unwind_blocks) = compiler_unwind_plan(function);
    let root_scans = function
        .call_targets
        .root_scans
        .iter()
        .map(|(id, scan)| {
            emit_ref_scan(
                context,
                llvm,
                &format!("{}.root_scan.{}", function.symbol, id.into_raw()),
                scan,
            )
            .unwrap_or_else(|| ptr_ty(context).const_null())
        })
        .collect();

    let mut emitter = FnEmitter {
        context,
        managed_address_space: module_ctx.managed_address_space,
        llvm,
        builder,
        function,
        llvm_function,
        current_block: function.entry,
        entry_block: blocks[arena_index(function.entry)],
        llvm_blocks: &blocks,
        functions: module_ctx.functions,
        structs: module_ctx.structs,
        enums: module_ctx.enums,
        extern_functions: module_ctx.extern_functions,
        native_globals: module_ctx.native_globals,
        native_global_bridges: module_ctx.native_global_bridges,
        foreign_callback_bridges: module_ctx.foreign_callback_bridges,
        globals_arena: module_ctx.globals_arena,
        globals: module_ctx.globals,
        arrays: module_ctx.arrays,
        array_tds: module_ctx.array_tds,
        type_tds: module_ctx.type_tds,
        external_type_tds: module_ctx.external_type_tds,
        root_scans,
        target_data: module_ctx.target_data,
        return_slot,
        param_offset,
        allocas: Vec::with_capacity(function.locals.len()),
        temps: HashMap::new(),
        root_storage: HashMap::new(),
        unwind_root_sources,
        compiler_unwind_blocks,
        native_call_index: 0,
        compiler_invoke_index: 0,
        allocation_index: 0,
        bounds_trap_block: None,
        bounds_message: module_ctx.bounds_message,
    };

    // All locals are stack slots allocated at the top of the entry block;
    // LLVM's mem2reg promotes them. Entry is empty at this point, so
    // positioning at its end places the allocas before every instruction.
    builder.position_at_end(blocks[arena_index(function.entry)]);
    for (_, local) in function.locals.iter() {
        let ty = basic_ty(
            context,
            module_ctx.structs,
            module_ctx.enums,
            module_ctx.managed_address_space,
            &local.ty,
        )?;
        emitter.allocas.push(
            builder
                .build_alloca(ty, &local.name)
                .map_err(|e| CodegenError(format!("alloca %{}: {e}", local.name)))?,
        );
    }
    emitter.prepare_root_storage()?;
    for (block_id, block) in function.blocks.iter() {
        emitter.current_block = block_id;
        builder.position_at_end(blocks[arena_index(block_id)]);
        // Invoke is an LLVM terminator even though it
        // are LIR instructions: one must be the last instruction of its
        // block, and the block's LIR terminator must be the redundant
        // `Br` to the invoke's normal target (kept so dumps stay
        // readable); it is not emitted. A LandingPad instruction must be
        // the first of its block (LLVM requires the landingpad first).
        let mut invoke_terminated = false;
        for (index, instruction) in block.instructions.iter().enumerate() {
            let is_invoke = matches!(instruction, Instruction::Invoke { .. });
            if is_invoke && index + 1 != block.instructions.len() {
                return Err(CodegenError(format!(
                    "invoke @{}: must be the last instruction of block {}",
                    function.symbol, block.name
                )));
            }
            if matches!(
                instruction,
                Instruction::LandingPad { .. } | Instruction::CleanupPad { .. }
            ) && index != 0
            {
                return Err(CodegenError(format!(
                    "landing pad @{}: must be the first instruction of block {}",
                    function.symbol, block.name
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
                        function.symbol, block.name
                    )));
                }
            }
            continue;
        }
        match &block.terminator {
            Terminator::Br(target) => {
                builder
                    .build_unconditional_branch(blocks[arena_index(*target)])
                    .map_err(|e| CodegenError(format!("br @{}: {e}", block.name)))?;
            }
            Terminator::CondBr {
                cond,
                then_block,
                else_block,
            } => {
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
                let value = value
                    .map(|value| emitter.value(value))
                    .transpose()
                    .map_err(|e: CodegenError| {
                        CodegenError(format!("ret @{}: {}", function.symbol, e.0))
                    })?;
                if let Some(slot) = emitter.return_slot {
                    let value = value.ok_or_else(|| {
                        CodegenError(format!(
                            "ret @{}: aggregate return has no value",
                            function.symbol
                        ))
                    })?;
                    builder
                        .build_store(slot, value)
                        .map_err(|e| CodegenError(format!("ret slot @{}: {e}", function.symbol)))?;
                    builder
                        .build_return(None)
                        .map_err(|e| CodegenError(format!("ret @{}: {e}", function.symbol)))?;
                } else {
                    builder
                        .build_return(
                            value
                                .as_ref()
                                .map(|v| v as &dyn inkwell::values::BasicValue),
                        )
                        .map_err(|e| CodegenError(format!("ret @{}: {e}", function.symbol)))?;
                }
            }
            Terminator::Resume { exception } => {
                let exception = emitter.value(*exception)?;
                builder
                    .build_resume(exception)
                    .map_err(|e| CodegenError(format!("resume @{}: {e}", function.symbol)))?;
            }
            Terminator::Unreachable => {
                builder
                    .build_unreachable()
                    .map_err(|e| CodegenError(format!("unreachable @{}: {e}", function.symbol)))?;
            }
        }
    }
    Ok(())
}
