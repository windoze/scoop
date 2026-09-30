mod providers;

use la_arena::Arena;
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeCoordinate, ConeIdentity, CoreBuiltinNominal,
    DeclarationScope, DefinitionOwnerChain, ExactTypeKey, GeneratedNominalKey, PackagePath,
    PersistentExactTypeId, PersistentTypeId, SourceDeclarationKey, SourceDeclarationSite,
    SourceNominalKind,
};
use scoop_wire::encode;

use super::{
    StrongLirBoxedValueMaterialization, StrongLirShapeSupportError, StrongLirShapeSupportPlan,
};
use crate::{
    CanonicalCAbiMetadata, EnumDefs, ExternFunctions, Layout, LayoutIdentity, LayoutKind, LirMeta,
    LirOutput, LirTargetProfile, MaterializationRoot, Module, NativeExternalMetadata,
    NativeGlobalBridges, RefScan, RuntimeTypeMappingRecord, StructDefs, TypeDescriptor,
    TypeDescriptorIdentity, TypeDescriptorRef, TypeInstanceShapeV1, VtableRecord,
    WellKnownTypeDescriptors,
};

#[test]
fn complete_value_and_reference_roots_seal_with_their_closed_box_branches() {
    let mut module = fixture_module(ConeIdentity::CORE);
    let unit = CoreBuiltinNominal::Unit.declaration_key();
    let any = CoreBuiltinNominal::Any.declaration_key();
    add_shape_support(&mut module, &unit, true);
    add_shape_support(&mut module, &any, false);
    let mut sources = vec![unit, any];
    sources.sort_by_key(source_nominal);

    let plan = StrongLirShapeSupportPlan::from_module(&module, sources).unwrap();

    assert_eq!(plan.roots().len(), 2);
    for root in plan.roots() {
        assert_eq!(root.source().nominal(), source_nominal(root.declaration()));
        assert_eq!(root.source().exact(), source_exact(root.declaration()));
        assert_eq!(
            root.coroutine_step().nominal(),
            generated_nominal(GeneratedNominalKey::CoroutineStep {
                result: root.source().exact(),
            })
        );
        assert_eq!(
            root.coroutine_slot().nominal(),
            generated_nominal(GeneratedNominalKey::CoroutineSlot {
                value: root.source().exact(),
            })
        );
        match root.declaration().declaration_kind() {
            scoop_identity::SourceDeclarationKind::Struct => {
                assert!(matches!(
                    root.boxed_value(),
                    StrongLirBoxedValueMaterialization::Available(_)
                ));
            }
            scoop_identity::SourceDeclarationKind::Class => {
                assert_eq!(
                    root.boxed_value(),
                    &StrongLirBoxedValueMaterialization::NotApplicable
                );
            }
            kind => panic!("unexpected fixture nominal kind {kind:?}"),
        }
    }
}

#[test]
fn non_core_output_rejects_shape_sources() {
    let producer = ConeCoordinate::reserved_single_file().identity().unwrap();
    let module = fixture_module(producer);

    assert!(matches!(
        StrongLirShapeSupportPlan::from_module(
            &module,
            vec![CoreBuiltinNominal::Unit.declaration_key()],
        ),
        Err(StrongLirShapeSupportError::InvalidSource { index: 0 })
    ));
}

#[test]
fn core_output_rejects_invalid_and_noncanonical_source_authority() {
    let mut module = fixture_module(ConeIdentity::CORE);
    let unit = CoreBuiltinNominal::Unit.declaration_key();
    add_shape_support(&mut module, &unit, true);

    assert!(matches!(
        StrongLirShapeSupportPlan::from_module(&module, vec![unit.clone(), unit]),
        Err(StrongLirShapeSupportError::NonCanonicalSources { index: 1, .. })
    ));

    let generic = source_declaration(ConeIdentity::CORE, "Generic", SourceNominalKind::Class, 1);
    assert!(matches!(
        StrongLirShapeSupportPlan::from_module(&module, vec![generic]),
        Err(StrongLirShapeSupportError::InvalidSource { index: 0 })
    ));

    let foreign = source_declaration(
        ConeCoordinate::reserved_single_file().identity().unwrap(),
        "Foreign",
        SourceNominalKind::Class,
        0,
    );
    assert!(matches!(
        StrongLirShapeSupportPlan::from_module(&module, vec![foreign]),
        Err(StrongLirShapeSupportError::InvalidSource { index: 0 })
    ));
}

#[test]
fn core_output_rejects_missing_exact_layout_and_descriptor_materialization() {
    let unit = CoreBuiltinNominal::Unit.declaration_key();
    let exact = source_exact(&unit);

    let module = fixture_module(ConeIdentity::CORE);
    assert!(matches!(
        StrongLirShapeSupportPlan::from_module(&module, vec![unit.clone()]),
        Err(StrongLirShapeSupportError::MissingExactType(found)) if found == exact
    ));

    let mut module = fixture_module(ConeIdentity::CORE);
    module
        .meta
        .exact_types
        .push(CborIdentityRecord::from_key(ExactTypeKey::Nominal(source_nominal(&unit))).unwrap());
    assert!(matches!(
        StrongLirShapeSupportPlan::from_module(&module, vec![unit.clone()]),
        Err(StrongLirShapeSupportError::ManagedValueLayoutSet {
            exact: found,
            actual,
        }) if found == exact && actual.is_empty()
    ));

    let mut module = fixture_module(ConeIdentity::CORE);
    add_layout_without_descriptor(&mut module, source_nominal(&unit));
    assert!(matches!(
        StrongLirShapeSupportPlan::from_module(&module, vec![unit]),
        Err(StrongLirShapeSupportError::TypeDescriptorSet {
            exact: found,
            actual: 0,
        }) if found == exact
    ));
}

#[test]
fn core_output_rejects_reference_box_and_mismatched_descriptor_layout() {
    let any = CoreBuiltinNominal::Any.declaration_key();
    let any_exact = source_exact(&any);
    let mut module = fixture_module(ConeIdentity::CORE);
    add_shape_support(&mut module, &any, false);
    add_exact_record(
        &mut module,
        generated_nominal(GeneratedNominalKey::BoxedValue { payload: any_exact }),
    );
    assert!(matches!(
        StrongLirShapeSupportPlan::from_module(&module, vec![any]),
        Err(StrongLirShapeSupportError::UnexpectedBoxedValue(_))
    ));

    let unit = CoreBuiltinNominal::Unit.declaration_key();
    let unit_exact = source_exact(&unit);
    let mut module = fixture_module(ConeIdentity::CORE);
    add_shape_support(&mut module, &unit, true);
    let anchor_layout = module.meta.layouts.iter().next().unwrap().0;
    let descriptor = module
        .meta
        .type_descriptors
        .iter()
        .find_map(|(id, descriptor)| (descriptor.identity.exact_type() == unit_exact).then_some(id))
        .unwrap();
    module.meta.type_descriptors[descriptor].instance_layout =
        module.meta.layouts[anchor_layout].identity.clone();
    assert!(matches!(
        StrongLirShapeSupportPlan::from_module(&module, vec![unit]),
        Err(StrongLirShapeSupportError::DescriptorLayoutMismatch(found))
            if found == unit_exact
    ));
}

#[test]
fn v2_writer_preserves_legacy_relation_bytes_for_a_final_lir_module() {
    let coordinate = ConeCoordinate::reserved_single_file();
    let module = fixture_module(coordinate.identity().unwrap());
    let output = crate::ConeLirOutput::try_new(module, Vec::new()).unwrap();
    let v1 = output
        .build_production_section(
            coordinate.clone(),
            &[scoop_identity::ConeIdentity::CORE],
            crate::EntryProductionSourceV1::Library,
        )
        .unwrap();
    let v2 = output
        .build_production_section_v2(
            coordinate,
            &[scoop_identity::ConeIdentity::CORE],
            crate::EntryProductionSourceV1::Library,
            &[],
        )
        .unwrap();

    assert_eq!(encode(&v2).unwrap(), encode(&v1).unwrap());
}

fn fixture_module(producer: ConeIdentity) -> Module {
    let anchor = source_declaration(producer, "MetadataAnchor", SourceNominalKind::Class, 0);
    let mut exact_types = Vec::new();
    let mut layouts = Arena::new();
    let mut type_descriptors = Arena::new();
    let (_, _anchor_layout, anchor_descriptor) = add_exact_shape_to_components(
        &mut exact_types,
        &mut layouts,
        &mut type_descriptors,
        source_nominal(&anchor),
    );
    Module {
        cone: producer,
        globals: Arena::new(),
        initialization_units: Arena::new(),
        structs: StructDefs::default(),
        enums: EnumDefs::default(),
        functions: Vec::new(),
        extern_functions: ExternFunctions::default(),
        native_globals: Arena::new(),
        native_global_bridges: NativeGlobalBridges::default(),
        callback_bridges: Arena::new(),
        foreign_callback_families: Arena::new(),
        foreign_callback_bridges: Arena::new(),
        output: LirOutput::Library,
        meta: LirMeta {
            exact_types,
            target_profile: LirTargetProfile::DARWIN_AARCH64,
            canonical_c_abi: CanonicalCAbiMetadata::default(),
            native_externals: NativeExternalMetadata::default(),
            well_known_type_descriptors: WellKnownTypeDescriptors {
                string: TypeDescriptorRef::Local(anchor_descriptor),
            },
            arrays: Arena::new(),
            layouts,
            type_descriptors,
            external_type_descriptors: Arena::new(),
            external_callables: Arena::new(),
        },
    }
}

fn add_shape_support(module: &mut Module, source: &SourceDeclarationKey, boxed: bool) {
    let source_exact = add_exact_shape(module, source_nominal(source));
    if boxed {
        add_exact_shape(
            module,
            generated_nominal(GeneratedNominalKey::BoxedValue {
                payload: source_exact,
            }),
        );
    }
    add_exact_shape(
        module,
        generated_nominal(GeneratedNominalKey::CoroutineStep {
            result: source_exact,
        }),
    );
    add_exact_shape(
        module,
        generated_nominal(GeneratedNominalKey::CoroutineSlot {
            value: source_exact,
        }),
    );
}

fn add_exact_record(module: &mut Module, nominal: PersistentTypeId) -> PersistentExactTypeId {
    let record = CborIdentityRecord::from_key(ExactTypeKey::Nominal(nominal)).unwrap();
    let exact = record.id();
    module.meta.exact_types.push(record);
    exact
}

fn add_layout_without_descriptor(
    module: &mut Module,
    nominal: PersistentTypeId,
) -> PersistentExactTypeId {
    let exact = add_exact_record(module, nominal);
    module.meta.layouts.alloc(layout(exact));
    exact
}

fn add_exact_shape(module: &mut Module, nominal: PersistentTypeId) -> PersistentExactTypeId {
    add_exact_shape_to_components(
        &mut module.meta.exact_types,
        &mut module.meta.layouts,
        &mut module.meta.type_descriptors,
        nominal,
    )
    .0
}

fn add_exact_shape_to_components(
    exact_types: &mut Vec<CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>>,
    layouts: &mut Arena<Layout>,
    type_descriptors: &mut Arena<TypeDescriptor>,
    nominal: PersistentTypeId,
) -> (
    PersistentExactTypeId,
    crate::LayoutId,
    crate::TypeDescriptorId,
) {
    let exact_record = CborIdentityRecord::from_key(ExactTypeKey::Nominal(nominal)).unwrap();
    let exact = exact_record.id();
    exact_types.push(exact_record);
    let layout = layouts.alloc(layout(exact));
    let instance_layout = layouts.alloc(instance_layout(exact));
    let identity = TypeDescriptorIdentity::new(
        RuntimeTypeMappingRecord::new(exact).unwrap(),
        MaterializationRoot::cone_owned(),
    )
    .unwrap();
    let descriptor = type_descriptors.alloc(TypeDescriptor {
        relations: Default::default(),
        diagnostic_name: nominal.to_string(),
        vtable: VtableRecord::new(&identity, Vec::new()).unwrap(),
        identity,
        instance_layout: layouts[instance_layout].identity.clone(),
        instance_shape: TypeInstanceShapeV1::abstract_ref(),
        inline_scan: crate::TypeDescriptorInlineScanV1::Null,
        parent: None,
        itables: Vec::new(),
    });
    (exact, layout, descriptor)
}

fn layout(exact: PersistentExactTypeId) -> Layout {
    Layout {
        identity: LayoutIdentity::managed_value(
            exact,
            LirTargetProfile::DARWIN_AARCH64,
            MaterializationRoot::cone_owned(),
        )
        .unwrap(),
        name: exact.to_string(),
        size: 0,
        align: 1,
        fields: Vec::new(),
        c_layout: None,
        interior_mutable: false,
        kind: LayoutKind::Plain {
            scan: RefScan::None,
        },
    }
}

fn instance_layout(exact: PersistentExactTypeId) -> Layout {
    Layout {
        identity: LayoutIdentity::managed_object(
            exact,
            LirTargetProfile::DARWIN_AARCH64,
            MaterializationRoot::cone_owned(),
        )
        .unwrap(),
        name: format!("{exact}.instance"),
        size: 0,
        align: 1,
        fields: Vec::new(),
        c_layout: None,
        interior_mutable: false,
        kind: LayoutKind::Plain {
            scan: RefScan::None,
        },
    }
}

fn source_declaration(
    producer: ConeIdentity,
    name: &str,
    kind: SourceNominalKind,
    type_parameter_count: u32,
) -> SourceDeclarationKey {
    let site = SourceDeclarationSite::new(
        producer,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    SourceDeclarationKey::nominal(
        site,
        CanonicalIdentifier::new(name).unwrap(),
        kind,
        type_parameter_count,
    )
}

fn source_nominal(source: &SourceDeclarationKey) -> PersistentTypeId {
    PersistentTypeId::from_source_declaration(source).unwrap()
}

fn source_exact(source: &SourceDeclarationKey) -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(source_nominal(source))).unwrap()
}

fn generated_nominal(key: GeneratedNominalKey) -> PersistentTypeId {
    PersistentTypeId::from_generated_key(&key).unwrap()
}
