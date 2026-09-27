use crate::*;
use la_arena::Arena;
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    ExactTypeKey, PackagePath, PersistentTypeId, SourceDeclarationKey, SourceDeclarationSite,
    SourceNominalKind,
};

pub(super) fn value(ty: LirType, size: u64, scan: RefScan) -> AbiValue {
    AbiValue::new(ty, AbiNonZeroLayout::new(size, size.min(8)).unwrap(), scan).unwrap()
}

pub(super) fn module(functions: Vec<Function>) -> Module {
    let declaration = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("String").unwrap(),
        SourceNominalKind::Class,
        0,
    );
    let string = CborIdentityRecord::from_key(ExactTypeKey::Nominal(
        PersistentTypeId::from_source_declaration(&declaration).unwrap(),
    ))
    .unwrap();
    let mut external_type_descriptors = Arena::new();
    let string_descriptor = external_type_descriptors
        .alloc(ExternalTypeDescriptor::new(ConeIdentity::CORE, string.id()).unwrap());
    Module {
        cone: ConeIdentity::SINGLE_FILE,
        globals: Arena::new(),
        initialization_units: Arena::new(),
        structs: StructDefs::default(),
        enums: EnumDefs::default(),
        functions,
        extern_functions: ExternFunctions::default(),
        native_globals: Arena::new(),
        native_global_bridges: NativeGlobalBridges::default(),
        callback_bridges: Arena::new(),
        foreign_callback_families: Arena::new(),
        foreign_callback_bridges: Arena::new(),
        output: LirOutput::Library,
        meta: LirMeta {
            exact_types: Vec::new(),
            target_profile: LirTargetProfile::DARWIN_AARCH64,
            canonical_c_abi: CanonicalCAbiMetadata::default(),
            native_externals: NativeExternalMetadata::default(),
            well_known_type_descriptors: WellKnownTypeDescriptors {
                string: TypeDescriptorRef::External(string_descriptor),
            },
            arrays: Arena::new(),
            layouts: Arena::new(),
            type_descriptors: Arena::new(),
            external_type_descriptors,
            external_callables: Arena::new(),
        },
    }
}

pub(super) fn scalar(shuffled: bool) -> Module {
    let mut function = function(
        "canonicalArithmetic",
        vec![
            AbiArgument::Direct(value(LirType::I64, 8, RefScan::None)),
            AbiArgument::Direct(value(LirType::I64, 8, RefScan::None)),
            AbiArgument::Direct(value(LirType::I1, 1, RefScan::None)),
        ],
        AbiReturn::Direct(value(LirType::I64, 8, RefScan::None)),
    );
    let slots = locals(
        &mut function,
        shuffled,
        value(LirType::I64, 8, RefScan::None),
    );
    if shuffled {
        function.temps.alloc(Temp { ty: RAW_PTR });
    }
    let first = function.temps.alloc(Temp { ty: LirType::I64 });
    let second = function.temps.alloc(Temp { ty: LirType::I64 });
    let [sum, product] = if shuffled {
        [second, first]
    } else {
        [first, second]
    };
    let first = function.blocks.alloc(block("first"));
    let second = function.blocks.alloc(block("second"));
    let [yes, no] = if shuffled {
        [second, first]
    } else {
        [first, second]
    };
    function.blocks[yes].terminator = Terminator::Return {
        value: Some(Value::Temp(sum)),
    };
    function.blocks[no].terminator = Terminator::Return {
        value: Some(Value::Temp(product)),
    };
    let entry = &mut function.blocks[function.entry];
    entry.instructions = vec![
        Instruction::Store {
            local: slots[0],
            value: Value::Param(0),
        },
        Instruction::Store {
            local: slots[1],
            value: Value::Param(1),
        },
        Instruction::IntegerBinary {
            out: sum,
            kind: IntegerKind::SIGNED_64,
            operation: IntegerBinaryOperation::Add,
            lhs: Value::Local(slots[0]),
            rhs: Value::Local(slots[1]),
        },
        Instruction::IntegerBinary {
            out: product,
            kind: IntegerKind::SIGNED_64,
            operation: IntegerBinaryOperation::Multiply,
            lhs: Value::Temp(sum),
            rhs: Value::IntegerConst(LirIntegerConstant::Signed64(7)),
        },
    ];
    entry.terminator = Terminator::CondBr {
        cond: Value::Param(2),
        then_block: yes,
        else_block: no,
    };
    if shuffled {
        let old_entry = function.entry;
        let entry = std::mem::replace(&mut function.blocks[old_entry], block("unreachable"));
        function.entry = function.blocks.alloc(entry);
    }
    module(vec![function])
}

pub(super) fn function(name: &str, arguments: Vec<AbiArgument>, result: AbiReturn) -> Function {
    let mut blocks = Arena::new();
    let entry = blocks.alloc(block("entry"));
    Function {
        callable_body: crate::tests::callable_body(name),
        gc_effect: GcEffect::NoGc,
        signature: ScoopAbiSignature::new(arguments, result, CallingConvention::Cdecl),
        call_targets: CallTargets::default(),
        safepoints: SafepointIdentities::default(),
        locals: Arena::new(),
        temps: Arena::new(),
        blocks,
        entry,
    }
}

pub(super) fn block(name: &str) -> BasicBlock {
    BasicBlock {
        name: name.to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Unreachable,
    }
}

pub(super) fn locals(function: &mut Function, shuffled: bool, abi: AbiValue) -> [LocalId; 2] {
    if shuffled {
        function.locals.alloc(Local::new(
            "unused",
            LocalStorage::NonZero(value(RAW_PTR, 8, RefScan::None)),
        ));
    }
    let first = function
        .locals
        .alloc(Local::new("a", LocalStorage::NonZero(abi.clone())));
    let second = function
        .locals
        .alloc(Local::new("b", LocalStorage::NonZero(abi)));
    if shuffled {
        [second, first]
    } else {
        [first, second]
    }
}

pub(super) fn roots(shuffled: bool) -> Module {
    let pointer = value(MANAGED_PTR, 8, RefScan::References(vec![0]));
    let mut function = function(
        "canonicalRoots",
        vec![
            AbiArgument::Direct(pointer.clone()),
            AbiArgument::Direct(pointer.clone()),
        ],
        AbiReturn::Direct(value(LirType::I1, 1, RefScan::None)),
    );
    function.gc_effect = GcEffect::Managed;
    let slots = locals(&mut function, shuffled, pointer);
    let out = function.temps.alloc(Temp { ty: LirType::I1 });
    let targets = &mut function.call_targets;
    if shuffled {
        targets
            .void_signatures
            .alloc(VoidCallSignature::new(Vec::new(), CallingConvention::Cdecl));
    }
    let signature = targets
        .void_signatures
        .alloc(VoidCallSignature::new(Vec::new(), CallingConvention::Cdecl));
    if shuffled {
        targets.managed_targets.void.alloc(CallTarget {
            destination: ManagedCallDestination::runtime(ManagedRuntimeFunction::GcCollect),
            signature,
        });
    }
    let target = targets.managed_targets.void.alloc(CallTarget {
        destination: ManagedCallDestination::runtime(ManagedRuntimeFunction::Safepoint),
        signature,
    });
    let site = SafepointSiteRef::from_u32(if shuffled { 9 } else { 0 });
    function.safepoints = SafepointIdentities::checked(vec![(
        site,
        SafepointIdentity::new(
            function.callable_body.id(),
            SafepointSiteRole::ManagedPoll,
            0,
        )
        .unwrap(),
    )])
    .unwrap();
    let mut ordered = slots;
    ordered.sort();
    let live = StatepointLiveSet::new(
        ordered
            .into_iter()
            .map(|id| StatepointLiveValue {
                source: CallerRootSource::Local(id),
                ty: MANAGED_PTR,
                leaves: ManagedLeafPaths::new(vec![ManagedLeafPath { byte_offset: 0 }]).unwrap(),
            })
            .collect(),
    )
    .unwrap();
    function.blocks[function.entry].instructions = vec![
        Instruction::Store {
            local: slots[0],
            value: Value::Param(0),
        },
        Instruction::Store {
            local: slots[1],
            value: Value::Param(1),
        },
        Instruction::ManagedPoll {
            site: ManagedPollSite {
                target,
                safepoint: site,
                live,
            },
        },
        Instruction::BinOp {
            out,
            op: BinOp::Eq,
            lhs: Value::Local(slots[0]),
            rhs: Value::Local(slots[1]),
        },
    ];
    function.blocks[function.entry].terminator = Terminator::Return {
        value: Some(Value::Temp(out)),
    };
    module(vec![function])
}

pub(super) fn foundation(module: &Module) -> ConeLirFoundation {
    let mut canonical = CanonicalLirFoundation::empty();
    canonical
        .set_callable_bodies(
            module
                .functions
                .iter()
                .map(|function| function.callable_body.identity_record().clone())
                .collect(),
        )
        .unwrap();
    ConeLirFoundation::try_new(module.cone, canonical).unwrap()
}
