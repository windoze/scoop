use super::*;
use crate::{
    DefaultAppliedOptionV1, DefaultBindingActionV1, DefaultBindingPlanV1,
    DefaultBindingProjectionV1, DefaultBindingShapeV1, DefaultBindingTemporaryV1,
    DefaultEnumVariantFieldRefV1, DefaultEnumVariantRefV1, DefaultForIterationPlanV1,
    DefaultIteratorConformanceV1, DefaultIteratorNextV1,
};

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
        _: &mut BudgetMeter,
        _: &WirePath,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    fn reference(
        &mut self,
        occurrence: Occurrence<'body>,
        _: &mut BudgetMeter,
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
            &mut BudgetMeter::new(DecodeLimits::default()),
            &WirePath::root(),
        )
        .unwrap_err();
    assert_eq!(error, Rejection::Metadata);
}

#[test]
fn iterator_and_binding_metadata_keep_the_actual_receiver_plans() {
    let fixture = Fixture::new();
    let origin = fixture.origin();
    let source_type = SignatureTypeKey::Nominal(fixture.type_id);
    let iterator_type = SignatureTypeKey::RawPointer(Box::new(source_type.clone()));
    let source = DefaultBindingTemporaryV1::new(fixture.local(), source_type.clone());
    let iterator = DefaultBindingTemporaryV1::new(fixture.local(), iterator_type.clone());
    let result = DefaultBindingTemporaryV1::new(fixture.local(), binder());
    let action = DefaultBindingActionV1::project(
        source.clone(),
        result.clone(),
        DefaultBindingProjectionV1::struct_field(fixture.field, source_type.clone()),
        origin.clone(),
    );
    let binding = DefaultBindingPlanV1::try_new(
        result.clone(),
        DefaultBindingShapeV1::wildcard(),
        vec![action],
    )
    .unwrap();
    let option = DefaultAppliedOptionV1::new(
        DefaultEnumVariantFieldRefV1::new(fixture.variant_field, fixture.value_type()),
        DefaultEnumVariantRefV1::new(fixture.variant, fixture.value_type()),
    );
    let next = DefaultIteratorNextV1::new(
        fixture.callable(),
        result.clone(),
        option,
        result,
        origin.clone(),
    );
    let unit = || {
        expression(
            DefaultExpressionKindV1::UnitLiteral,
            binder(),
            origin.clone(),
        )
    };
    let plan = DefaultForIterationPlanV1::try_new(
        Vec::new(),
        source.clone(),
        unit(),
        Vec::new(),
        unit(),
        DefaultIteratorConformanceV1::new(source, iterator, fixture.value_type(), origin.clone()),
        next,
        binding,
        Vec::new(),
    )
    .unwrap();
    let statement =
        DefaultStatementV1::try_new(DefaultStatementKindV1::For(Box::new(plan)), origin.clone())
            .unwrap();
    let body = ExportDefaultBodyV1::try_new(vec![statement], unit()).unwrap();
    let records = collect(
        &body,
        &CanonicalTemplateLocalTableV1::try_new(Vec::new()).unwrap(),
        &origin,
    );
    assert_eq!(records.iterator_receivers, [iterator_type]);
    assert_eq!(records.binding_receivers, [source_type]);
    assert!(
        records
            .references
            .iter()
            .filter(|(kind, _, _)| matches!(
                kind,
                ExportDefaultReferenceKindV1::Callable | ExportDefaultReferenceKindV1::Field
            ))
            .all(|(_, index, _)| index.is_none())
    );
    assert_eq!(records.expressions, [0, 1, 2]);
}
