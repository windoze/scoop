use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeCoordinate, ConeIdentity, DeclarationScope,
    DefinitionOrigin, DefinitionOriginRecord, DefinitionOriginSubject, DefinitionOwnerChain,
    NormalizedSourcePath, PackagePath, PendingIdentityValidation, SourceContextKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceIdentity, SourceNominalKind, SourceSpan,
};
use scoop_wire::{BudgetMeter, DecodeLimits, decode_canonical, encode};

use super::*;
use crate::{NativeBoundaryCLayoutPolicy, NativeBoundaryNominalShape};

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

struct Fixture {
    coordinate: ConeCoordinate,
    canonical: CanonicalHirFoundation,
    subject: DefinitionOriginSubject,
}

fn fixture(include_origin: bool, include_points: bool) -> Fixture {
    let coordinate = ConeCoordinate::new("example", "foundation", "0.1.0").unwrap();
    fixture_at(coordinate, include_origin, include_points)
}

fn fixture_at(coordinate: ConeCoordinate, include_origin: bool, include_points: bool) -> Fixture {
    let cone = coordinate.identity().unwrap();
    let declaration = declaration(cone, "Widget");
    let record = CborIdentityRecord::from_key(declaration.clone()).unwrap();
    let subject = DefinitionOriginSubject::Type(record.id());
    let source =
        SourceIdentity::new(cone, NormalizedSourcePath::new("src/Widget.scoop").unwrap()).unwrap();
    let context = CborIdentityRecord::from_key(SourceContextKey::File {
        source: source.clone(),
    })
    .unwrap();
    let span = SourceSpan::new(0, 6).unwrap();
    let origin = DefinitionOrigin::new(source.clone(), span, context.key()).unwrap();
    let source_record = SourceRecord::from_utf8(
        source,
        "struct Widget",
        include_points
            .then_some([span.start_byte(), span.end_byte()])
            .into_iter()
            .flatten(),
    )
    .unwrap();
    let boundary = NativeBoundaryTypeDefinitionRecord::new(
        &declaration,
        &[0],
        NativeBoundaryNominalShape::Struct {
            c_layout: NativeBoundaryCLayoutPolicy::NotCLayout,
            fields: Vec::new(),
        },
    )
    .unwrap();

    let mut canonical = CanonicalHirFoundation::empty();
    canonical.set_sources(vec![source_record]).unwrap();
    canonical
        .set_types(vec![
            CoreBuiltinNominal::Unit.identity_record(),
            CoreBuiltinNominal::Any.identity_record(),
            record,
        ])
        .unwrap();
    canonical.set_source_contexts(vec![context]).unwrap();
    if include_origin {
        canonical
            .set_definition_origins(vec![DefinitionOriginRecord::new(subject, origin)])
            .unwrap();
    }
    canonical.set_native_boundary_types(vec![boundary]).unwrap();
    Fixture {
        coordinate,
        canonical,
        subject,
    }
}

fn declaration(cone: ConeIdentity, name: &str) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            cone,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Struct,
        0,
    )
}

fn decode(canonical: &CanonicalHirFoundation) -> DecodedHirFoundation {
    decode_canonical(&encode(canonical).unwrap(), DecodeLimits::default()).unwrap()
}

fn validate_identities(
    decoded: &DecodedHirFoundation,
    authorities: impl IntoIterator<Item = ConeIdentity>,
) -> ValidatedIdentityGraph {
    let mut pending = PendingIdentityValidation::new();
    for authority in authorities {
        pending.register_authority(authority).unwrap();
    }
    decoded.register_identities(&mut pending).unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    pending.finish().unwrap()
}

fn validate_origins_only(
    fixture: &Fixture,
    meter: &mut BudgetMeter,
) -> Result<(), HirFoundationValidationError> {
    let foundation = &fixture.canonical;
    origins::validate(
        fixture.coordinate.identity().unwrap(),
        &foundation.sources,
        &foundation.types,
        &foundation.generic_types,
        &foundation.functions,
        &foundation.generic_functions,
        &foundation.constructors,
        &foundation.properties,
        &foundation.extension_properties,
        &foundation.type_aliases,
        &foundation.property_accessors,
        &foundation.fields,
        &foundation.enum_variants,
        &foundation.enum_variant_fields,
        &foundation.generated_callables,
        &foundation.initialization_units,
        &foundation.local_bindings,
        &foundation.local_values,
        &foundation.callback_registrations,
        &foundation.source_native_contracts,
        &foundation.definition_origins,
        meter,
    )
}

#[test]
fn validates_the_complete_hir_foundation_atomically() {
    let fixture = fixture(true, true);
    let bytes = encode(&fixture.canonical).unwrap();
    let decoded = decode(&fixture.canonical);
    let mut identities = validate_identities(
        &decoded,
        [ConeIdentity::CORE, fixture.coordinate.identity().unwrap()],
    );

    let validated = decoded
        .validate(&fixture.coordinate, &mut identities, &mut meter())
        .unwrap();

    assert_eq!(encode(&validated).unwrap(), bytes);
    assert_eq!(validated.artifact(), fixture.coordinate.identity().unwrap());
    assert_eq!(validated.counts().types, 3);
    assert_eq!(validated.counts().definition_origins, 1);
    assert_eq!(validated.counts().native_boundary_types, 1);
}

#[test]
fn source_context_index_has_inclusive_heap_boundaries() {
    let fixture = fixture(true, true);
    let required = scoop_wire::budget::COLLECTION_ELEMENT_BYTES;

    for (limit, accepted) in [
        (required - 1, false),
        (required, true),
        (required + 1, true),
    ] {
        let mut meter = BudgetMeter::new(DecodeLimits {
            logical_heap_bytes: limit,
            ..DecodeLimits::default()
        });
        let result = validate_source_contexts(
            &fixture.canonical.source_contexts,
            &fixture.canonical.sources,
            &mut meter,
        );
        assert_eq!(result.is_ok(), accepted);
        if !accepted {
            assert!(matches!(
                result.unwrap_err(),
                HirFoundationValidationError::Resource(ref error)
                    if error.kind() == &scoop_wire::WireErrorKind::LimitExceeded {
                        resource: scoop_wire::ResourceKind::LogicalHeapBytes,
                        limit,
                        observed: required,
                    }
            ));
        }
    }
}

#[test]
fn absent_source_diagnostic_copy_has_inclusive_owned_byte_boundaries() {
    let fixture = fixture(true, true);
    let required = u64::try_from(
        fixture.canonical.source_contexts[0]
            .key()
            .source()
            .logical_path()
            .as_str()
            .len(),
    )
    .unwrap();

    for (limit, accepted) in [
        (required - 1, false),
        (required, true),
        (required + 1, true),
    ] {
        let mut meter = BudgetMeter::new(DecodeLimits {
            owned_bytes: limit,
            ..DecodeLimits::default()
        });
        let error = validate_source_contexts(&fixture.canonical.source_contexts, &[], &mut meter)
            .unwrap_err();

        if accepted {
            assert!(matches!(
                error,
                HirFoundationValidationError::UnknownContextSource { .. }
            ));
            assert_eq!(meter.usage().owned_bytes, required);
        } else {
            assert!(matches!(
                error,
                HirFoundationValidationError::Resource(ref error)
                    if error.kind() == &scoop_wire::WireErrorKind::LimitExceeded {
                        resource: scoop_wire::ResourceKind::OwnedBytes,
                        limit,
                        observed: required,
                    }
            ));
        }
    }
}

#[test]
fn definition_origin_indexes_have_inclusive_heap_boundaries() {
    let fixture = fixture(true, true);
    let required = 3 * scoop_wire::budget::COLLECTION_ELEMENT_BYTES;

    for (limit, accepted) in [
        (required - 1, false),
        (required, true),
        (required + 1, true),
    ] {
        let mut meter = BudgetMeter::new(DecodeLimits {
            logical_heap_bytes: limit,
            ..DecodeLimits::default()
        });
        let result = validate_origins_only(&fixture, &mut meter);
        assert_eq!(result.is_ok(), accepted);
        if !accepted {
            assert!(matches!(
                result.unwrap_err(),
                HirFoundationValidationError::Resource(ref error)
                    if error.kind() == &scoop_wire::WireErrorKind::LimitExceeded {
                        resource: scoop_wire::ResourceKind::LogicalHeapBytes,
                        limit,
                        observed: required,
                    }
            ));
        }
    }
}

#[test]
fn rejects_a_missing_required_definition_origin() {
    let fixture = fixture(false, true);
    let decoded = decode(&fixture.canonical);
    let mut identities = validate_identities(
        &decoded,
        [ConeIdentity::CORE, fixture.coordinate.identity().unwrap()],
    );

    assert!(matches!(
        decoded.validate(&fixture.coordinate, &mut identities, &mut meter()),
        Err(HirFoundationValidationError::Origin(
            DefinitionOriginValidationError::MissingSubject { subject }
        )) if subject == fixture.subject
    ));
}

#[test]
fn source_declared_core_nominals_require_definition_origins() {
    let complete = fixture_at(ConeCoordinate::reserved_core(), true, true);
    validate_origins_only(&complete, &mut meter()).unwrap();

    let missing = fixture_at(ConeCoordinate::reserved_core(), false, true);
    assert!(matches!(
        validate_origins_only(&missing, &mut meter()),
        Err(HirFoundationValidationError::Origin(
            DefinitionOriginValidationError::MissingSubject { subject }
        )) if subject == missing.subject
    ));
}

#[test]
fn rejects_an_origin_endpoint_absent_from_the_source_point_table() {
    let fixture = fixture(true, false);
    let decoded = decode(&fixture.canonical);
    let mut identities = validate_identities(
        &decoded,
        [ConeIdentity::CORE, fixture.coordinate.identity().unwrap()],
    );

    assert!(matches!(
        decoded.validate(&fixture.coordinate, &mut identities, &mut meter()),
        Err(HirFoundationValidationError::Origin(
            DefinitionOriginValidationError::MissingPoint {
                subject,
                byte_offset: 0,
                ..
            }
        )) if subject == fixture.subject
    ));
}

#[test]
fn missing_point_diagnostic_copy_has_inclusive_owned_byte_boundaries() {
    let fixture = fixture(true, false);
    let required = u64::try_from(
        fixture.canonical.sources[0]
            .identity()
            .logical_path()
            .as_str()
            .len(),
    )
    .unwrap();

    for (limit, accepted) in [
        (required - 1, false),
        (required, true),
        (required + 1, true),
    ] {
        let mut meter = BudgetMeter::new(DecodeLimits {
            owned_bytes: limit,
            ..DecodeLimits::default()
        });
        let error = validate_origins_only(&fixture, &mut meter).unwrap_err();

        if accepted {
            assert!(matches!(
                error,
                HirFoundationValidationError::Origin(
                    DefinitionOriginValidationError::MissingPoint { .. }
                )
            ));
            assert_eq!(meter.usage().owned_bytes, required);
        } else {
            assert!(matches!(
                error,
                HirFoundationValidationError::Resource(ref error)
                    if error.kind() == &scoop_wire::WireErrorKind::LimitExceeded {
                        resource: scoop_wire::ResourceKind::OwnedBytes,
                        limit,
                        observed: required,
                    }
            ));
        }
    }
}

#[test]
fn rejects_semantically_noncanonical_table_order() {
    let fixture = fixture(true, true);
    let mut decoded = decode(&fixture.canonical);
    decoded.decoded.types.swap(0, 1);
    let mut identities = validate_identities(
        &decoded,
        [ConeIdentity::CORE, fixture.coordinate.identity().unwrap()],
    );

    assert!(matches!(
        decoded.validate(&fixture.coordinate, &mut identities, &mut meter()),
        Err(HirFoundationValidationError::NonCanonicalFoundation)
    ));
}

#[test]
fn rejects_a_foreign_source_declaration_in_the_artifact_delta() {
    let coordinate = ConeCoordinate::new("example", "current", "0.1.0").unwrap();
    let foreign = ConeCoordinate::new("example", "foreign", "0.1.0")
        .unwrap()
        .identity()
        .unwrap();
    let record: TypeRecord =
        CborIdentityRecord::from_key(declaration(foreign, "Intruder")).unwrap();
    let identity = *record.id().as_array();
    let mut canonical = CanonicalHirFoundation::empty();
    canonical
        .set_types(vec![
            CoreBuiltinNominal::Unit.identity_record(),
            CoreBuiltinNominal::Any.identity_record(),
            record,
        ])
        .unwrap();
    let decoded = decode(&canonical);
    let mut identities = validate_identities(
        &decoded,
        [ConeIdentity::CORE, coordinate.identity().unwrap(), foreign],
    );

    assert!(matches!(
        decoded.validate(&coordinate, &mut identities, &mut meter()),
        Err(HirFoundationValidationError::ForeignDeclaration {
            table: HirFoundationTable::Type,
            identity: actual,
            actual: actual_cone,
            ..
        }) if actual == identity && actual_cone == foreign
    ));
}

#[test]
fn rejects_a_foundation_without_the_trusted_core_nominals() {
    let coordinate = ConeCoordinate::new("example", "current", "0.1.0").unwrap();
    let cone = coordinate.identity().unwrap();
    let mut canonical = CanonicalHirFoundation::empty();
    canonical
        .set_types(vec![
            CborIdentityRecord::from_key(declaration(cone, "Only")).unwrap(),
        ])
        .unwrap();
    let decoded = decode(&canonical);
    let mut identities = validate_identities(&decoded, [ConeIdentity::CORE, cone]);

    assert!(matches!(
        decoded.validate(&coordinate, &mut identities, &mut meter()),
        Err(HirFoundationValidationError::MissingCoreBuiltin { .. })
    ));
}
