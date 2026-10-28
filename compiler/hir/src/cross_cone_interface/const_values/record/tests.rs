use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOrigin,
    DefinitionOwnerChain, IdentityReferenceError, NormalizedSourcePath, PackagePath,
    PendingIdentityValidation, PersistentPropertyId, PersistentSourceContextId, PersistentTypeId,
    SignatureTypeKey, SourceContextKey, SourceDeclarationKey, SourceDeclarationSite,
    SourceIdentity, SourceNominalKind, SourceSpan, ValidatedIdentityGraph,
};
use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::*;
use crate::{CanonicalBooleanV1, CanonicalConstValueV1, ExportDefinitionSourceV1};

#[test]
fn record_has_fixed_wire_and_exposes_all_fields() {
    let fixture = Fixture::new();
    let record = fixture.record();
    let expected = [
        b"\xa4\x01".as_slice(),
        encode(&record.property()).unwrap().as_slice(),
        b"\x02".as_slice(),
        encode(record.value_type()).unwrap().as_slice(),
        b"\x03".as_slice(),
        encode(record.value()).unwrap().as_slice(),
        b"\x04".as_slice(),
        encode(record.definition_origin()).unwrap().as_slice(),
    ]
    .concat();

    assert_eq!(encode(&record).unwrap(), expected);
    assert_eq!(record.property(), fixture.property.id());
    assert_eq!(
        record.value_type(),
        &SignatureTypeKey::Nominal(fixture.value_type.id())
    );
    assert_eq!(
        record.value(),
        &super::CanonicalValueConstantV1::Scalar(CanonicalConstValueV1::Boolean(
            CanonicalBooleanV1::True
        ))
    );
    assert_eq!(record.definition_origin(), &fixture.origin);
}

#[test]
fn decoded_record_resolves_every_typed_reference() {
    let fixture = Fixture::new();
    let record = fixture.record();

    assert_eq!(
        decode_record(&record)
            .resolve(&mut fixture.authority(true, true, true))
            .unwrap(),
        record
    );
}

#[test]
fn aggregate_const_wire_preserves_nested_values_and_resolves_variant_identity() {
    use crate::CanonicalValueConstantV1 as C;
    use scoop_identity::{EnumVariantIdentityKey, PersistentEnumVariantId};
    let enumeration = CborIdentityRecord::<PersistentTypeId, _>::from_key(
        SourceDeclarationKey::nominal(site(), identifier("Choice"), SourceNominalKind::Enum, 0),
    )
    .unwrap();
    let variant = CborIdentityRecord::<PersistentEnumVariantId, _>::from_key(
        EnumVariantIdentityKey::source(enumeration.key(), identifier("Value")).unwrap(),
    )
    .unwrap();
    let value = C::Tuple(vec![
        C::Unit,
        C::Struct(vec![C::Scalar(CanonicalConstValueV1::String(
            "embedded".into(),
        ))]),
        C::Enum {
            variant: variant.id(),
            fields: vec![C::Unit],
        },
    ]);
    let bytes = encode(&value).unwrap();
    let decoded: crate::DecodedValueConstantV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    pending
        .register_external_canonical_authority(enumeration)
        .unwrap();
    let mut missing = pending.finish().unwrap();
    assert!(decoded.clone().resolve(&mut missing).is_err());
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    pending
        .register_external_canonical_authority(variant)
        .unwrap();
    assert_eq!(
        decoded.resolve(&mut pending.finish().unwrap()).unwrap(),
        value
    );
    assert!(
        decode_canonical::<crate::DecodedValueConstantV1>(&[0xa2, 0x00, 0x06, 0x01, 0x80]).is_err()
    );
}

#[test]
fn reader_requires_the_exact_four_field_shape() {
    let error = decode_canonical::<DecodedExportConstValueV1>(&[0xa0]).unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 4,
            actual: 0,
        }
    );
}

#[test]
fn resolution_reports_each_missing_typed_authority() {
    let fixture = Fixture::new();
    assert!(matches!(
        decode_record(&fixture.record()).resolve(&mut fixture.authority(false, true, true)),
        Err(ExportConstValueResolutionError::Property(
            IdentityReferenceError::Missing { .. }
        ))
    ));
    assert!(matches!(
        decode_record(&fixture.record()).resolve(&mut fixture.authority(true, false, true)),
        Err(ExportConstValueResolutionError::ValueType(
            IdentityReferenceError::Missing { .. }
        ))
    ));
    assert!(matches!(
        decode_record(&fixture.record()).resolve(&mut fixture.authority(true, true, false)),
        Err(ExportConstValueResolutionError::DefinitionOrigin(_))
    ));
}

struct Fixture {
    property: CborIdentityRecord<PersistentPropertyId, SourceDeclarationKey>,
    value_type: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    context: CborIdentityRecord<PersistentSourceContextId, SourceContextKey>,
    origin: ExportDefinitionSourceV1,
}

impl Fixture {
    fn new() -> Self {
        let source = SourceIdentity::new(
            ConeIdentity::CORE,
            NormalizedSourcePath::new("src/Constants.scoop").unwrap(),
        );
        let context = CborIdentityRecord::from_key(SourceContextKey::File {
            source: source.clone(),
        })
        .unwrap();
        let origin = ExportDefinitionSourceV1::new(
            DefinitionOrigin::new(source, SourceSpan::new(0, 12).unwrap(), context.key()).unwrap(),
        );
        Self {
            property: CborIdentityRecord::from_key(SourceDeclarationKey::property(
                site(),
                identifier("Answer"),
            ))
            .unwrap(),
            value_type: CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
                site(),
                identifier("Boolean"),
                SourceNominalKind::Struct,
                0,
            ))
            .unwrap(),
            context,
            origin,
        }
    }

    fn record(&self) -> ExportConstValueV1 {
        ExportConstValueV1::new(
            self.property.id(),
            SignatureTypeKey::Nominal(self.value_type.id()),
            CanonicalConstValueV1::Boolean(CanonicalBooleanV1::True),
            self.origin.clone(),
        )
    }

    fn authority(
        &self,
        include_property: bool,
        include_value_type: bool,
        include_context: bool,
    ) -> ValidatedIdentityGraph {
        let mut pending = PendingIdentityValidation::new();
        pending.register_authority(ConeIdentity::CORE).unwrap();
        if include_property {
            pending
                .register_external_canonical_authority(self.property.clone())
                .unwrap();
        }
        if include_value_type {
            pending
                .register_external_canonical_authority(self.value_type.clone())
                .unwrap();
        }
        if include_context {
            pending
                .register_external_canonical_authority(self.context.clone())
                .unwrap();
        }
        pending.finish().unwrap()
    }
}

fn decode_record(record: &ExportConstValueV1) -> DecodedExportConstValueV1 {
    decode_canonical(&encode(record).unwrap()).unwrap()
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

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}
