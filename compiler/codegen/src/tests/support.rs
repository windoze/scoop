use super::*;
use scoop_lir::LirIntegerConstant;
use std::collections::{BTreeMap, HashSet};

mod calls;

pub(super) use calls::*;

pub(super) const fn signed64(raw_bits: u64) -> Value {
    Value::IntegerConst(LirIntegerConstant::Signed64(raw_bits))
}

pub(super) fn callable_body(symbol: &str) -> scoop_lir::CallableBodyIdentity {
    let identifier = format!(
        "test{}",
        symbol
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let declaration = SourceDeclarationKey::function(
        site,
        CanonicalIdentifier::new(&identifier).unwrap(),
        0,
        None,
        Vec::new(),
    );
    let function = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
    scoop_lir::CallableBodyIdentity::for_function(function).unwrap()
}

pub(super) fn callable_body_at(source: &str, line: u32) -> scoop_lir::CallableBodyIdentity {
    callable_body(&format!("{source}:{line}"))
}

pub(super) fn runtime_type(name: &str) -> scoop_lir::RuntimeTypeMappingRecord {
    scoop_lir::RuntimeTypeMappingRecord::new(test_exact_type(name)).unwrap()
}

pub(super) fn layout_identity(
    name: &str,
    role: scoop_identity::RepresentationRole,
) -> scoop_lir::LayoutIdentity {
    let exact_type = test_exact_type(name);
    let target = scoop_lir::LirTargetProfile::DARWIN_AARCH64;
    match role {
        scoop_identity::RepresentationRole::ManagedValue => {
            scoop_lir::LayoutIdentity::managed_value(exact_type, target)
        }
        scoop_identity::RepresentationRole::ManagedObject => {
            scoop_lir::LayoutIdentity::managed_object(exact_type, target)
        }
        scoop_identity::RepresentationRole::CValue => {
            scoop_lir::LayoutIdentity::c_value(exact_type, target)
        }
        scoop_identity::RepresentationRole::NativeFunctionPointer => {
            scoop_lir::LayoutIdentity::native_function_pointer(exact_type, target)
        }
    }
    .unwrap()
}

pub(super) fn array_layout_identity(name: &str) -> scoop_lir::LayoutIdentity {
    scoop_lir::LayoutIdentity::managed_array(
        test_exact_type(name),
        scoop_lir::LirTargetProfile::DARWIN_AARCH64,
    )
    .unwrap()
}

pub(super) fn vtable(owner: &str, slots: Vec<scoop_lir::DispatchEntry>) -> scoop_lir::VtableRecord {
    scoop_lir::VtableRecord::new(test_exact_type(owner), slots).unwrap()
}

pub(super) fn itable(
    owner: &str,
    interface: &str,
    interface_descriptor: scoop_lir::TypeDescriptorRef,
    slots: Vec<scoop_lir::DispatchEntry>,
) -> scoop_lir::ItableRecord {
    scoop_lir::ItableRecord::new(
        test_exact_type(owner),
        test_exact_type(interface),
        interface_descriptor,
        slots,
    )
    .unwrap()
}

pub(super) fn static_storage_identity(name: &str) -> scoop_lir::StaticStorageIdentity {
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let declaration = SourceDeclarationKey::property(
        site,
        CanonicalIdentifier::new(&format!("test{name}")).unwrap(),
    );
    let property = PersistentPropertyId::from_source_declaration(&declaration).unwrap();
    scoop_lir::StaticStorageIdentity::property_backing(scoop_identity::PropertyOwner::Property(
        property,
    ))
    .unwrap()
}

pub(super) fn immortal_string_identity(name: &str) -> scoop_lir::ImmortalObjectIdentity {
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let declaration = SourceDeclarationKey::function(
        site,
        CanonicalIdentifier::new(&format!("string{name}")).unwrap(),
        0,
        None,
        Vec::new(),
    );
    let function = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
    let owner = scoop_identity::ImmortalObjectOwner::Callable(
        scoop_identity::CallableMaterialization::new(
            scoop_identity::CallableTemplateOwner::Function(function),
            scoop_identity::CallableMaterializationContext::NoSubstitution,
        ),
    );
    let path = StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::StringConstant, 0),
        [],
    );
    scoop_lir::ImmortalObjectIdentity::from_key(scoop_identity::ImmortalObjectKey::string_constant(
        owner, path,
    ))
    .unwrap()
}

fn test_exact_type(name: &str) -> PersistentExactTypeId {
    let identifier = format!(
        "test{}",
        name.as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let declaration = SourceDeclarationKey::nominal(
        site,
        CanonicalIdentifier::new(&identifier).unwrap(),
        SourceNominalKind::Class,
        0,
    );
    let nominal = PersistentTypeId::from_source_declaration(&declaration).unwrap();
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap()
}

pub(super) fn test_safepoints(
    owner_symbol: &str,
    blocks: &Arena<BasicBlock>,
    entry: scoop_lir::BlockId,
) -> scoop_lir::SafepointIdentities {
    test_safepoints_for_owner(callable_body(owner_symbol).id(), blocks, entry)
}

fn test_safepoints_for_owner(
    owner: scoop_identity::PersistentCallableBodyId,
    blocks: &Arena<BasicBlock>,
    entry: scoop_lir::BlockId,
) -> scoop_lir::SafepointIdentities {
    fn successors(block: &BasicBlock) -> Vec<scoop_lir::BlockId> {
        if let Some(Instruction::Invoke { site }) = block.instructions.last() {
            assert!(
                matches!(block.terminator, Terminator::Br(target) if target == site.normal()),
                "test invoke block must branch to its normal successor"
            );
            return vec![site.normal(), site.unwind()];
        }
        match block.terminator {
            Terminator::Br(target) => vec![target],
            Terminator::CondBr {
                then_block,
                else_block,
                ..
            } => vec![then_block, else_block],
            Terminator::Return { .. } | Terminator::Resume { .. } | Terminator::Unreachable => {
                Vec::new()
            }
        }
    }

    let mut visited = HashSet::new();
    let mut postorder = Vec::with_capacity(blocks.len());
    let mut stack = vec![(entry, false)];
    while let Some((block, expanded)) = stack.pop() {
        if expanded {
            postorder.push(block);
            continue;
        }
        if !visited.insert(block) {
            continue;
        }
        stack.push((block, true));
        stack.extend(
            successors(&blocks[block])
                .into_iter()
                .rev()
                .map(|successor| (successor, false)),
        );
    }
    assert_eq!(
        visited.len(),
        blocks.len(),
        "test CFG must be fully reachable"
    );
    postorder.reverse();

    let mut ordinals = BTreeMap::<scoop_lir::SafepointSiteRole, u32>::new();
    let mut references = HashSet::new();
    let mut identities = Vec::new();
    for block in postorder {
        for instruction in &blocks[block].instructions {
            let Some((role, reference)) = instruction.safepoint() else {
                continue;
            };
            assert!(
                references.insert(reference),
                "test fixture reuses a safepoint reference"
            );
            let ordinal = ordinals.entry(role).or_default();
            identities.push((
                reference,
                scoop_lir::SafepointIdentity::new(owner, role, *ordinal)
                    .expect("test safepoint identity"),
            ));
            *ordinal = ordinal.checked_add(1).expect("test safepoint ordinal");
        }
    }
    scoop_lir::SafepointIdentities::checked(identities).expect("test safepoint relation")
}

pub(super) fn refresh_test_safepoints(function: &mut Function) {
    function.safepoints = test_safepoints_for_owner(
        function.callable_body.id(),
        &function.blocks,
        function.entry,
    );
}

pub(super) fn refresh_module_safepoints(module: &mut Module) {
    for function in &mut module.functions {
        refresh_test_safepoints(function);
    }
}

pub(super) fn host_profile() -> TargetProfile {
    TargetProfile::resolve_host().expect("supported host target")
}

pub(super) fn host_managed_address_space() -> ManagedAddressSpace {
    host_profile().managed_address_space_contract()
}

fn abi_sequence(parts: impl IntoIterator<Item = RefScan>) -> RefScan {
    fn collect(scan: RefScan, references: &mut Vec<u64>) {
        match scan {
            RefScan::None => {}
            RefScan::References(offsets) => references.extend(offsets),
            RefScan::Sequence(parts) => {
                for part in parts {
                    collect(part, references);
                }
            }
        }
    }

    let mut references = Vec::new();
    for part in parts {
        collect(part, &mut references);
    }
    if references.is_empty() {
        RefScan::None
    } else {
        RefScan::References(references)
    }
}

fn shifted_scan(scan: &RefScan, base: u64) -> RefScan {
    match scan {
        RefScan::None => RefScan::None,
        RefScan::References(offsets) => {
            RefScan::References(offsets.iter().map(|offset| base + offset).collect())
        }
        RefScan::Sequence(parts) => abi_sequence(parts.iter().map(|part| shifted_scan(part, base))),
    }
}

fn abi_layout(
    structs: &scoop_lir::StructDefs,
    enums: &scoop_lir::EnumDefs,
    ty: &LirType,
) -> (u64, u64) {
    let scalar = |kind| {
        let layout = scoop_lir::LirTargetProfile::DARWIN_AARCH64.scalar_layout(kind);
        (layout.size_bytes(), layout.alignment_bytes())
    };
    match ty {
        LirType::Void => (0, 1),
        LirType::I1 => scalar(scoop_lir::BackendScalarKind::I1),
        LirType::I8 => scalar(scoop_lir::BackendScalarKind::I8),
        LirType::I16 => scalar(scoop_lir::BackendScalarKind::I16),
        LirType::I32 => scalar(scoop_lir::BackendScalarKind::I32),
        LirType::I64 | LirType::MachineScalar(_) => scalar(scoop_lir::BackendScalarKind::I64),
        LirType::Ptr(kind) => {
            let layout = scoop_lir::LirTargetProfile::DARWIN_AARCH64.pointer_layout(*kind);
            (layout.size_bytes(), layout.alignment_bytes())
        }
        LirType::ExceptionRecord => (16, 8),
        LirType::Aggregate(fields) => {
            let mut size = 0u64;
            let mut align = 1u64;
            for field in fields {
                let (field_size, field_align) = abi_layout(structs, enums, field);
                size = size.next_multiple_of(field_align) + field_size;
                align = align.max(field_align);
            }
            (size.next_multiple_of(align), align)
        }
        LirType::Struct(id) => (structs[*id].size, structs[*id].align),
        LirType::Enum(id) => match &enums[*id].repr {
            EnumRepr::Niche { kind, .. } => {
                let layout =
                    scoop_lir::LirTargetProfile::DARWIN_AARCH64.pointer_layout(kind.pointer_kind());
                (layout.size_bytes(), layout.alignment_bytes())
            }
            EnumRepr::Tagged { size, align, .. } => (*size, *align),
        },
    }
}

fn abi_scan(
    structs: &scoop_lir::StructDefs,
    enums: &scoop_lir::EnumDefs,
    ty: &LirType,
    base: u64,
) -> RefScan {
    match ty {
        LirType::Ptr(PointerKind::Managed) => RefScan::References(vec![base]),
        LirType::Aggregate(fields) => {
            let mut offset = 0u64;
            abi_sequence(fields.iter().map(|field| {
                let (field_size, field_align) = abi_layout(structs, enums, field);
                offset = offset.next_multiple_of(field_align);
                let scan = abi_scan(structs, enums, field, base + offset);
                offset += field_size;
                scan
            }))
        }
        LirType::Struct(id) => {
            let definition = &structs[*id];
            abi_sequence((0..definition.field_count()).map(|index| {
                let field = definition
                    .field_storage_type(index)
                    .expect("test struct field index is in range");
                let offset = definition
                    .field_layout(index)
                    .expect("test struct field index is in range")
                    .offset;
                abi_scan(structs, enums, &field, base + offset)
            }))
        }
        LirType::Enum(id) => shifted_scan(&enums[*id].scan, base),
        LirType::Void
        | LirType::I1
        | LirType::I8
        | LirType::I16
        | LirType::I32
        | LirType::I64
        | LirType::MachineScalar(_)
        | LirType::Ptr(_)
        | LirType::ExceptionRecord => RefScan::None,
    }
}

pub(super) fn abi_value_with_layout(
    ty: LirType,
    size: u64,
    align: u64,
    scan: RefScan,
) -> scoop_lir::AbiValue {
    scoop_lir::AbiValue::new(
        ty,
        scoop_lir::AbiNonZeroLayout::new(size, align)
            .expect("test ABI value has a non-zero valid layout"),
        scan,
    )
    .expect("test ABI value has a storable type")
}

fn abi_argument(
    structs: &scoop_lir::StructDefs,
    enums: &scoop_lir::EnumDefs,
    ty: LirType,
) -> scoop_lir::AbiArgument {
    let (size, align) = abi_layout(structs, enums, &ty);
    if size == 0 {
        return scoop_lir::AbiArgument::ElidedZst(
            scoop_lir::AbiZst::new(
                ty,
                scoop_lir::AbiZeroSizedLayout::new(align)
                    .expect("test ABI ZST has a valid alignment"),
            )
            .expect("test ABI ZST has a storable type"),
        );
    }
    let scan = abi_scan(structs, enums, &ty, 0);
    let value = abi_value_with_layout(ty, size, align, scan);
    match scoop_lir::classify_non_zero_scoop_abi_value(
        scoop_lir::LirTargetProfile::DARWIN_AARCH64,
        enums,
        value.storage_type(),
    )
    .expect("test ABI helper only classifies valid non-void value types")
    {
        scoop_lir::ScoopAbiPassing::Direct => scoop_lir::AbiArgument::Direct(value),
        scoop_lir::ScoopAbiPassing::Indirect => scoop_lir::AbiArgument::Indirect(value),
    }
}

pub(super) fn scoop_signature(
    structs: &scoop_lir::StructDefs,
    enums: &scoop_lir::EnumDefs,
    params: Vec<LirType>,
    return_ty: LirType,
) -> scoop_lir::ScoopAbiSignature {
    let arguments = params
        .into_iter()
        .map(|ty| abi_argument(structs, enums, ty))
        .collect();
    let result = if return_ty == LirType::Void {
        scoop_lir::AbiReturn::UnitVoid
    } else {
        match abi_argument(structs, enums, return_ty) {
            scoop_lir::AbiArgument::ElidedZst(value) => scoop_lir::AbiReturn::ElidedZst(value),
            scoop_lir::AbiArgument::Direct(value) => scoop_lir::AbiReturn::Direct(value),
            scoop_lir::AbiArgument::Indirect(value) => scoop_lir::AbiReturn::Indirect(value),
        }
    };
    scoop_lir::ScoopAbiSignature::new(arguments, result, scoop_lir::CallingConvention::Cdecl)
}

pub(super) fn plain_scoop_signature(
    params: Vec<LirType>,
    return_ty: LirType,
) -> scoop_lir::ScoopAbiSignature {
    scoop_signature(
        &scoop_lir::StructDefs::default(),
        &scoop_lir::EnumDefs::default(),
        params,
        return_ty,
    )
}

pub(super) fn string_metadata() -> LirMeta {
    let mut layouts = Arena::new();
    let string_layout = layouts.alloc(Layout {
        identity: layout_identity("String", scoop_identity::RepresentationRole::ManagedObject),
        name: "String".to_string(),
        size: 24,
        align: 8,
        fields: Vec::new(),
        c_layout: None,
        interior_mutable: false,
        kind: LayoutKind::Intrinsic(scoop_lir::IntrinsicTypeRepresentation::String),
    });
    let mut type_descriptors = Arena::new();
    let string_descriptor = type_descriptors.alloc(TypeDescriptor {
        name: "String".to_string(),
        symbol: scoop_lir::STRING_TD_SYMBOL.to_string(),
        runtime_type: runtime_type("String"),
        size: 24,
        align: 8,
        scan: TypeDescriptorScan::Fixed(RefScan::None),
        parent: None,
        vtable: vtable("String", Vec::new()),
        itables: Vec::new(),
    });
    LirMeta {
        target_profile: scoop_lir::LirTargetProfile::DARWIN_AARCH64,
        well_known_layouts: WellKnownLayouts {
            string: string_layout,
        },
        well_known_type_descriptors: WellKnownTypeDescriptors {
            string: TypeDescriptorRef::Local(string_descriptor),
        },
        arrays: Arena::new(),
        layouts,
        type_descriptors,
        external_type_descriptors: Arena::new(),
        external_callables: Arena::new(),
    }
}

pub(super) fn array_type(
    meta: &mut LirMeta,
    name: &str,
    kind: scoop_lir::ArrayKind,
    element: LirType,
    element_size: u64,
    element_align: u64,
    scan: RefScan,
) -> ArrayTypeId {
    let type_descriptor = meta.type_descriptors.alloc(TypeDescriptor {
        name: name.to_string(),
        symbol: format!("scoop_td_{name}"),
        runtime_type: runtime_type(name),
        size: element_size,
        align: element_align,
        scan: TypeDescriptorScan::ArrayElement {
            stride: element_size,
            scan,
        },
        parent: None,
        vtable: vtable(name, Vec::new()),
        itables: Vec::new(),
    });
    meta.arrays.alloc(ArrayType {
        identity: array_layout_identity(name),
        kind,
        element,
        element_size,
        element_align,
        type_descriptor: TypeDescriptorRef::Local(type_descriptor),
    })
}

/// An M2-shaped module: string constants, a user function exercising
/// alloca/store/load, arithmetic, branches, aggregates and
/// extractvalue, plus calls into the new runtime functions.
pub(super) fn values_module() -> Module {
    let mut globals = Arena::default();
    let hello = globals.alloc(Global {
        symbol: "scoop.string.0".to_string(),
        address_kind: PointerKind::Managed,
        scan: RefScan::None,
        init: GlobalInit::StringConst {
            identity: immortal_string_identity("hello"),
            value: "hello, ".to_string(),
        },
    });
    let world = globals.alloc(Global {
        symbol: "scoop.string.1".to_string(),
        address_kind: PointerKind::Managed,
        scan: RefScan::None,
        init: GlobalInit::StringConst {
            identity: immortal_string_identity("world"),
            value: "world".to_string(),
        },
    });

    let mut locals = Arena::default();
    let n = locals.alloc(Local {
        name: "n".to_string(),
        ty: LirType::I64,
    });
    let point = locals.alloc(Local {
        name: "p".to_string(),
        ty: LirType::Aggregate(vec![LirType::I64, LirType::I64]),
    });
    let unit = locals.alloc(Local {
        name: "u".to_string(),
        ty: LirType::Aggregate(vec![]),
    });

    let mut temps = Arena::default();
    let temp = |temps: &mut Arena<Temp>, ty: LirType| temps.alloc(Temp { ty });
    // t0 = 1 + 2
    let t0 = temp(&mut temps, LirType::I64);
    // t1 = {40, 2} (Point)
    let t1 = temp(
        &mut temps,
        LirType::Aggregate(vec![LirType::I64, LirType::I64]),
    );
    // t2 = p.x
    let t2 = temp(&mut temps, LirType::I64);
    // t3 = t2 < 100
    let t3 = temp(&mut temps, LirType::I1);
    // t4 = -t2
    let t4 = temp(&mut temps, LirType::I64);
    // t5 = !true
    let t5 = temp(&mut temps, LirType::I1);
    // t6 = concat(hello, world)
    let t6 = temp(&mut temps, MANAGED_PTR);
    // t7 = ()
    let t7 = temp(&mut temps, LirType::Aggregate(vec![]));
    let mut call_targets = CallTargets::default();
    let concat = direct_site(
        &mut call_targets,
        TestCallProtocol::Managed {
            safepoint: 1,
            destination: managed_runtime(scoop_lir::ManagedRuntimeFunction::StringConcat),
        },
        vec![MANAGED_PTR, MANAGED_PTR],
        (MANAGED_PTR, RefScan::References(vec![0])),
        t6,
        vec![Value::Global(hello), Value::Global(world)],
    );

    // Allocate the four blocks first so terminators can reference
    // them, then fill in their bodies.
    let mut blocks = Arena::default();
    let placeholder = |blocks: &mut Arena<BasicBlock>, name: &str| {
        blocks.alloc(BasicBlock {
            name: name.to_string(),
            instructions: vec![],
            terminator: Terminator::Return { value: None },
        })
    };
    let entry = placeholder(&mut blocks, "entry");
    let then_block = placeholder(&mut blocks, "then");
    let else_block = placeholder(&mut blocks, "else");
    let end = placeholder(&mut blocks, "end");

    blocks[entry] = BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::IntegerBinary {
                out: t0,
                kind: IntegerKind::SIGNED_64,
                operation: IntegerBinaryOperation::Add,
                lhs: signed64(1),
                rhs: signed64(2),
            },
            Instruction::Store {
                local: n,
                value: Value::Temp(t0),
            },
            Instruction::MakeAggregate {
                out: t1,
                elements: vec![signed64(40), signed64(2)],
            },
            Instruction::Store {
                local: point,
                value: Value::Temp(t1),
            },
            Instruction::ExtractValue {
                out: t2,
                aggregate: Value::Local(point),
                index: 0,
            },
            Instruction::IntegerCompare {
                out: t3,
                kind: IntegerKind::SIGNED_64,
                comparison: IntegerComparison::Less,
                lhs: Value::Temp(t2),
                rhs: signed64(100),
            },
            Instruction::Call { site: concat },
        ],
        terminator: Terminator::CondBr {
            cond: Value::Temp(t3),
            then_block,
            else_block,
        },
    };
    blocks[then_block] = BasicBlock {
        name: "then".to_string(),
        instructions: vec![Instruction::IntegerUnary {
            out: t4,
            kind: IntegerKind::SIGNED_64,
            operation: IntegerUnaryOperation::Negate,
            operand: Value::Temp(t2),
        }],
        terminator: Terminator::Br(end),
    };
    blocks[else_block] = BasicBlock {
        name: "else".to_string(),
        instructions: vec![Instruction::UnaryOp {
            out: t5,
            op: UnOp::Not,
            operand: Value::BoolConst(true),
        }],
        terminator: Terminator::Br(end),
    };
    blocks[end] = BasicBlock {
        name: "end".to_string(),
        instructions: vec![
            Instruction::MakeAggregate {
                out: t7,
                elements: vec![],
            },
            Instruction::Store {
                local: unit,
                value: Value::Temp(t7),
            },
        ],
        terminator: Terminator::Return { value: None },
    };

    Module {
        globals,
        initialization_units: Arena::default(),
        structs: scoop_lir::StructDefs::default(),
        enums: scoop_lir::EnumDefs::default(),
        extern_functions: Default::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_families: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![Function {
            callable_body: callable_body("scoop_main"),
            safepoints: test_safepoints("scoop_main", &blocks, entry),
            gc_effect: GcEffect::Managed,
            symbol: "scoop_main".to_string(),
            signature: plain_scoop_signature(vec![], LirType::Void),
            call_targets,
            locals,
            temps,
            blocks,
            entry,
        }],
        entry_symbol: "scoop_main".to_string(),
        meta: string_metadata(),
    }
}

/// The LLVM IR text of a module, verified, before the statepoint rewrite.
pub(super) fn ir_of(module: &Module) -> String {
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let llvm = emit_llvm_module(&context, module, &machine, host_profile()).expect("emit module");
    llvm.verify().expect("valid LLVM module");
    llvm.print_to_string().to_string()
}

pub(super) fn rewritten_ir_of(module: &Module) -> String {
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let llvm = emit_llvm_module(&context, module, &machine, host_profile()).expect("emit module");
    llvm.verify().expect("valid pre-statepoint module");
    let expected = statepoint::expectations(module).expect("complete safepoint manifest");
    statepoint::rewrite(&llvm, &machine).expect("rewrite statepoints");
    llvm.verify().expect("valid relocated module");
    statepoint::verify_rewritten(&llvm, &expected, host_profile())
        .expect("rewritten manifest agrees with LIR");
    llvm.print_to_string().to_string()
}
