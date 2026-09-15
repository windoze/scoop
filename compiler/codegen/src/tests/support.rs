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

pub(super) fn outbound_bridge(seed: u8) -> scoop_lir::GeneratedBridgeEntryIdentity {
    scoop_lir::GeneratedBridgeEntryIdentity::new(
        ConeIdentity::SINGLE_FILE,
        scoop_identity::GeneratedBridgeUnitKey::OutboundFunction(native_contract(seed)),
    )
    .unwrap()
}

pub(super) fn global_read_bridge(seed: u8) -> scoop_lir::GeneratedBridgeEntryIdentity {
    scoop_lir::GeneratedBridgeEntryIdentity::new(
        ConeIdentity::SINGLE_FILE,
        scoop_identity::GeneratedBridgeUnitKey::GlobalRead(native_contract(seed)),
    )
    .unwrap()
}

pub(super) fn global_address_bridge(seed: u8) -> scoop_lir::GeneratedBridgeEntryIdentity {
    scoop_lir::GeneratedBridgeEntryIdentity::new(
        ConeIdentity::SINGLE_FILE,
        scoop_identity::GeneratedBridgeUnitKey::GlobalAddress(native_contract(seed)),
    )
    .unwrap()
}

pub(super) fn callback_trampoline(
    signature_seed: u8,
    context_index: u32,
) -> scoop_lir::ManagedCallbackTrampolineIdentity {
    scoop_lir::ManagedCallbackTrampolineIdentity::new(
        ConeIdentity::SINGLE_FILE,
        c_signature(signature_seed).fingerprint(),
        CallbackParameterIndex::new(context_index),
    )
    .unwrap()
}

pub(super) fn static_callback_trampoline(seed: u8) -> scoop_lir::StaticCallbackTrampolineIdentity {
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let declaration = SourceDeclarationKey::function(
        site,
        CanonicalIdentifier::new(&format!("staticCallback{seed}")).unwrap(),
        0,
        None,
        Vec::new(),
    );
    let source = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
    let exact_type = test_exact_type(&format!("staticCallbackType{seed}"));
    let storage_bridge = scoop_identity::StaticNoGcCallbackStorageBridgeId::from_key(
        &scoop_identity::GeneratedCallableKey::StaticNoGcCallbackStorageBridge {
            source: scoop_identity::CallableMaterialization::new(
                scoop_identity::CallableTemplateOwner::Function(source),
                scoop_identity::CallableMaterializationContext::NoSubstitution,
            ),
            signature: scoop_identity::ExactCallableSignature::new(
                scoop_identity::Effect::Ordinary,
                None,
                vec![exact_type],
                exact_type,
            ),
        },
    )
    .unwrap();
    scoop_lir::StaticCallbackTrampolineIdentity::new(
        ConeIdentity::SINGLE_FILE,
        storage_bridge,
        c_signature(seed).fingerprint(),
    )
    .unwrap()
}

fn native_contract(seed: u8) -> scoop_identity::NativeExternalContractFingerprint {
    let symbol = scoop_identity::NativeExternalSymbolKey::darwin_macho_external(
        &scoop_identity::SourceNativeSymbol::new(&format!("test_bridge_{seed}")).unwrap(),
    )
    .unwrap();
    let contract = scoop_identity::NativeExternalContract::c_function(
        scoop_identity::NativeLibraryBinding::DefaultNativeNamespace,
        c_signature(seed).signature().clone(),
    );
    scoop_identity::NativeExternalContractFingerprint::from_symbol_and_contract(
        scoop_identity::PersistentNativeExternalSymbolId::from_key(&symbol).unwrap(),
        &contract,
    )
    .unwrap()
}

fn c_signature(seed: u8) -> scoop_identity::CanonicalCAbiSignatureFingerprintRecord {
    let exact_type = test_exact_type(&format!("bridgeSignature{seed}"));
    let storage = scoop_identity::CanonicalCStorageType::Integer {
        exact_type,
        signedness: scoop_identity::Signedness::Signed,
        bit_width: scoop_identity::IntegerBitWidth::Bits32,
    };
    scoop_identity::CanonicalCAbiSignatureFingerprintRecord::new(
        scoop_identity::CanonicalCAbiFunctionSignature::cdecl(
            vec![scoop_identity::CanonicalCAbiParameter::new(exact_type, storage).unwrap()],
            scoop_identity::CanonicalCAbiReturn::Void,
        ),
    )
    .unwrap()
}

pub(super) fn odr_callable_body(symbol: &str) -> scoop_lir::CallableBodyIdentity {
    let exact_type = test_exact_type(&format!("odr{symbol}"));
    let generated = scoop_identity::CborIdentityRecord::from_key(
        scoop_identity::GeneratedCallableKey::CoroutineStart { result: exact_type },
    )
    .unwrap();
    let group =
        scoop_identity::OdrGroupId::from_key(&scoop_identity::SpecializationKey::StructuralType {
            exact_type,
        })
        .unwrap();
    let member_key = scoop_identity::OdrMemberKey::new(
        group,
        scoop_identity::OdrMemberRole::CallableBody,
        scoop_identity::OdrMemberDiscriminator::GeneratedCallable(generated.id()),
    )
    .unwrap();
    let member = scoop_identity::CallableOdrMemberId::from_key(&member_key).unwrap();
    scoop_lir::CallableBodyIdentity::for_odr_member(member).unwrap()
}

pub(super) fn local_function_ref(index: usize, effect: GcEffect) -> scoop_lir::LocalFunctionRef {
    let mut identities = scoop_lir::LocalFunctionIdentities::default();
    for _ in 0..index {
        identities.alloc_managed();
    }
    match effect {
        GcEffect::Managed => scoop_lir::LocalFunctionRef::Managed(identities.alloc_managed()),
        GcEffect::NoGc => scoop_lir::LocalFunctionRef::NoGc(identities.alloc_no_gc()),
    }
}

pub(super) fn managed_function_ref(index: usize) -> scoop_lir::LocalFunctionRef {
    scoop_lir::LocalFunctionRef::Managed(managed_local_function_ref(index))
}

pub(super) fn no_gc_function_ref(index: usize) -> scoop_lir::LocalFunctionRef {
    scoop_lir::LocalFunctionRef::NoGc(no_gc_local_function_ref(index))
}

pub(super) fn managed_local_function_ref(index: usize) -> scoop_lir::ManagedLocalFunctionRef {
    let scoop_lir::LocalFunctionRef::Managed(reference) =
        local_function_ref(index, GcEffect::Managed)
    else {
        unreachable!()
    };
    reference
}

pub(super) fn no_gc_local_function_ref(index: usize) -> scoop_lir::NoGcLocalFunctionRef {
    let scoop_lir::LocalFunctionRef::NoGc(reference) = local_function_ref(index, GcEffect::NoGc)
    else {
        unreachable!()
    };
    reference
}

pub(super) fn append_executable_entry(module: &mut Module, identity_seed: &str) {
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Return { value: None },
    });
    let index = module.functions.len();
    module.functions.push(Function {
        callable_body: callable_body(identity_seed),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(Vec::new(), LirType::Void),
        call_targets: CallTargets::default(),
        locals: Arena::default(),
        temps: Arena::default(),
        blocks,
        entry,
    });
    module.output = scoop_lir::LirOutput::Executable {
        entry: managed_function_ref(index),
    };
}

pub(super) fn llvm_function_symbol(function: &Function) -> String {
    format!("@\"{}\"", function.symbol())
}

pub(super) fn callable_body_at(source: &str, line: u32) -> scoop_lir::CallableBodyIdentity {
    callable_body(&format!("{source}:{line}"))
}

pub(super) fn runtime_type(name: &str) -> scoop_lir::RuntimeTypeMappingRecord {
    scoop_lir::RuntimeTypeMappingRecord::new(test_exact_type(name)).unwrap()
}

pub(super) fn type_descriptor_identity(name: &str) -> scoop_lir::TypeDescriptorIdentity {
    scoop_lir::TypeDescriptorIdentity::new(
        runtime_type(name),
        scoop_lir::MaterializationRoot::cone_owned(),
    )
    .unwrap()
}

pub(super) fn type_descriptor<'a>(module: &'a Module, name: &str) -> &'a TypeDescriptor {
    module
        .meta
        .type_descriptors
        .iter()
        .find_map(|(_, descriptor)| (descriptor.diagnostic_name == name).then_some(descriptor))
        .unwrap_or_else(|| panic!("missing test TypeDescriptor {name}"))
}

pub(super) fn type_descriptor_symbol(module: &Module, name: &str) -> String {
    type_descriptor(module, name).identity.symbol().to_string()
}

pub(super) fn strong_scan_symbol(module: &Module, scan: scoop_lir::PersistentScanId) -> String {
    let foundation = scoop_lir::OdrFreeLirFoundation::from_module(module)
        .expect("test module has a strong foundation");
    let surface = scoop_lir::StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&foundation)
        .expect("test module has a canonical symbol surface");
    surface
        .plans()
        .iter()
        .find(|plan| {
            plan.owner() == scoop_lir::StrongDefinitionEntity::scan(scan)
                && plan.definition_role() == scoop_lir::StrongDefinitionRole::ScanProgram
        })
        .unwrap_or_else(|| panic!("missing canonical strong scan definition {scan}"))
        .primary_symbol()
        .symbol()
        .to_string()
}

pub(super) fn layout_identity(
    name: &str,
    role: scoop_identity::RepresentationRole,
) -> scoop_lir::LayoutIdentity {
    let exact_type = test_exact_type(name);
    let target = scoop_lir::LirTargetProfile::DARWIN_AARCH64;
    let root = scoop_lir::MaterializationRoot::cone_owned();
    match role {
        scoop_identity::RepresentationRole::ManagedValue => {
            scoop_lir::LayoutIdentity::managed_value(exact_type, target, root)
        }
        scoop_identity::RepresentationRole::ManagedObject => {
            scoop_lir::LayoutIdentity::managed_object(exact_type, target, root)
        }
        scoop_identity::RepresentationRole::CValue => {
            scoop_lir::LayoutIdentity::c_value(exact_type, target, root)
        }
        scoop_identity::RepresentationRole::NativeFunctionPointer => {
            scoop_lir::LayoutIdentity::native_function_pointer(exact_type, target, root)
        }
    }
    .unwrap()
}

pub(super) fn array_layout_identity(name: &str) -> scoop_lir::LayoutIdentity {
    scoop_lir::LayoutIdentity::managed_array(
        test_exact_type(name),
        scoop_lir::LirTargetProfile::DARWIN_AARCH64,
        scoop_lir::MaterializationRoot::cone_owned(),
    )
    .unwrap()
}

pub(super) fn vtable(owner: &str, slots: Vec<scoop_lir::DispatchEntry>) -> scoop_lir::VtableRecord {
    scoop_lir::VtableRecord::new(&type_descriptor_identity(owner), slots).unwrap()
}

pub(super) fn itable(
    owner: &str,
    interface: &str,
    interface_descriptor: scoop_lir::TypeDescriptorRef,
    slots: Vec<scoop_lir::DispatchEntry>,
) -> scoop_lir::ItableRecord {
    scoop_lir::ItableRecord::new(
        &type_descriptor_identity(owner),
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
    scoop_lir::StaticStorageIdentity::property_backing(
        scoop_identity::PropertyOwner::Property(property),
        scoop_lir::MaterializationRoot::cone_owned(),
    )
    .unwrap()
}

pub(super) fn odr_static_storage_identity(name: &str) -> scoop_lir::StaticStorageIdentity {
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let declaration = SourceDeclarationKey::extension_property(
        site,
        CanonicalIdentifier::new(&format!("test{name}")).unwrap(),
        1,
        SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
    );
    let property =
        scoop_identity::PersistentExtensionPropertyId::from_source_declaration(&declaration)
            .unwrap();
    let receiver_arguments = scoop_identity::NonEmptyVec::from_first(test_exact_type(name), []);
    let unit = scoop_identity::PersistentInitializationUnitId::from_key(
        &InitializationUnitKey::GenericDelegatedExtensionApplication {
            property,
            receiver_arguments: receiver_arguments.clone(),
        },
    )
    .unwrap();
    let group = scoop_identity::OdrGroupId::from_key(
        &scoop_identity::SpecializationKey::DelegatedProperty {
            origin: property,
            receiver_arguments,
        },
    )
    .unwrap();
    scoop_lir::StaticStorageIdentity::initialization_failure_root(
        unit,
        scoop_lir::MaterializationRoot::prior_stage_odr(group),
    )
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
    scoop_lir::ImmortalObjectIdentity::from_key(
        scoop_identity::ImmortalObjectKey::string_constant(owner, path),
        scoop_lir::MaterializationRoot::cone_owned(),
    )
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

pub(super) fn host_profile() -> ValidatedBackendProfile {
    ResolvedTargetProfile::resolve_host()
        .expect("supported host target")
        .backend()
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
            RefScan::Array { .. } => {
                unreachable!("Scoop ABI value scans cannot contain variable object scans")
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
        RefScan::Array { .. } => {
            unreachable!("Scoop ABI value scans cannot contain variable object scans")
        }
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
        diagnostic_name: "String".to_string(),
        identity: scoop_lir::TypeDescriptorIdentity::new(
            runtime_type("String"),
            scoop_lir::MaterializationRoot::cone_owned(),
        )
        .unwrap(),
        instance_layout: layouts[string_layout].identity.clone(),
        instance_shape: TypeInstanceShapeV1::inline_bytes(
            scoop_lir::LirTargetProfile::DARWIN_AARCH64,
        )
        .unwrap(),
        parent: None,
        vtable: vtable("String", Vec::new()),
        itables: Vec::new(),
    });
    LirMeta {
        exact_types: Vec::new(),
        target_profile: scoop_lir::LirTargetProfile::DARWIN_AARCH64,
        canonical_c_abi: scoop_lir::CanonicalCAbiMetadata::default(),
        native_externals: scoop_lir::NativeExternalMetadata::default(),
        well_known_layouts: WellKnownLayouts {
            string: string_layout,
        },
        well_known_type_descriptors: WellKnownTypeDescriptors {
            string: TypeDescriptorRef::Local(string_descriptor),
        },
        arrays: Arena::new(),
        layouts,
        type_descriptors,
        core_external_type_descriptors: Arena::new(),
        core_external_callables: Arena::new(),
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
    let element_scan = scan.clone();
    let type_descriptor = meta.type_descriptors.alloc(TypeDescriptor {
        diagnostic_name: name.to_string(),
        identity: type_descriptor_identity(name),
        instance_layout: layout_identity(name, scoop_identity::RepresentationRole::ManagedObject),
        instance_shape: TypeInstanceShapeV1::inline_array(
            scoop_lir::LirTargetProfile::DARWIN_AARCH64,
            if element_size == 0 {
                ArrayElementStorageV1::zero_sized(element_align)
            } else {
                ArrayElementStorageV1::inline(element_size, element_align, scan)
            }
            .unwrap(),
        )
        .unwrap(),
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
        element_scan,
        type_descriptor: TypeDescriptorRef::Local(type_descriptor),
    })
}

/// An M2-shaped module: string constants, a user function exercising
/// alloca/store/load, arithmetic, branches, aggregates and
/// extractvalue, plus calls into the new runtime functions.
pub(super) fn values_module() -> Module {
    let mut globals = Arena::default();
    let hello = globals.alloc(Global {
        address_kind: PointerKind::Managed,
        scan: RefScan::None,
        init: GlobalInit::StringConst {
            identity: immortal_string_identity("hello"),
            value: "hello, ".to_string(),
        },
    });
    let world = globals.alloc(Global {
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
        cone: scoop_identity::ConeIdentity::SINGLE_FILE,
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
            signature: plain_scoop_signature(vec![], LirType::Void),
            call_targets,
            locals,
            temps,
            blocks,
            entry,
        }],
        output: scoop_lir::LirOutput::Executable {
            entry: managed_function_ref(0),
        },
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

/// The canonical persistent shape definitions of a module, without target
/// machine creation or function-body emission.
pub(super) fn strong_shape_ir_of(module: &Module) -> String {
    try_strong_shape_ir_of(module).expect("emit canonical strong shape definitions")
}

pub(super) fn try_strong_shape_ir_of(module: &Module) -> Result<String, CodegenError> {
    let foundation = scoop_lir::OdrFreeLirFoundation::from_module(module)
        .map_err(|error| CodegenError(format!("strong LIR projection failed: {error}")))?;
    let surface = scoop_lir::StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&foundation)
        .map_err(|error| CodegenError(format!("strong symbol projection failed: {error}")))?;
    let context = Context::create();
    let llvm = context.create_module("strong-shape-test");
    let target_data = inkwell::targets::TargetData::create(
        scoop_lir::LirTargetProfile::DARWIN_AARCH64.canonical_llvm_data_layout(),
    );
    let descriptor_type =
        runtime_metadata_v1::RuntimeMetadataV1Types::new(&context).type_descriptor();
    let type_globals = module
        .meta
        .type_descriptors
        .iter()
        .map(|(_, descriptor)| {
            let global = llvm.add_global(descriptor_type, None, descriptor.identity.symbol());
            global.set_linkage(inkwell::module::Linkage::External);
            global.set_constant(true);
            global
        })
        .collect::<Vec<_>>();
    let external_type_globals = module
        .meta
        .core_external_type_descriptors
        .iter()
        .map(|(_, descriptor)| {
            let global = llvm.add_global(
                descriptor_type,
                None,
                descriptor.expected_symbol().symbol().as_str(),
            );
            global.set_linkage(inkwell::module::Linkage::External);
            global
        })
        .collect::<Vec<_>>();
    let placeholder_function_type = context.void_type().fn_type(&[], false);
    for function in &module.functions {
        let function = llvm.add_function(function.symbol(), placeholder_function_type, None);
        function.set_linkage(inkwell::module::Linkage::External);
    }
    for (_, callable) in module.meta.core_external_callables.iter() {
        let function = llvm.add_function(
            callable.expected_symbol().symbol().as_str(),
            placeholder_function_type,
            None,
        );
        function.set_linkage(inkwell::module::Linkage::External);
    }
    shape_definitions::emit_strong_shape_definitions_v1(
        &context,
        &llvm,
        &target_data,
        &surface,
        module,
        &type_globals,
        &external_type_globals,
    )?;
    llvm.verify()
        .map_err(|error| CodegenError(format!("invalid strong shape LLVM module: {error}")))?;
    Ok(llvm.print_to_string().to_string())
}

pub(super) fn write_verified_test_object(module: &Module, output: &Path) {
    validation::validate_module(module).expect("valid test LIR");
    let profile = host_profile();
    let expected_safepoints =
        statepoint::expectations(module).expect("complete safepoint manifest");
    let expected_eh = artifact::eh_expectations(module).expect("complete EH manifest");
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let llvm = emit_llvm_module(&context, module, &machine, profile).expect("emit module");
    llvm.verify().expect("valid LLVM module");
    statepoint::rewrite(&llvm, &machine).expect("rewrite statepoints");
    llvm.verify().expect("valid rewritten LLVM module");
    statepoint::verify_rewritten(&llvm, &expected_safepoints, profile)
        .expect("rewritten manifest agrees with LIR");
    machine
        .write_to_file(&llvm, FileType::Object, output)
        .expect("write object");
    profile
        .verify_object(output, &expected_safepoints, &expected_eh)
        .expect("verified object");
}
