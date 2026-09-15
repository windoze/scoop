use scoop_identity::{
    BindingNamespace, BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeIdentity,
    CoreBuiltinNominal, DeclarationName, DeclarationScope, DefinitionOrigin,
    DefinitionOriginRecord, DefinitionOriginSubject, DefinitionOwnerChain, EnumVariantFieldKey,
    EnumVariantFieldSelector, EnumVariantIdentityKey, ExactOrdinaryNoArgUnitSignature,
    ExactTypeKey, ExecutableSourceEntryIdentity, ExportBindingKey, NormalizedSourcePath,
    PackagePath, PendingIdentityValidation, PersistentEnumVariantFieldId, PersistentEnumVariantId,
    PersistentExactTypeId, PersistentExportBindingId, PersistentFunctionId,
    PersistentGenericTypeId, PersistentTypeId, SemanticIdentitySession, SemanticOriginFingerprint,
    SourceContextKey, SourceDeclarationKey, SourceDeclarationSite, SourceIdentity,
    SourceNominalKind, SourceSpan,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;
use crate::{
    CorePreludeImportError, CorePreludeUnavailableCapability, CoreTypeTargetV1,
    DecodedHirFoundation, ImportedCorePreludeTarget, ImportedHirFoundation,
};

#[test]
fn core_interface_has_a_fixed_wire_vector_and_validates_atomically() {
    let fixture = fixture();
    let bytes = encode(&fixture.interface).unwrap();
    assert_eq!(
        hex(&bytes),
        "a501a4018258207f51ff5c92b58632d1c34a6bbb2a6a049b7f62f9346af2ff15bc634796879c745820e05985483f0ca519145c05813043883a14295cb251b64e08bc92875c8fa2951f025820faf63376f9f38e514ad8979cc6cdeb6a9b86ce537a42088d289fe72f3888e0360358204e898ae8df7bb1bf27b577557433460cd313549ac96a109058dd1499115b5e5b045820dbb5e74c2eb2076e994fea90f134c93ad7ada2ba9a9ef77cb0318c254c935a3802a300010158200252c865acc9bf0786a7b7e2b1ca777f76ae038ab51bb159b9a6ad961aad6097025820ad3ae7a719e82101f547257b8a8ac185f05531c14504be81962250566a3ee86803800482a30158207f51ff5c92b58632d1c34a6bbb2a6a049b7f62f9346af2ff15bc634796879c7402a200020158207b55673b75f26171d5e7f0d0cc721d4721bd8cb8f96cf54c7df9bef4a95b520a03a200030101a3015820e05985483f0ca519145c05813043883a14295cb251b64e08bc92875c8fa2951f02a200010158200252c865acc9bf0786a7b7e2b1ca777f76ae038ab51bb159b9a6ad961aad609703a20001015820ad3ae7a719e82101f547257b8a8ac185f05531c14504be81962250566a3ee8680580"
    );

    assert_eq!(
        decode_interface(&fixture.interface).validate_against(&fixture.foundation, &fixture.direct),
        Ok(fixture.interface)
    );
}

#[test]
fn interface_and_section_readers_require_closed_products_and_sums() {
    for bytes in [vec![0xa4], vec![0xa6], vec![0xa5, 0x06, 0x00]] {
        assert!(
            decode_canonical::<DecodedCoreHirInterfaceV1>(&bytes, DecodeLimits::default()).is_err()
        );
    }

    for bytes in [
        vec![0xa0],
        vec![0xa1, 0x00, 0x03],
        vec![0xa2, 0x00, 0x01, 0x01, 0x00],
    ] {
        assert!(
            decode_canonical::<DecodedCoreHirInterfaceBranchV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
    }

    for bytes in [
        vec![0xa2],
        vec![0xa4],
        vec![
            0xa3, 0x01, 0xa1, 0x00, 0x01, 0x02, 0xa1, 0x00, 0x01, 0x04, 0x80,
        ],
    ] {
        assert!(
            decode_canonical::<DecodedCoreBootstrapInterfaceSectionV1>(
                &bytes,
                DecodeLimits::default(),
            )
            .is_err()
        );
    }
}

#[test]
fn core_branch_and_non_core_section_have_fixed_wire_vectors() {
    assert_eq!(
        hex(&encode(&CoreHirInterfaceBranchV1::NotCore).unwrap()),
        "a10001"
    );

    let section = non_core_section();
    assert_eq!(hex(&encode(&section).unwrap()), "a301a1000102a100010380");
    assert_eq!(
        decode_section(&section)
            .validate_against(ConeIdentity::SINGLE_FILE, &CanonicalHirFoundation::empty(),),
        Ok(section)
    );
}

#[test]
fn section_selects_exactly_one_branch_from_the_artifact_identity() {
    let fixture = fixture();
    assert_eq!(
        decode_section(&fixture.section).validate_against(ConeIdentity::CORE, &fixture.foundation),
        Ok(fixture.section.clone())
    );
    assert_eq!(
        decode_section(&fixture.section)
            .validate_against(ConeIdentity::SINGLE_FILE, &fixture.foundation),
        Err(
            CoreBootstrapInterfaceValidationError::UnexpectedCoreInterface(
                ConeIdentity::SINGLE_FILE,
            )
        )
    );

    let not_core = CoreBootstrapInterfaceSectionV1 {
        core_interface: CoreHirInterfaceBranchV1::NotCore,
        output_contract: HirOutputContractV1::Library,
        direct_public_surface: fixture.direct,
    };
    assert_eq!(
        decode_section(&not_core).validate_against(ConeIdentity::CORE, &fixture.foundation),
        Err(CoreBootstrapInterfaceValidationError::MissingCoreInterface)
    );
}

#[test]
fn core_section_rejects_an_executable_output_contract() {
    let mut fixture = fixture();
    let entry = function("main");
    fixture
        .foundation
        .set_functions(vec![entry.clone()])
        .unwrap();
    let proof = ExecutableSourceEntryIdentity::try_new(
        &entry,
        ExactOrdinaryNoArgUnitSignature::new(unit_exact_record().id()),
    )
    .unwrap();
    let executable = CoreBootstrapInterfaceSectionV1 {
        core_interface: CoreHirInterfaceBranchV1::Core(Box::new(fixture.interface)),
        output_contract: HirOutputContractV1::Executable(Box::new(proof)),
        direct_public_surface: fixture.direct,
    };

    assert_eq!(
        decode_section(&executable).validate_against(ConeIdentity::CORE, &fixture.foundation),
        Err(CoreBootstrapInterfaceValidationError::CoreMustBeLibrary)
    );
}

#[test]
fn interface_relations_require_one_shared_complete_surface() {
    let fixture = fixture();

    let mut wrong_prelude = fixture.interface.clone();
    wrong_prelude.prelude_snapshot.ordinary_bindings =
        CanonicalDirectPublicSurfaceV1::try_new(Vec::new()).unwrap();
    assert_eq!(
        validate_relations(&wrong_prelude, &fixture.direct),
        Err(CoreHirInterfaceRelationError::PreludeSurfaceMismatch)
    );

    let mut incomplete = fixture.interface.clone();
    let string_target = incomplete
        .type_targets
        .targets()
        .iter()
        .find(|target| matches!(target.definition(), CoreTypeDefinitionV1::Type(_)))
        .unwrap()
        .clone();
    incomplete.type_targets = CoreTypeTargetSurfaceV1::try_new(vec![string_target]).unwrap();
    assert_eq!(
        validate_relations(&incomplete, &fixture.direct),
        Err(CoreHirInterfaceRelationError::ConstituentCoverage {
            expected: 2,
            actual: 1,
        })
    );

    let mut wrong_string_target = fixture.interface;
    let targets = wrong_string_target
        .type_targets
        .targets()
        .iter()
        .cloned()
        .map(|mut target| {
            if matches!(target.definition, CoreTypeDefinitionV1::Type(_)) {
                target.capability =
                    CoreHirTypeCapabilityV1::StructuralUnavailable(fixture.string_exact);
            }
            target
        })
        .collect();
    wrong_string_target.type_targets = CoreTypeTargetSurfaceV1::try_new(targets).unwrap();
    assert_eq!(
        validate_relations(&wrong_string_target, &fixture.direct),
        Err(CoreHirInterfaceRelationError::StringTypeTargetMismatch)
    );
}

#[test]
fn core_shape_support_sources_are_derived_from_param_free_source_nominals() {
    let fixture = fixture();
    let foundation = OdrFreeHirFoundation::try_new(fixture.foundation).unwrap();
    let sources = fixture
        .interface
        .param_free_shape_support_sources(&foundation)
        .unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(
        PersistentTypeId::from_source_declaration(&sources[0]).unwrap(),
        fixture.interface.string_capability().source_type()
    );
}

#[test]
fn imported_core_prelude_exposes_only_the_checked_lookup_surface() {
    let fixture = fixture();
    let imported = imported_foundation(&fixture.foundation);

    let prelude = imported
        .import_core_prelude(&fixture.interface, &[])
        .unwrap();

    assert_eq!(prelude.origin(), ConeIdentity::CORE);
    let string = prelude
        .candidates(BindingNamespace::Type, "String")
        .collect::<Vec<_>>();
    assert_eq!(string.len(), 1);
    let string_binding = fixture
        .interface
        .type_targets()
        .targets()
        .iter()
        .find(|target| {
            target.capability() == CoreHirTypeCapabilityV1::ParamFreeStrong(fixture.string_exact)
        })
        .unwrap()
        .binding();
    assert_eq!(string[0].identity().persistent(), string_binding);
    assert!(matches!(
        string[0].target(),
        ImportedCorePreludeTarget::Type(target)
            if target.capability()
                == CoreHirTypeCapabilityV1::ParamFreeStrong(fixture.string_exact)
    ));
    let selected = string[0].select_param_free_strong().unwrap();
    let other_imported = imported_foundation(&fixture.foundation);
    let other_interface = fixture.interface.clone();
    assert!(selected.belongs_to(&imported, &fixture.interface, &[]));
    assert!(!selected.belongs_to(&other_imported, &fixture.interface, &[]));
    assert!(!selected.belongs_to(&imported, &other_interface, &[]));
    assert_eq!(selected.binding(), string[0].identity());
    assert!(matches!(
        selected.target(),
        ImportedCorePreludeTarget::Type(target)
            if target.capability()
                == CoreHirTypeCapabilityV1::ParamFreeStrong(fixture.string_exact)
    ));

    let option = prelude
        .candidates(BindingNamespace::Type, "Option")
        .next()
        .unwrap();
    let error = option.select_param_free_strong().unwrap_err();
    assert_eq!(error.required(), CorePreludeUnavailableCapability::Generic);
    assert_eq!(error.binding(), option.identity());
    assert_eq!(error.code(), "SCOOPC_CAPABILITY_CORE_GENERIC_UNAVAILABLE");
    assert_eq!(prelude.string_exact().persistent(), fixture.string_exact);
    assert_eq!(
        prelude.option_some().persistent(),
        fixture.interface.prelude_snapshot().option_some()
    );
    assert_eq!(
        prelude.option_some_payload().persistent(),
        fixture.interface.prelude_snapshot().option_some_payload()
    );
    assert_eq!(
        prelude.option_none().persistent(),
        fixture.interface.prelude_snapshot().option_none()
    );
}

#[test]
fn selected_imported_core_set_is_typed_deduplicated_and_atomic() {
    let fixture = fixture();
    let imported = imported_foundation(&fixture.foundation);
    let prelude = imported
        .import_core_prelude(&fixture.interface, &[])
        .unwrap();
    let string = prelude
        .candidates(BindingNamespace::Type, "String")
        .next()
        .unwrap();
    let option = prelude
        .candidates(BindingNamespace::Type, "Option")
        .next()
        .unwrap();
    let mut selected = prelude.selected_set();

    let first = selected.select(string).unwrap();
    let second = selected.select(string).unwrap();
    assert_eq!(first, second);
    assert_eq!(selected.callable_count(), 0);
    assert_eq!(selected.type_count(), 1);
    assert_eq!(selected.value_count(), 0);
    let crate::SelectedImportedCoreId::Type(id) = first else {
        panic!("String must enter the imported type id domain")
    };
    let retained = selected.ty(id).unwrap();
    assert_eq!(retained.binding(), string.identity());
    assert!(retained.belongs_to(&imported, &fixture.interface, &[]));

    let error = selected.select(option).unwrap_err();
    assert!(matches!(
        error,
        crate::CorePreludeSelectionError::Capability(error)
            if error.required() == CorePreludeUnavailableCapability::Generic
    ));
    assert_eq!(selected.type_count(), 1);
    assert_eq!(selected.callable_count(), 0);
    assert_eq!(selected.value_count(), 0);
}

#[test]
fn selected_imported_core_set_rejects_a_foreign_prelude_binding() {
    let fixture = fixture();
    let imported = imported_foundation(&fixture.foundation);
    let other_imported = imported_foundation(&fixture.foundation);
    let prelude = imported
        .import_core_prelude(&fixture.interface, &[])
        .unwrap();
    let other_prelude = other_imported
        .import_core_prelude(&fixture.interface, &[])
        .unwrap();
    let foreign = other_prelude
        .candidates(BindingNamespace::Type, "String")
        .next()
        .unwrap();
    let mut selected = prelude.selected_set();

    assert!(matches!(
        selected.select(foreign),
        Err(crate::CorePreludeSelectionError::ForeignBinding(binding))
            if binding == foreign.identity().persistent()
    ));
    assert_eq!(selected.type_count(), 0);
}

#[test]
fn imported_core_prelude_rejects_an_interface_from_another_foundation() {
    let fixture = fixture();
    let first_binding = fixture.direct.bindings()[0];
    let mut missing_binding = fixture.foundation.clone();
    missing_binding.set_export_bindings(Vec::new()).unwrap();
    let imported = imported_foundation(&missing_binding);

    assert!(matches!(
        imported.import_core_prelude(&fixture.interface, &[]),
        Err(CorePreludeImportError::MissingBindingKey(binding)) if binding == first_binding
    ));

    assert!(matches!(
        imported_foundation(&fixture.foundation)
            .import_core_prelude(&fixture.interface, &[first_binding]),
        Err(CorePreludeImportError::UnknownStrongCallableBinding(binding))
            if binding == first_binding
    ));
}

fn decode_interface(interface: &CoreHirInterfaceV1) -> DecodedCoreHirInterfaceV1 {
    decode_canonical(&encode(interface).unwrap(), DecodeLimits::default()).unwrap()
}

fn decode_section(
    section: &CoreBootstrapInterfaceSectionV1,
) -> DecodedCoreBootstrapInterfaceSectionV1 {
    decode_canonical(&encode(section).unwrap(), DecodeLimits::default()).unwrap()
}

fn non_core_section() -> CoreBootstrapInterfaceSectionV1 {
    CoreBootstrapInterfaceSectionV1 {
        core_interface: CoreHirInterfaceBranchV1::NotCore,
        output_contract: HirOutputContractV1::Library,
        direct_public_surface: CanonicalDirectPublicSurfaceV1::try_new(Vec::new()).unwrap(),
    }
}

struct Fixture {
    foundation: CanonicalHirFoundation,
    direct: CanonicalDirectPublicSurfaceV1,
    interface: CoreHirInterfaceV1,
    section: CoreBootstrapInterfaceSectionV1,
    string_exact: PersistentExactTypeId,
}

fn fixture() -> Fixture {
    let string_key = nominal("String", SourceNominalKind::Class, 0);
    let option_key = nominal("Option", SourceNominalKind::Enum, 1);
    let string: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> =
        CborIdentityRecord::from_key(string_key).unwrap();
    let option: CborIdentityRecord<PersistentGenericTypeId, SourceDeclarationKey> =
        CborIdentityRecord::from_key(option_key).unwrap();
    let string_id = string.id();
    let option_id = option.id();

    let string_exact_record: CborIdentityRecord<PersistentExactTypeId, ExactTypeKey> =
        CborIdentityRecord::from_key(ExactTypeKey::Nominal(string_id)).unwrap();
    let some: CborIdentityRecord<PersistentEnumVariantId, EnumVariantIdentityKey> =
        CborIdentityRecord::from_key(
            EnumVariantIdentityKey::source(option.key(), CanonicalIdentifier::new("Some").unwrap())
                .unwrap(),
        )
        .unwrap();
    let none: CborIdentityRecord<PersistentEnumVariantId, EnumVariantIdentityKey> =
        CborIdentityRecord::from_key(
            EnumVariantIdentityKey::source(option.key(), CanonicalIdentifier::new("None").unwrap())
                .unwrap(),
        )
        .unwrap();
    let some_payload = variant_field(some.id(), 0);
    let string_binding = type_binding(string.key());
    let option_binding = type_binding(option.key());
    let direct =
        CanonicalDirectPublicSurfaceV1::try_new(vec![string_binding.id(), option_binding.id()])
            .unwrap();
    let type_targets = CoreTypeTargetSurfaceV1::try_new(vec![
        CoreTypeTargetV1 {
            binding: string_binding.id(),
            definition: CoreTypeDefinitionV1::Type(string_id),
            capability: CoreHirTypeCapabilityV1::ParamFreeStrong(string_exact_record.id()),
        },
        CoreTypeTargetV1 {
            binding: option_binding.id(),
            definition: CoreTypeDefinitionV1::GenericType(option_id),
            capability: CoreHirTypeCapabilityV1::GenericUnavailable {
                type_parameter_count: 1,
            },
        },
    ])
    .unwrap();
    let interface = CoreHirInterfaceV1 {
        prelude_snapshot: CorePreludeSnapshotV1 {
            ordinary_bindings: direct.clone(),
            option_some: some.id(),
            option_some_payload: some_payload.id(),
            option_none: none.id(),
        },
        string_capability: RuntimeCoreCapabilityV1::String {
            source_type: string_id,
            exact_type: string_exact_record.id(),
        },
        callable_targets: CoreCallableTargetSurfaceV1::try_new(Vec::new()).unwrap(),
        type_targets,
        value_targets: CoreValueTargetSurfaceV1::try_new(Vec::new()).unwrap(),
    };
    let section = CoreBootstrapInterfaceSectionV1 {
        core_interface: CoreHirInterfaceBranchV1::Core(Box::new(interface.clone())),
        output_contract: HirOutputContractV1::Library,
        direct_public_surface: direct.clone(),
    };

    let mut foundation = CanonicalHirFoundation::empty();
    foundation
        .set_types(vec![CoreBuiltinNominal::Unit.identity_record(), string])
        .unwrap();
    foundation.set_generic_types(vec![option]).unwrap();
    foundation.set_enum_variants(vec![some, none]).unwrap();
    foundation
        .set_enum_variant_fields(vec![some_payload])
        .unwrap();
    foundation
        .set_exact_types(vec![string_exact_record.clone(), unit_exact_record()])
        .unwrap();
    foundation
        .set_export_bindings(vec![string_binding, option_binding])
        .unwrap();
    foundation
        .set_definition_origins(vec![
            origin_record(DefinitionOriginSubject::Type(string_id)),
            origin_record(DefinitionOriginSubject::GenericType(option_id)),
        ])
        .unwrap();

    Fixture {
        foundation,
        direct,
        interface,
        section,
        string_exact: string_exact_record.id(),
    }
}

fn imported_foundation(foundation: &CanonicalHirFoundation) -> ImportedHirFoundation {
    let decoded: DecodedHirFoundation =
        decode_canonical(&encode(foundation).unwrap(), DecodeLimits::default()).unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    decoded.register_identities(&mut pending).unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    let identities = pending.finish().unwrap();
    let mut session = SemanticIdentitySession::new();
    let imported = session
        .import(
            ConeIdentity::CORE,
            SemanticOriginFingerprint::new([1; 32], [2; 32], [3; 32]),
            &identities,
        )
        .unwrap();
    let (hir, _, _) = imported.into_parts();
    ImportedHirFoundation::from_odr_free(
        OdrFreeHirFoundation::try_new(foundation.clone()).unwrap(),
        hir,
    )
}

fn nominal(name: &str, kind: SourceNominalKind, type_parameter_count: u32) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        site(),
        CanonicalIdentifier::new(name).unwrap(),
        kind,
        type_parameter_count,
    )
}

fn type_binding(
    declaration: &SourceDeclarationKey,
) -> CborIdentityRecord<PersistentExportBindingId, ExportBindingKey> {
    let DeclarationName::Named(name) = declaration.name() else {
        panic!("fixture declaration must be named")
    };
    CborIdentityRecord::from_key(ExportBindingKey::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        name.clone(),
        BindingTarget::type_name(declaration).unwrap(),
    ))
    .unwrap()
}

fn function(name: &str) -> CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::function(
        site(),
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap()
}

fn unit_exact_record() -> CborIdentityRecord<PersistentExactTypeId, ExactTypeKey> {
    CborIdentityRecord::from_key(ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap()
}

fn variant_field(
    variant: PersistentEnumVariantId,
    declaration_index: u32,
) -> CborIdentityRecord<PersistentEnumVariantFieldId, EnumVariantFieldKey> {
    CborIdentityRecord::from_key(EnumVariantFieldKey::new(
        variant,
        EnumVariantFieldSelector::Positional { declaration_index },
    ))
    .unwrap()
}

fn origin_record(subject: DefinitionOriginSubject) -> DefinitionOriginRecord {
    let source = SourceIdentity::new(
        ConeIdentity::CORE,
        NormalizedSourcePath::new("src/core.scoop").unwrap(),
    )
    .unwrap();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    let origin = DefinitionOrigin::new(source, SourceSpan::new(0, 0).unwrap(), &context).unwrap();
    DefinitionOriginRecord::new(subject, origin)
}

fn site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
