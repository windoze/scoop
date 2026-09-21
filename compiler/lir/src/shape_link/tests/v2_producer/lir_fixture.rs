use la_arena::Arena;
use scoop_identity::*;
use scoop_wire::BudgetMeter;

use super::{TARGET, meter};
use crate::*;

pub(super) fn provider_protocol(
    module: &mut Module,
    exact: PersistentExactTypeId,
) -> CoreLirBridgeBranchV1 {
    if module.cone != ConeIdentity::CORE {
        return CoreLirBridgeBranchV1::NotCore;
    }
    let protocol = function(module.cone, "initializationCycle");
    module.functions.push(local_function(protocol));
    let signature = CanonicalScoopAbiFunctionSignature::new(
        ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), exact),
        Vec::new(),
        ScoopAbiReturn::direct(CanonicalScoopStorage::new(
            exact,
            8,
            std::num::NonZeroU64::new(8).unwrap(),
            scoop_identity::ScoopAbiValueShape::Scalar,
        ))
        .unwrap(),
        scoop_identity::GcEffect::Managed,
    )
    .unwrap();
    CoreLirBridgeBranchV1::Core(CoreLirBridgeV1::new(
        CoreLirInitializationCycleThrowerV1::new(
            StrongCallableDefinitionOwner::Function(protocol),
            signature,
            crate::CallingConvention::Cdecl,
            ExternalCallableRootPlan::ManagedStatepoint,
        )
        .unwrap(),
    ))
}

pub(super) fn provider_module(
    producer: ConeIdentity,
    exact_record: CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
    callable: StrongCallableDefinitionOwner,
    diagnostic_name: String,
) -> Module {
    let exact = exact_record.id();
    let mut layouts = Arena::new();
    layouts.alloc(value_layout(exact));
    let instance = layouts.alloc(instance_layout(exact));
    let identity = TypeDescriptorIdentity::new(
        RuntimeTypeMappingRecord::new(exact).unwrap(),
        MaterializationRoot::cone_owned(),
    )
    .unwrap();
    let mut descriptors = Arena::new();
    let descriptor = descriptors.alloc(TypeDescriptor {
        diagnostic_name,
        vtable: VtableRecord::new(&identity, Vec::new()).unwrap(),
        identity,
        instance_layout: layouts[instance].identity.clone(),
        instance_shape: TypeInstanceShapeV1::fixed_object(TARGET, 16, 8, RefScan::None).unwrap(),
        inline_scan: TypeDescriptorInlineScanV1::Null,
        parent: None,
        itables: Vec::new(),
    });
    let StrongCallableDefinitionOwner::Function(function) = callable else {
        unreachable!()
    };
    module(
        producer,
        vec![exact_record],
        layouts,
        descriptors,
        descriptor,
        vec![local_function(function)],
    )
}

pub(super) fn consumer_module(coordinate: &ConeCoordinate) -> Module {
    let producer = coordinate.identity().unwrap();
    let source = nominal(producer, "Child");
    let exact_record = exact(&source);
    let exact = exact_record.id();
    let graph = DiagnosticGraph::new(coordinate.clone(), source, exact_record.clone());
    let diagnostic_name = CanonicalExactTypeDiagnosticName::from_validated_graph(exact, &graph)
        .unwrap()
        .as_str()
        .to_owned();
    let mut layouts = Arena::new();
    layouts.alloc(value_layout(exact));
    let instance = layouts.alloc(instance_layout(exact));
    let identity = TypeDescriptorIdentity::new(
        RuntimeTypeMappingRecord::new(exact).unwrap(),
        MaterializationRoot::cone_owned(),
    )
    .unwrap();
    let mut descriptors = Arena::new();
    let descriptor = descriptors.alloc(TypeDescriptor {
        diagnostic_name,
        vtable: VtableRecord::new(&identity, Vec::new()).unwrap(),
        identity,
        instance_layout: layouts[instance].identity.clone(),
        instance_shape: TypeInstanceShapeV1::fixed_object(TARGET, 16, 8, RefScan::None).unwrap(),
        inline_scan: TypeDescriptorInlineScanV1::Null,
        parent: None,
        itables: Vec::new(),
    });
    module(
        producer,
        vec![exact_record],
        layouts,
        descriptors,
        descriptor,
        Vec::new(),
    )
}

pub(super) fn module(
    cone: ConeIdentity,
    exact_types: Vec<CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>>,
    layouts: Arena<Layout>,
    type_descriptors: Arena<TypeDescriptor>,
    string: TypeDescriptorId,
    functions: Vec<Function>,
) -> Module {
    Module {
        cone,
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
            exact_types,
            target_profile: TARGET,
            canonical_c_abi: CanonicalCAbiMetadata::default(),
            native_externals: NativeExternalMetadata::default(),
            well_known_type_descriptors: WellKnownTypeDescriptors {
                string: TypeDescriptorRef::Local(string),
            },
            arrays: Arena::new(),
            layouts,
            type_descriptors,
            external_type_descriptors: Arena::new(),
            external_callables: Arena::new(),
        },
    }
}

pub(super) fn exact_layouts(
    foundation: &OdrFreeLirFoundation,
    exact_record: CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
) -> (CanonicalExactLayoutExportsV1, ExactLayoutExportV1) {
    let value_identity = ExactLayoutIdentityV1::from_foundation(
        TARGET,
        exact_record.clone(),
        RepresentationRole::ManagedValue,
        foundation,
        &mut meter(),
    )
    .unwrap();
    let value: ExactLayoutExportV1 = ExactValueLayoutV1::qualified_pointer(
        value_identity,
        NichePointerKind::Managed,
        foundation,
        &mut meter(),
    )
    .unwrap()
    .into();
    let instance_identity = ExactLayoutIdentityV1::from_foundation(
        TARGET,
        exact_record,
        RepresentationRole::ManagedObject,
        foundation,
        &mut meter(),
    )
    .unwrap();
    let instance: ExactLayoutExportV1 = ExactInstanceLayoutV1::class(
        instance_identity,
        ClassLayoutBaseV1::NoBase,
        &[],
        foundation,
        &mut meter(),
    )
    .unwrap()
    .into();
    let layouts = CanonicalExactLayoutExportsV1::try_new(
        TARGET,
        foundation,
        vec![value.clone(), instance],
        &mut meter(),
    )
    .unwrap();
    (layouts, value)
}

pub(super) fn local_function(function: PersistentFunctionId) -> Function {
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_owned(),
        instructions: Vec::new(),
        terminator: Terminator::Return {
            value: Some(Value::NullPointer(PointerKind::Managed)),
        },
    });
    Function {
        callable_body: CallableBodyIdentity::for_function(function).unwrap(),
        gc_effect: crate::GcEffect::Managed,
        signature: pointer_result_signature(),
        call_targets: CallTargets::default(),
        safepoints: SafepointIdentities::default(),
        locals: Arena::new(),
        temps: Arena::new(),
        blocks,
        entry,
    }
}

pub(super) fn pointer_result_signature() -> ScoopAbiSignature {
    ScoopAbiSignature::new(
        Vec::new(),
        AbiReturn::Direct(
            AbiValue::new(
                LirType::Ptr(PointerKind::Managed),
                AbiNonZeroLayout::new(8, 8).unwrap(),
                RefScan::References(vec![0]),
            )
            .unwrap(),
        ),
        crate::CallingConvention::Cdecl,
    )
}

pub(super) fn value_layout(exact: PersistentExactTypeId) -> Layout {
    Layout {
        identity: LayoutIdentity::managed_value(exact, TARGET, MaterializationRoot::cone_owned())
            .unwrap(),
        name: format!("{exact}.value"),
        size: 8,
        align: 8,
        fields: Vec::new(),
        c_layout: None,
        interior_mutable: false,
        kind: LayoutKind::Plain {
            scan: RefScan::References(vec![0]),
        },
    }
}

pub(super) fn instance_layout(exact: PersistentExactTypeId) -> Layout {
    Layout {
        identity: LayoutIdentity::managed_object(exact, TARGET, MaterializationRoot::cone_owned())
            .unwrap(),
        name: format!("{exact}.instance"),
        size: 16,
        align: 8,
        fields: Vec::new(),
        c_layout: None,
        interior_mutable: false,
        kind: LayoutKind::Plain {
            scan: RefScan::None,
        },
    }
}

pub(super) fn nominal(provider: ConeIdentity, name: &str) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            provider,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        0,
    )
}

pub(super) fn exact(
    source: &SourceDeclarationKey,
) -> CborIdentityRecord<PersistentExactTypeId, ExactTypeKey> {
    CborIdentityRecord::from_key(ExactTypeKey::Nominal(
        PersistentTypeId::from_source_declaration(source).unwrap(),
    ))
    .unwrap()
}

pub(super) fn function(provider: ConeIdentity, name: &str) -> PersistentFunctionId {
    PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            provider,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap()
}

pub(super) struct DiagnosticGraph {
    coordinate: ConeCoordinate,
    source: SourceDeclarationKey,
    nominal: PersistentTypeId,
    exact: PersistentExactTypeId,
    key: ExactTypeKey,
}

impl DiagnosticGraph {
    pub(super) fn new(
        coordinate: ConeCoordinate,
        source: SourceDeclarationKey,
        exact: CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
    ) -> Self {
        Self {
            coordinate,
            nominal: PersistentTypeId::from_source_declaration(&source).unwrap(),
            source,
            exact: exact.id(),
            key: exact.key().clone(),
        }
    }
}

impl ExactTypeDiagnosticGraph for DiagnosticGraph {
    fn exact_type_key(&self, id: PersistentExactTypeId) -> Option<&ExactTypeKey> {
        (id == self.exact).then_some(&self.key)
    }

    fn source_type_declaration(&self, id: PersistentTypeId) -> Option<&SourceDeclarationKey> {
        (id == self.nominal).then_some(&self.source)
    }

    fn generated_nominal_key(&self, _: PersistentTypeId) -> Option<&GeneratedNominalKey> {
        None
    }

    fn source_generic_type_declaration(
        &self,
        _: PersistentGenericTypeId,
    ) -> Option<&SourceDeclarationKey> {
        None
    }

    fn cone_coordinate(&self, id: ConeIdentity) -> Option<&ConeCoordinate> {
        (id == self.coordinate.identity().unwrap()).then_some(&self.coordinate)
    }
}

pub(super) struct LayoutSource;

impl LayoutAbiSectionSourceAuthorityV1<()> for LayoutSource {
    fn validate_local_exports(
        &self,
        _: &LayoutAbiExportConstituentsV1,
        _: &mut BudgetMeter,
    ) -> Result<(), ()> {
        Ok(())
    }

    fn committed_semantic_roots(&self) -> Result<&[LayoutAbiDependencyV1], ()> {
        Ok(&[])
    }

    fn validate_physical_imports(
        &self,
        _: &[ExternalShapeLinkImportV1<'_>],
        _: &mut BudgetMeter,
    ) -> Result<(), ()> {
        Ok(())
    }
}
