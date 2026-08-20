use scoop_identity::{
    CallableTemplateOrigin, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};
use scoop_wire::{Encoder, WireEncode, decode_canonical, encode};

use super::super::body::expression_test_support::{Fixture, ResolutionError, Resolver};
use super::*;
use crate::{
    CanonicalBinderUseListV1, CanonicalBooleanV1, CanonicalTemplateLocalTableV1,
    CanonicalTemplateValueParametersV1, DefaultExpressionKindV1, DefaultExpressionV1,
    ExportDefaultBodyV1, ExportDefaultReferenceSetV1, OptionalTemplateReceiverV1,
    PersistentLexicalRootV1,
};

#[test]
fn producer_sorts_rejects_duplicates_and_supports_bidirectional_lookup() {
    let fixture = Fixture::new();
    let first = template(&fixture, 0);
    let second = template(&fixture, 1);
    let mut table =
        CanonicalExportDefaultTemplatesV1::try_new(vec![second.clone(), first.clone()]).unwrap();

    assert_eq!(table.records(), &[first.clone(), second.clone()]);
    assert_eq!(table.len_u32(), 2);
    assert!(!table.is_empty());
    assert_eq!(table.get(first.key()), Some(&first));
    assert_eq!(table.index_of(second.key()), Some(1));
    assert_eq!(table.resolve_default_template_key(0), Ok(first.key()));
    assert_eq!(table.resolve_default_template_index(second.key()), Ok(1));
    assert_eq!(
        table.resolve_default_template_key(2),
        Err(ExportDefaultTemplateLookupError::IndexOutOfRange { index: 2, len: 2 })
    );

    let missing = ExportDefaultTemplateKeyV1::new(first.key().owner(), 9);
    assert_eq!(
        table.resolve_default_template_index(missing),
        Err(ExportDefaultTemplateLookupError::MissingKey(missing))
    );
    assert_eq!(
        CanonicalExportDefaultTemplatesV1::try_new(vec![first.clone(), first]),
        Err(ExportDefaultTemplateSetBuildError::DuplicateKey(
            ExportDefaultTemplateKeyV1::new(CallableTemplateOrigin::Function(fixture.function), 0,)
        ))
    );
}

#[test]
fn indexed_table_has_canonical_wire_and_round_trips() {
    let fixture = Fixture::new();
    let first = template(&fixture, 0);
    let second = template(&fixture, 1);
    let table =
        CanonicalExportDefaultTemplatesV1::try_new(vec![second.clone(), first.clone()]).unwrap();

    let bytes = encode(&table.index_locals().unwrap()).unwrap();
    let first_bytes = encode(&first.index_locals().unwrap()).unwrap();
    let second_bytes = encode(&second.index_locals().unwrap()).unwrap();
    assert_eq!(
        bytes,
        [
            b"\x82".as_slice(),
            first_bytes.as_slice(),
            second_bytes.as_slice(),
        ]
        .concat()
    );

    let decoded: DecodedCanonicalExportDefaultTemplatesV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(decoded.resolve(&mut fixture.resolver()).unwrap(), table);
}

#[test]
fn reader_rejects_duplicate_and_noncanonical_key_order() {
    let fixture = Fixture::new();
    let first = template(&fixture, 0);
    let second = template(&fixture, 1);

    let duplicate = decode_table(&TemplateSequence(vec![first.clone(), first.clone()]));
    assert!(matches!(
        duplicate.resolve(&mut fixture.resolver()),
        Err(ExportDefaultTemplateSetValidationError::DuplicateKey {
            index: 1,
            key,
        }) if key == first.key()
    ));

    let reversed = decode_table(&TemplateSequence(vec![second, first]));
    assert!(matches!(
        reversed.resolve(&mut fixture.resolver()),
        Err(ExportDefaultTemplateSetValidationError::NonCanonicalOrder { index: 1 })
    ));
}

#[test]
fn table_preserves_record_resolution_location() {
    let fixture = Fixture::new();
    let decoded = decode_table(&TemplateSequence(vec![template(&fixture, 0)]));

    assert!(matches!(
        decoded.resolve(&mut Resolver::rejecting()),
        Err(ExportDefaultTemplateSetValidationError::Record {
            index: 0,
            error: ExportDefaultTemplateResolutionError::Key(ResolutionError),
        })
    ));
}

#[test]
fn empty_table_uses_the_canonical_empty_array() {
    let table = CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap();
    assert_eq!(encode(&table.index_locals().unwrap()).unwrap(), vec![0x80]);

    let decoded: DecodedCanonicalExportDefaultTemplatesV1 = decode_canonical(&[0x80]).unwrap();
    assert_eq!(
        decoded.resolve(&mut Fixture::new().resolver()).unwrap(),
        table
    );
}

pub(super) fn template(fixture: &Fixture, position: u32) -> ExportDefaultTemplateV1 {
    let result = fixture.value_type();
    let body = ExportDefaultBodyV1::try_new(
        Vec::new(),
        DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::UnitLiteral,
            result.clone(),
            fixture.origin(),
            scoop_identity::EvaluationOrigin::at_definition(fixture.origin().origin()),
        )
        .unwrap(),
    )
    .unwrap();
    ExportDefaultTemplateV1::try_new(
        ExportDefaultTemplateKeyV1::new(
            CallableTemplateOrigin::Function(fixture.function),
            position,
        ),
        PersistentLexicalRootV1::Function(fixture.function),
        StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, position),
            [],
        ),
        CanonicalTemplateLocalTableV1::try_new(Vec::new()).unwrap(),
        body,
        result,
        CanonicalBooleanV1::False,
        CanonicalBinderUseListV1::try_new(Vec::new()).unwrap(),
        OptionalTemplateReceiverV1::Absent,
        CanonicalTemplateValueParametersV1::try_new(Vec::new()).unwrap(),
        ExportDefaultReferenceSetV1::default(),
        fixture.origin(),
    )
    .unwrap()
}

struct TemplateSequence(Vec<ExportDefaultTemplateV1>);

impl WireEncode for TemplateSequence {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for record in &self.0 {
            record.index_locals().unwrap().encode(encoder)?;
        }
        Ok(())
    }
}

fn decode_table(value: &impl WireEncode) -> DecodedCanonicalExportDefaultTemplatesV1 {
    decode_canonical(&encode(value).unwrap()).unwrap()
}
