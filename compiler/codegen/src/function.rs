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
    structs: &'a StructDefs,
    enums: &'a EnumDefs,
    extern_functions: &'a ExternFunctions,
    native_globals: &'a Arena<NativeGlobal>,
    native_global_bridges: &'a scoop_lir::NativeGlobalBridges,
    callback_bridges: &'a Arena<scoop_lir::CallbackBridge>,
    foreign_callback_families: &'a Arena<scoop_lir::ForeignCallbackFamily>,
    foreign_callback_bridges: &'a Arena<scoop_lir::ForeignCallbackBridge>,
    globals_arena: &'a Arena<Global>,
    globals: &'a [Option<GlobalValue<'ctx>>],
    initialization_units: &'a [GlobalValue<'ctx>],
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
    /// Cone-image-owned "array index out of bounds" support atom it references.
    bounds_trap_block: Option<inkwell::basic_block::BasicBlock<'ctx>>,
    bounds_message: GlobalValue<'ctx>,
    array_size_trap_block: Option<inkwell::basic_block::BasicBlock<'ctx>>,
    array_size_message: GlobalValue<'ctx>,
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
            Self::NoGc => None,
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
        value: &'a scoop_lir::AbiValue,
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
        TypedCallResult::Direct { value, .. } | TypedCallResult::Indirect { value, .. } => {
            value.scan()
        }
    }
}

pub(super) fn emit_function<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    builder: &inkwell::builder::Builder<'ctx>,
    module_ctx: &ModuleCtx<'_, 'ctx>,
    function: &Function,
) -> Result<(), CodegenError> {
    // Pre-declared in the first pass of the selected object member.
    let llvm_function = llvm
        .get_function(function.symbol())
        .expect("function declared in the first pass");

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
        let personality_symbol = scoop_lir::TargetEhSupportV1::ScoopPersonality.logical_symbol();
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
    let root_scans = function
        .call_targets
        .root_scans
        .iter()
        .map(|(id, scan)| {
            emit_ref_scan(
                context,
                llvm,
                &format!("{}.root_scan.{}", function.symbol(), id.into_raw()),
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
        callback_bridges: module_ctx.callback_bridges,
        foreign_callback_families: module_ctx.foreign_callback_families,
        foreign_callback_bridges: module_ctx.foreign_callback_bridges,
        globals_arena: module_ctx.globals_arena,
        globals: module_ctx.globals,
        initialization_units: module_ctx.initialization_units,
        arrays: module_ctx.arrays,
        array_tds: module_ctx.array_tds,
        type_tds: module_ctx.type_tds,
        external_type_tds: module_ctx.external_type_tds,
        root_scans,
        target_data: module_ctx.target_data,
        return_slot,
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
        array_size_trap_block: None,
        array_size_message: module_ctx.array_size_message,
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
        let is_zero_sized = module_ctx.target_data.get_store_size(&ty) == 0;
        let allocation_type = if is_zero_sized {
            context.i8_type().into()
        } else {
            ty
        };
        let storage = builder
            .build_alloca(allocation_type, &local.name)
            .map_err(|e| CodegenError(format!("alloca %{}: {e}", local.name)))?;
        if is_zero_sized {
            storage
                .as_instruction_value()
                .expect("alloca is an instruction")
                .set_alignment(module_ctx.target_data.get_abi_alignment(&ty))
                .map_err(|e| {
                    CodegenError(format!("align zero-sized place %{}: {e}", local.name))
                })?;
        }
        emitter.allocas.push(storage);
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
                    scoop_lir::AbiReturn::Direct(_) => {
                        let value = emitter.value(value.expect("direct result was validated"))?;
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
    Ok(())
}
