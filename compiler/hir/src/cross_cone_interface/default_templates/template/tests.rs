use scoop_identity::{
    CallableTemplateOrigin, SignatureTypeKey, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
};
use scoop_wire::{Encoder, WireEncode, WireErrorKind, decode_canonical, encode};

use super::super::body::expression_test_support::{Fixture, ResolutionError, Resolver};
use super::*;
use crate::{
    DefaultExpressionIndexError, DefaultExpressionKindV1, DefaultExpressionV1,
    TemplateLocalDefinitionV1, TemplateLocalRecordV1, TemplateValueParameterV1,
};

#[test]
fn template_has_exact_indexed_wire_and_round_trips() {
    let fixture = Fixture::new();
    let expected = template_with_parameter(&fixture);

    let bytes = encode(&expected.index_locals().unwrap()).unwrap();
    assert_eq!(bytes[0], 0xac);
    assert_eq!(bytes[1], 0x01);

    let decoded: DecodedExportDefaultTemplateV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(decoded.resolve(&mut fixture.resolver()).unwrap(), expected);
}

#[test]
fn template_rejects_result_type_mismatch() {
    let fixture = Fixture::new();
    let body_type = fixture.value_type();
    let declared = SignatureTypeKey::Binder { depth: 0, index: 1 };

    assert_eq!(
        build_template(
            &fixture,
            unit_body(&fixture, body_type.clone()),
            declared.clone(),
            empty_locals(),
            empty_value_parameters(),
            ExportDefaultReferenceSetV1::default(),
        ),
        Err(ExportDefaultTemplateBuildError::ResultType {
            declared,
            body: body_type,
        })
    );
}

#[test]
fn template_rejects_body_locals_absent_from_canonical_table() {
    let fixture = Fixture::new();
    let value_type = fixture.value_type();
    let body = ExportDefaultBodyV1::try_new(
        Vec::new(),
        DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::Local(fixture.local()),
            value_type.clone(),
            fixture.origin(),
        )
        .unwrap(),
    )
    .unwrap();

    assert_eq!(
        build_template(
            &fixture,
            body,
            value_type,
            empty_locals(),
            empty_value_parameters(),
            ExportDefaultReferenceSetV1::default(),
        ),
        Err(ExportDefaultTemplateBuildError::BodyLocal(
            ExportDefaultBodyIndexError::Value(DefaultExpressionIndexError::Local(
                TemplateLocalLookupError::MissingSelector(fixture.local()),
            )),
        ))
    );
}

#[test]
fn reader_replays_template_record_invariants() {
    let fixture = Fixture::new();
    let template = template_with_parameter(&fixture);
    let declared = SignatureTypeKey::Binder { depth: 0, index: 1 };
    let bytes = encode(&TemplateWire {
        template: &template,
        result: &declared,
    })
    .unwrap();
    let decoded: DecodedExportDefaultTemplateV1 = decode_canonical(&bytes).unwrap();

    assert!(matches!(
        decoded.resolve(&mut fixture.resolver()),
        Err(ExportDefaultTemplateResolutionError::Record(
            ExportDefaultTemplateBuildError::ResultType {
                declared: actual_declared,
                body,
            },
        )) if actual_declared == declared && body == fixture.value_type()
    ));
}

#[test]
fn reader_preserves_outer_resolution_context() {
    let fixture = Fixture::new();
    let bytes = encode(&template_with_parameter(&fixture).index_locals().unwrap()).unwrap();
    let decoded: DecodedExportDefaultTemplateV1 = decode_canonical(&bytes).unwrap();

    assert_eq!(
        decoded.resolve(&mut fixture.resolver()).unwrap(),
        template_with_parameter(&fixture)
    );

    let decoded: DecodedExportDefaultTemplateV1 = decode_canonical(&bytes).unwrap();
    assert!(matches!(
        decoded.resolve(&mut Resolver::rejecting()),
        Err(ExportDefaultTemplateResolutionError::Key(ResolutionError))
    ));
}

#[test]
fn decoder_requires_the_exact_twelve_field_product() {
    let error = decode_canonical::<DecodedExportDefaultTemplateV1>(&[0xa0]).unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 12,
            actual: 0,
        }
    );
}

fn template_with_parameter(fixture: &Fixture) -> ExportDefaultTemplateV1 {
    let value_type = fixture.value_type();
    let local = fixture.local();
    let locals = CanonicalTemplateLocalTableV1::try_new(vec![
        TemplateLocalRecordV1::try_new(
            local.clone(),
            value_type.clone(),
            CanonicalBooleanV1::False,
            TemplateLocalDefinitionV1::Source(fixture.origin()),
        )
        .unwrap(),
    ])
    .unwrap();
    let body = ExportDefaultBodyV1::try_new(
        Vec::new(),
        DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::Local(local.clone()),
            value_type.clone(),
            fixture.origin(),
        )
        .unwrap(),
    )
    .unwrap();
    let value_parameters = CanonicalTemplateValueParametersV1::try_new(vec![
        TemplateValueParameterV1::try_new(0, local).unwrap(),
    ])
    .unwrap();

    build_template(
        fixture,
        body,
        value_type,
        locals,
        value_parameters,
        ExportDefaultReferenceSetV1::default(),
    )
    .unwrap()
}

fn build_template(
    fixture: &Fixture,
    body: ExportDefaultBodyV1,
    result: SignatureTypeKey,
    locals: CanonicalTemplateLocalTableV1,
    value_parameters: CanonicalTemplateValueParametersV1,
    references: ExportDefaultReferenceSetV1,
) -> Result<ExportDefaultTemplateV1, ExportDefaultTemplateBuildError> {
    ExportDefaultTemplateV1::try_new(
        ExportDefaultTemplateKeyV1::new(CallableTemplateOrigin::Function(fixture.function), 0),
        PersistentLexicalRootV1::Function(fixture.function),
        definition_path(),
        locals,
        body,
        result,
        CanonicalBooleanV1::False,
        CanonicalBinderUseListV1::try_new(Vec::new()).unwrap(),
        OptionalTemplateReceiverV1::Absent,
        value_parameters,
        references,
        fixture.origin(),
    )
}

fn unit_body(fixture: &Fixture, result: SignatureTypeKey) -> ExportDefaultBodyV1 {
    ExportDefaultBodyV1::try_new(
        Vec::new(),
        DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::UnitLiteral,
            result,
            fixture.origin(),
        )
        .unwrap(),
    )
    .unwrap()
}

fn empty_locals() -> CanonicalTemplateLocalTableV1 {
    CanonicalTemplateLocalTableV1::try_new(Vec::new()).unwrap()
}

fn empty_value_parameters() -> CanonicalTemplateValueParametersV1 {
    CanonicalTemplateValueParametersV1::try_new(Vec::new()).unwrap()
}

fn definition_path() -> StructuralDefinitionPath {
    StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
        [],
    )
}

struct TemplateWire<'a> {
    template: &'a ExportDefaultTemplateV1,
    result: &'a SignatureTypeKey,
}

impl WireEncode for TemplateWire<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let mut local_index = self.template.locals().clone();
        let body = self.template.body().index_locals(&mut local_index).unwrap();
        let receiver = self
            .template
            .receiver()
            .index_local(&mut local_index)
            .unwrap();
        let value_parameters = self
            .template
            .value_parameters()
            .index_locals(&mut local_index)
            .unwrap();

        encoder.map(12)?;
        encoder.field(1)?;
        self.template.key().encode(encoder)?;
        encoder.field(2)?;
        self.template.definition_root().encode(encoder)?;
        encoder.field(3)?;
        self.template.definition_path().encode(encoder)?;
        encoder.field(4)?;
        self.template.locals().encode(encoder)?;
        encoder.field(5)?;
        body.encode(encoder)?;
        encoder.field(6)?;
        self.result.encode(encoder)?;
        encoder.field(7)?;
        self.template.allows_suspend().encode(encoder)?;
        encoder.field(8)?;
        self.template.type_parameters().encode(encoder)?;
        encoder.field(9)?;
        receiver.encode(encoder)?;
        encoder.field(10)?;
        value_parameters.encode(encoder)?;
        encoder.field(11)?;
        self.template.references().encode(encoder)?;
        encoder.field(12)?;
        self.template.definition_origin().encode(encoder)
    }
}
