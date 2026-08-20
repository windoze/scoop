use super::*;

fn local_records(fixture: &Fixture) -> CanonicalTemplateLocalTableV1 {
    CanonicalTemplateLocalTableV1::try_new(vec![
        TemplateLocalRecordV1::try_new(
            LocalValueSelector::Parameter {
                declaration_index: 0,
            },
            SignatureTypeKey::Nominal(fixture.type_id),
            CanonicalBooleanV1::False,
            TemplateLocalDefinitionV1::Source(origin_at("src/Local.scoop", 41)),
        )
        .unwrap(),
        TemplateLocalRecordV1::try_new(
            LocalValueSelector::Synthetic {
                path: definition_path(),
                role: scoop_identity::SyntheticLocalRole::Temporary,
            },
            SignatureTypeKey::Nominal(fixture.type_id),
            CanonicalBooleanV1::False,
            TemplateLocalDefinitionV1::Synthetic,
        )
        .unwrap(),
    ])
    .unwrap()
}

#[test]
fn metadata_only_references_keep_source_origin_without_expression_uses() {
    let fixture = Fixture::new();
    let origin = fixture.origin();
    let body = body(
        DefaultExpressionKindV1::UnitLiteral,
        binder(),
        origin.clone(),
    );
    let records = collect(&body, &local_records(&fixture), &origin);
    assert_eq!(records.local_indices, [0, 1]);
    assert_eq!(
        records.references,
        [
            (
                ExportDefaultReferenceKindV1::Type,
                None,
                origin_at("src/Local.scoop", 41)
            ),
            (ExportDefaultReferenceKindV1::Type, None, origin),
        ]
    );
}

#[derive(Debug, Eq, PartialEq)]
enum Rejection {
    Metadata,
    Resource(WireError),
}
impl From<WireError> for Rejection {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
struct RejectMetadata;
impl<'body> Visitor<'body> for RejectMetadata {
    type Error = Rejection;
    fn expression(
        &mut self,
        _: u32,
        _: &DefaultExpressionV1,

        _: &WirePath,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    fn reference(
        &mut self,
        occurrence: Occurrence<'body>,

        _: &WirePath,
    ) -> Result<(), Self::Error> {
        if matches!(occurrence.attachment, Attachment::Metadata(_)) {
            Err(Rejection::Metadata)
        } else {
            Ok(())
        }
    }
}

#[test]
fn metadata_only_target_validation_is_not_short_circuited_by_absent_expression_uses() {
    let fixture = Fixture::new();
    let body = body(
        DefaultExpressionKindV1::UnitLiteral,
        binder(),
        fixture.origin(),
    );
    let error = body
        .visit_direct_references(
            &local_records(&fixture),
            &fixture.origin(),
            &mut RejectMetadata,
            &WirePath::root(),
        )
        .unwrap_err();
    assert_eq!(error, Rejection::Metadata);
}
