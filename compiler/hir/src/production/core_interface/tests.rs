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
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;
use crate::{DecodedHirFoundation, ImportedHirFoundation, PublicNominalShapeRequirementsV1};

mod wire;

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
fn shape_support_sources_are_derived_from_param_free_source_nominals() {
    let fixture = fixture();
    let foundation = OdrFreeHirFoundation::try_new(fixture.foundation).unwrap();
    let sources = PublicNominalShapeRequirementsV1::from_direct_surface(
        &fixture.direct,
        foundation.as_canonical(),
    )
    .unwrap()
    .source_declarations(foundation.as_canonical())
    .unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(
        PersistentTypeId::from_source_declaration(&sources[0]).unwrap(),
        fixture.interface.string_capability().source_type()
    );
}

#[test]
fn imported_core_inputs_expose_compiler_protocols() {
    let fixture = fixture();
    let imported = imported_foundation(&fixture.foundation);

    let core = imported.import_core_inputs(&fixture.interface).unwrap();
    assert_eq!(core.protocols().fixed_subject_count(), 84);
    assert_eq!(
        core.protocols().compiler_operations().len(),
        crate::intrinsic_function_kinds().len()
    );
    let protocols = core.protocols().clone();
    assert_eq!(
        protocols.fundamental_types().unit().persistent(),
        CoreBuiltinNominal::Unit.identity_record().id()
    );
    assert_eq!(
        protocols.fundamental_types().string().persistent(),
        fixture.interface.string_capability().source_type()
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
    let native_boundary = core.native_boundary_types();
    assert_eq!(native_boundary.records().len(), integers.len() + 2);
    for identity in integers
        .into_iter()
        .chain([protocols.fundamental_types().boolean().persistent()])
    {
        let record = native_boundary
            .records()
            .iter()
            .find(|record| record.owner() == crate::NativeBoundaryNominalOwner::Concrete(identity))
            .unwrap();
        assert_eq!(
            record.shape(),
            &crate::NativeBoundaryNominalShape::Struct {
                c_layout: crate::NativeBoundaryCLayoutPolicy::NotCLayout,
                fields: Vec::new(),
            }
        );
    }
    let string_boundary = native_boundary
        .records()
        .iter()
        .find(|record| {
            record.owner()
                == crate::NativeBoundaryNominalOwner::Concrete(
                    protocols.fundamental_types().string().persistent(),
                )
        })
        .unwrap();
    assert_eq!(
        string_boundary.shape(),
        &crate::NativeBoundaryNominalShape::Reference
    );
    assert_eq!(
        protocols.option().some().persistent(),
        fixture.interface.compiler_protocols().option_some()
    );
    assert_eq!(
        protocols.option().some_payload().persistent(),
        fixture.interface.compiler_protocols().option_some_payload()
    );
    assert_eq!(
        protocols.option().none().persistent(),
        fixture.interface.compiler_protocols().option_none()
    );
    let operations = protocols.compiler_operations();
    assert_eq!(
        operations
            .iter()
            .map(|operation| operation.kind())
            .collect::<Vec<_>>(),
        crate::intrinsic_function_kinds()
    );
    let operation = |kind| {
        operations
            .iter()
            .find(|operation| operation.kind() == kind)
            .unwrap()
            .callable()
    };
    assert_eq!(
        protocols.ffi().ptr_to_ulong(),
        operation(crate::IntrinsicFunctionKind::Pointer(
            crate::PointerIntrinsic::ToULong,
        ))
    );
    assert_eq!(
        protocols.coroutines().start_coroutine(),
        operation(crate::IntrinsicFunctionKind::CoroutineStart)
    );
    assert_eq!(
        protocols.foreign_callbacks().register(),
        operation(crate::IntrinsicFunctionKind::ForeignCallbackRegister)
    );
    assert_eq!(
        protocols.source_location().current(),
        operation(crate::IntrinsicFunctionKind::CurrentSourceLocation)
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
    let interface = CoreHirInterfaceV1 {
        string_capability: RuntimeCoreCapabilityV1::String {
            source_type: string_id,
            exact_type: string_exact_record.id(),
        },
        compiler_protocols,
    };
    let section = CoreBootstrapInterfaceSectionV1 {
        core_interface: CoreHirInterfaceBranchV1::Core(Box::new(interface.clone())),
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
