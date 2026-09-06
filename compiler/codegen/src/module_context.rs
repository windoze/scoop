use super::*;

/// Module-level data function emission needs, bundled to keep
/// signatures small.
pub(crate) struct ModuleCtx<'a, 'ctx> {
    pub(crate) managed_address_space: ManagedAddressSpace,
    pub(crate) functions: &'a [Function],
    pub(crate) structs: &'a StructDefs,
    pub(crate) enums: &'a EnumDefs,
    pub(crate) extern_functions: &'a ExternFunctions,
    pub(crate) native_globals: &'a Arena<NativeGlobal>,
    pub(crate) native_global_bridges: &'a scoop_lir::NativeGlobalBridges,
    pub(crate) foreign_callback_families: &'a Arena<scoop_lir::ForeignCallbackFamily>,
    pub(crate) foreign_callback_bridges: &'a Arena<scoop_lir::ForeignCallbackBridge>,
    pub(crate) globals_arena: &'a Arena<Global>,
    pub(crate) globals: &'a [Option<GlobalValue<'ctx>>],
    pub(crate) initialization_units: &'a [GlobalValue<'ctx>],
    pub(crate) arrays: &'a Arena<ArrayType>,
    pub(crate) array_tds: &'a [GlobalValue<'ctx>],
    pub(crate) type_tds: &'a [GlobalValue<'ctx>],
    pub(crate) external_type_tds: &'a [GlobalValue<'ctx>],
    pub(crate) target_data: &'a inkwell::targets::TargetData,
    pub(crate) bounds_message: Option<GlobalValue<'ctx>>,
    pub(crate) array_size_message: Option<GlobalValue<'ctx>>,
}

fn compiler_root_source_key(source: scoop_lir::CallerRootSource) -> (u8, u32) {
    match source {
        scoop_lir::CallerRootSource::Param(index) => (0, index),
        scoop_lir::CallerRootSource::Local(id) => (1, id.into_raw().into_u32()),
        scoop_lir::CallerRootSource::Temp(id) => (2, id.into_raw().into_u32()),
    }
}

pub(crate) fn root_storage_sources(function: &Function) -> Vec<scoop_lir::CallerRootSource> {
    let mut sources = HashSet::new();
    for (_, block) in function.blocks.iter() {
        for instruction in &block.instructions {
            match instruction {
                Instruction::ManagedPoll { site } => {
                    sources.extend(site.live.as_slice().iter().map(|value| value.source));
                }
                Instruction::ArrayAlloc { live, .. }
                | Instruction::ArrayAssembly { live, .. }
                | Instruction::ArrayClone { live, .. } => {
                    sources.extend(live.as_slice().iter().map(|value| value.source));
                }
                Instruction::Call { site } => match site {
                    scoop_lir::CallSite::Managed(site) => {
                        sources.extend(site.live.as_slice().iter().map(|value| value.source));
                    }
                    scoop_lir::CallSite::NativeSafe(site) => {
                        sources.extend(site.roots.as_slice().iter().map(|root| root.source));
                    }
                    scoop_lir::CallSite::NativeBorrowed(site) => {
                        sources.extend(site.roots.as_slice().iter().map(|root| root.source));
                    }
                    scoop_lir::CallSite::NoGc(_) => {}
                },
                Instruction::Invoke {
                    site: scoop_lir::InvokeSite::Managed(site),
                } => {
                    sources.extend(site.roots.as_slice().iter().map(|root| root.root.source));
                }
                Instruction::Invoke {
                    site: scoop_lir::InvokeSite::NoGc(_),
                } => {}
                Instruction::NativeGlobalLoad { roots, .. }
                | Instruction::NativeGlobalStore { roots, .. }
                | Instruction::NativeGlobalAddress { roots, .. } => {
                    sources.extend(roots.as_slice().iter().map(|root| root.source));
                }
                _ => {}
            }
        }
    }
    let mut sources = sources.into_iter().collect::<Vec<_>>();
    sources.sort_by_key(|source| compiler_root_source_key(*source));
    sources
}

pub(crate) fn instruction_temp_defs(instruction: &Instruction) -> [Option<TempId>; 2] {
    let first = match instruction {
        Instruction::BinOp { out, .. }
        | Instruction::UnaryOp { out, .. }
        | Instruction::IntegerUnary { out, .. }
        | Instruction::IntegerBinary { out, .. }
        | Instruction::SafeIntegerDivRem { out, .. }
        | Instruction::IntegerCompare { out, .. }
        | Instruction::IntegerCompareTo { out, .. }
        | Instruction::IntegerShift { out, .. }
        | Instruction::IntegerConvert { out, .. }
        | Instruction::MakeAggregate { out, .. }
        | Instruction::ExtractValue { out, .. }
        | Instruction::HeapLoad { out, .. }
        | Instruction::MachineHeapLoad { out, .. }
        | Instruction::AtomicLoad { out, .. }
        | Instruction::AtomicCompareExchange { out, .. }
        | Instruction::GlobalLoad { out, .. }
        | Instruction::GlobalAddress { out, .. }
        | Instruction::NativeGlobalLoad { out, .. }
        | Instruction::NativeGlobalAddress { out, .. }
        | Instruction::FunctionAddress { out, .. }
        | Instruction::ForeignCallbackRegister { out, .. }
        | Instruction::ULongToPtr { out, .. }
        | Instruction::PtrToULong { out, .. }
        | Instruction::RawLoad { out, .. }
        | Instruction::PtrOffset { out, .. }
        | Instruction::LocalAddress { out, .. }
        | Instruction::BeginCatch { out, .. }
        | Instruction::ArrayAlloc { out, .. }
        | Instruction::ArrayAssembly { out, .. }
        | Instruction::ArrayLen { out, .. }
        | Instruction::ArrayGet { out, .. }
        | Instruction::ArrayClone { out, .. }
        | Instruction::EnumWrap { out, .. }
        | Instruction::EnumTag { out, .. }
        | Instruction::EnumField { out, .. }
        | Instruction::VariantTest { out, .. }
        | Instruction::VariantPayloadProject { out, .. } => Some(*out),
        Instruction::Call { site } => site.result_temp(),
        Instruction::Invoke { site } => site.result_temp(),
        Instruction::LandingPad { record, .. } | Instruction::CleanupPad { record, .. } => {
            Some(*record)
        }
        Instruction::ForeignCallbackOperation(operation) => operation.out(),
        Instruction::Store { .. }
        | Instruction::GlobalStore { .. }
        | Instruction::NativeGlobalStore { .. }
        | Instruction::HeapStore { .. }
        | Instruction::MachineHeapStore { .. }
        | Instruction::AtomicStore { .. }
        | Instruction::RawStore { .. }
        | Instruction::ManagedPoll { .. }
        | Instruction::EndCatch
        | Instruction::Throw { .. }
        | Instruction::ArraySet { .. } => None,
    };
    let second = match instruction {
        Instruction::LandingPad { raw, .. } | Instruction::CleanupPad { raw, .. } => Some(*raw),
        _ => None,
    };
    [first, second]
}

pub(crate) fn compiler_unwind_plan(
    function: &Function,
) -> (
    HashMap<scoop_lir::BlockId, Vec<scoop_lir::CallerRootSource>>,
    HashSet<scoop_lir::BlockId>,
) {
    let mut sources = HashMap::<_, HashSet<_>>::new();
    let mut blocks = HashSet::new();
    for (_, block) in function.blocks.iter() {
        for instruction in &block.instructions {
            let Instruction::Invoke { site } = instruction else {
                continue;
            };
            blocks.insert(site.unwind());
            if let scoop_lir::InvokeSite::Managed(site) = site {
                let entry = sources.entry(site.unwind).or_default();
                entry.extend(
                    site.roots
                        .as_slice()
                        .iter()
                        .filter(|root| root.unwind_live)
                        .map(|root| root.root.source),
                );
            }
        }
    }
    let sources = sources
        .into_iter()
        .map(|(block, set)| {
            let mut set = set.into_iter().collect::<Vec<_>>();
            set.sort_by_key(|source| compiler_root_source_key(*source));
            (block, set)
        })
        .collect();
    (sources, blocks)
}
