use la_arena::Arena;
use scoop_identity::{
    BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, CoreBuiltinNominal,
    DeclarationName, DeclarationScope, DefinitionOwnerChain, EnumVariantFieldKey,
    EnumVariantFieldSelector, EnumVariantIdentityKey, ExactOrdinaryNoArgUnitSignature,
    ExactTypeKey, ExecutableSourceEntryIdentity, ExportBindingKey, PackagePath,
    PendingIdentityValidation, PersistentEnumVariantFieldId, PersistentEnumVariantId,
    PersistentExactTypeId, PersistentExportBindingId, PersistentFunctionId,
    PersistentGenericTypeId, PersistentTypeId, SemanticIdentitySession, SemanticOriginFingerprint,
    SignatureTypeKey, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{decode_canonical, encode};

use super::*;
use crate::{DecodedHirFoundation, ImportedHirFoundation, PublicNominalShapeRequirementsV1};

mod wire;

#[test]
fn protocol_definitions_are_data_independent_of_the_artifact_coordinate() {
    let fixture = fixture();
    for artifact in [ConeIdentity::CORE, ConeIdentity::SINGLE_FILE] {
        assert_eq!(
            decode_section(&fixture.section).validate_against(artifact, &fixture.foundation),
            Ok(fixture.section.clone())
        );
        let imported = CoreBootstrapInterfaceSectionV1 {
            compiler_protocols: None,
            output_contract: HirOutputContractV1::Library,
            direct_public_surface: fixture.direct.clone(),
        };
        assert_eq!(
            decode_section(&imported).validate_against(artifact, &fixture.foundation),
            Ok(imported)
        );
    }
}

#[test]
fn ordinary_provider_definitions_validate_and_import_their_actual_typed_roles() {
    let provider = crate::production::core_protocol_test_support::ordinary_origin();
    let (compiler_protocols, mut foundation) =
        crate::production::core_protocol_test_support::standalone_at(provider);
    let source_type = compiler_protocols.string_source_type();
    let exact = CborIdentityRecord::from_key(ExactTypeKey::Nominal(source_type)).unwrap();
    let mut exact_types = foundation.type_source_exact_records().to_vec();
    exact_types.extend([exact.clone(), unit_exact_record()]);
    foundation.set_exact_types(exact_types).unwrap();
    let definitions = compiler_protocols;
    let section = CoreBootstrapInterfaceSectionV1 {
        compiler_protocols: Some(Box::new(definitions.clone())),
        ..non_core_section()
    };
    assert_eq!(
        decode_section(&section).validate_against(provider, &foundation),
        Ok(section)
    );
    let imported = imported_foundation_at(&foundation, provider);
    assert_eq!(
        imported
            .import_core_inputs(&definitions)
            .unwrap()
            .protocols()
            .fundamental_types()
            .string()
            .persistent(),
        source_type
    );
}

#[test]
fn protocol_definitions_do_not_replace_the_executable_output_contract() {
    let mut fixture = fixture();
    let entry = function("main");
    let mut functions = fixture.foundation.type_source_function_records().to_vec();
    functions.push(entry.clone());
    fixture.foundation.set_functions(functions).unwrap();
    let proof = ExecutableSourceEntryIdentity::try_new(
        &entry,
        ExactOrdinaryNoArgUnitSignature::new(unit_exact_record().id()),
    )
    .unwrap();
    let executable = CoreBootstrapInterfaceSectionV1 {
        compiler_protocols: Some(Box::new(fixture.interface)),
        output_contract: HirOutputContractV1::Executable(Box::new(proof)),
        direct_public_surface: fixture.direct,
    };

    assert_eq!(
        decode_section(&executable).validate_against(ConeIdentity::CORE, &fixture.foundation),
        Ok(executable)
    );
}

#[test]
fn shape_support_sources_are_derived_from_param_free_source_nominals() {
    let fixture = fixture();
    let foundation = OdrFreeHirFoundation::try_new(fixture.foundation).unwrap();
    let sources = PublicNominalShapeRequirementsV1::from_direct_surface(
        ConeIdentity::CORE,
        &fixture.direct,
        foundation.as_canonical(),
    )
    .unwrap()
    .source_declarations(foundation.as_canonical())
    .unwrap();
    let mut expected = vec![
        fixture.interface.string_source_type(),
        scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id(),
        scoop_identity::CoreBuiltinNominal::Any
            .identity_record()
            .id(),
    ];
    expected.sort_unstable();
    assert_eq!(
        sources
            .iter()
            .map(|source| PersistentTypeId::from_source_declaration(source).unwrap())
            .collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn imported_core_inputs_expose_compiler_protocols() {
    let fixture = fixture();
    let imported = imported_foundation(&fixture.foundation);

    let core = imported.import_core_inputs(&fixture.interface).unwrap();
    assert_eq!(core.protocols().fixed_subject_count(), 84);
    let protocols = core.protocols().clone();
    assert_eq!(
        protocols.fundamental_types().unit().persistent(),
        CoreBuiltinNominal::Unit.identity_record().id()
    );
    assert_eq!(
        protocols.fundamental_types().string().persistent(),
        fixture.interface.string_source_type()
    );
    let integers = crate::IntegerKind::ALL
        .map(|kind| protocols.fundamental_types().integer(kind).persistent());
    assert_eq!(
        integers
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        integers.len()
    );
    assert_eq!(
        protocols.option().some().persistent(),
        fixture.interface.option_some()
    );
    assert_eq!(
        protocols.option().some_payload().persistent(),
        fixture.interface.option_some_payload()
    );
    assert_eq!(
        protocols.option().none().persistent(),
        fixture.interface.option_none()
    );
    assert_ne!(
        protocols.iteration().next().definition(),
        protocols.exceptions().throwable_constructor().definition()
    );
    assert_ne!(
        protocols.foreign_callbacks().reusable(),
        protocols.foreign_callbacks().one_shot()
    );
}

#[test]
fn imported_fundamental_types_build_identities_without_local_core_nominals() {
    let fixture = fixture();
    let imported = imported_foundation(&fixture.foundation);
    let core = imported.import_core_inputs(&fixture.interface).unwrap();
    let fundamental = core.protocols().fundamental_types();

    let mut types = Arena::new();
    let unit = types.alloc(crate::Type::Unit);
    let integer = types.alloc(crate::Type::Integer(crate::IntegerKind::SIGNED_32));
    let boolean = types.alloc(crate::Type::Boolean);
    let string = types.alloc(crate::Type::String);
    let function_types = Arena::new();
    let structs = Arena::new();
    let struct_applications = Arena::new();
    let enums = Arena::new();
    let enum_applications = Arena::new();
    let classes = Arena::new();
    let class_applications = Arena::new();
    let interfaces = Arena::new();
    let interface_applications = Arena::new();
    let objects = Arena::new();
    let nominals = crate::HirNominalIdentities::checked(
        &structs,
        Vec::new(),
        &enums,
        Vec::new(),
        &classes,
        Vec::new(),
        &interfaces,
        Vec::new(),
        &objects,
        Vec::new(),
    )
    .unwrap();
    let inputs = crate::HirTypeIdentityInputs {
        types: &types,
        function_types: &function_types,
        structs: &structs,
        struct_applications: &struct_applications,
        enums: &enums,
        loaded_enum_definitions: &std::collections::HashMap::new(),
        loaded_struct_definitions: &std::collections::HashMap::new(),
        loaded_class_definitions: &std::collections::HashMap::new(),
        loaded_interface_definitions: &std::collections::HashMap::new(),
        enum_applications: &enum_applications,
        classes: &classes,
        class_applications: &class_applications,
        interfaces: &interfaces,
        interface_applications: &interface_applications,
        objects: &objects,
        core_types: crate::HirCoreTypeIdentityAuthority::Imported(fundamental),
        nominal_identities: &nominals,
    };
    let identities = crate::HirTypeIdentities::from_types(inputs).unwrap();
    assert_eq!(
        identities[unit].exact().unwrap().key(),
        &ExactTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id())
    );
    assert_eq!(
        identities[integer].exact().unwrap().key(),
        &ExactTypeKey::Nominal(
            fundamental
                .integer(crate::IntegerKind::SIGNED_32)
                .persistent()
        )
    );
    assert_eq!(
        identities[boolean].exact().unwrap().key(),
        &ExactTypeKey::Nominal(fundamental.boolean().persistent())
    );
    assert_eq!(
        identities[string].exact().unwrap().key(),
        &ExactTypeKey::Nominal(fundamental.string().persistent())
    );
    let mapper = crate::HirSignatureTypeMapper::new(inputs);
    assert_eq!(
        mapper.map(string, &[]).unwrap(),
        SignatureTypeKey::Nominal(fundamental.string().persistent())
    );

    let mut concrete_types = Arena::new();
    let concrete_integer = concrete_types.alloc(crate::concrete::Type {
        kind: crate::concrete::TypeKind::Integer(crate::IntegerKind::SIGNED_32),
        gc_free: true,
    });
    let concrete_boolean = concrete_types.alloc(crate::concrete::Type {
        kind: crate::concrete::TypeKind::Boolean,
        gc_free: true,
    });
    let concrete_string = concrete_types.alloc(crate::concrete::Type {
        kind: crate::concrete::TypeKind::String,
        gc_free: false,
    });
    let concrete_identities = crate::concrete::ExactTypeIdentities::from_types(
        crate::concrete::ExactTypeIdentityInputs {
            types: &concrete_types,
            function_types: &Arena::new(),
            structs: &Arena::new(),
            enums: &Arena::new(),
            classes: &Arena::new(),
            interfaces: &Arena::new(),
            objects: &Arena::new(),
            core_types: crate::concrete::ConcreteCoreTypeIdentityAuthority::Imported(fundamental),
        },
    )
    .unwrap();
    assert_eq!(
        concrete_identities[concrete_integer].key(),
        &ExactTypeKey::Nominal(
            fundamental
                .integer(crate::IntegerKind::SIGNED_32)
                .persistent()
        )
    );
    assert_eq!(
        concrete_identities[concrete_boolean].key(),
        &ExactTypeKey::Nominal(fundamental.boolean().persistent())
    );
    assert_eq!(
        concrete_identities[concrete_string].key(),
        &ExactTypeKey::Nominal(fundamental.string().persistent())
    );
}

#[test]
fn imported_protocols_do_not_require_duplicate_public_bindings() {
    let fixture = fixture();
    let mut foundation = fixture.foundation.clone();
    foundation.set_export_bindings(Vec::new()).unwrap();
    let imported = imported_foundation(&foundation);
    assert!(imported.import_core_inputs(&fixture.interface).is_ok());
}

fn decode_section(
    section: &CoreBootstrapInterfaceSectionV1,
) -> DecodedCoreBootstrapInterfaceSectionV1 {
    decode_canonical(&encode(section).unwrap()).unwrap()
}

fn non_core_section() -> CoreBootstrapInterfaceSectionV1 {
    CoreBootstrapInterfaceSectionV1 {
        compiler_protocols: None,
        output_contract: HirOutputContractV1::Library,
        direct_public_surface: CanonicalDirectPublicSurfaceV1::try_new(Vec::new()).unwrap(),
    }
}

struct Fixture {
    foundation: CanonicalHirFoundation,
    direct: CanonicalDirectPublicSurfaceV1,
    interface: CoreCompilerProtocolSurfaceV1,
    section: CoreBootstrapInterfaceSectionV1,
}

fn fixture() -> Fixture {
    let string_key = nominal("String", SourceNominalKind::Class, 0);
    let option_key = nominal("Option", SourceNominalKind::Enum, 1);
    let string: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> =
        CborIdentityRecord::from_key(string_key).unwrap();
    let option: CborIdentityRecord<PersistentGenericTypeId, SourceDeclarationKey> =
        CborIdentityRecord::from_key(option_key).unwrap();
    let string_id = string.id();

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
    let mut foundation = CanonicalHirFoundation::empty();
    let compiler_protocols = crate::production::core_protocol_test_support::install(
        &mut foundation,
        crate::production::core_protocol_test_support::ExistingProtocolFixture {
            string: string.clone(),
            option: option.clone(),
            option_some: some.clone(),
            option_some_payload: some_payload.clone(),
            option_none: none.clone(),
            exact_types: vec![string_exact_record.clone(), unit_exact_record()],
        },
    );
    let interface = compiler_protocols;
    let section = CoreBootstrapInterfaceSectionV1 {
        compiler_protocols: Some(Box::new(interface.clone())),
        output_contract: HirOutputContractV1::Library,
        direct_public_surface: direct.clone(),
    };

    foundation
        .set_export_bindings(vec![string_binding, option_binding])
        .unwrap();

    Fixture {
        foundation,
        direct,
        interface,
        section,
    }
}

fn imported_foundation(foundation: &CanonicalHirFoundation) -> ImportedHirFoundation {
    imported_foundation_at(foundation, ConeIdentity::CORE)
}

fn imported_foundation_at(
    foundation: &CanonicalHirFoundation,
    provider: ConeIdentity,
) -> ImportedHirFoundation {
    let decoded: DecodedHirFoundation = decode_canonical(&encode(foundation).unwrap()).unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    if provider != ConeIdentity::CORE {
        pending.register_authority(provider).unwrap();
    }
    decoded.register_identities(&mut pending).unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    let identities = pending.finish().unwrap();
    let mut session = SemanticIdentitySession::new();
    let imported = session
        .import(
            provider,
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
