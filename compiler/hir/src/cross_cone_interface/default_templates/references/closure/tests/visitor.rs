use super::*;
use crate::{
    DefaultBodyReferenceAttachmentV1 as Attachment, DefaultBodyReferenceMetadataV1 as Metadata,
    DefaultBodyReferenceOccurrenceV1 as Occurrence, DefaultBodyReferenceTargetV1 as Target,
    DefaultBodyReferenceVisitorV1 as Visitor, DefaultStatementKindV1, DefaultStatementV1,
};

mod metadata;

#[derive(Default)]
struct Recorder {
    expressions: Vec<u32>,
    references: Vec<(
        ExportDefaultReferenceKindV1,
        Option<u32>,
        ExportDefinitionSourceV1,
    )>,
    local_indices: Vec<usize>,
    iterator_receivers: Vec<SignatureTypeKey>,
    binding_receivers: Vec<SignatureTypeKey>,
}

impl<'body> Visitor<'body> for Recorder {
    type Error = WireError;

    fn expression(
        &mut self,
        index: u32,
        _: &DefaultExpressionV1,
        _: &mut BudgetMeter,
        _: &WirePath,
    ) -> Result<(), Self::Error> {
        self.expressions.push(index);
        Ok(())
    }

    fn reference(
        &mut self,
        occurrence: Occurrence<'body>,
        _: &mut BudgetMeter,
        _: &WirePath,
    ) -> Result<(), Self::Error> {
        let kind = match occurrence.target {
            Target::Callable(_) => ExportDefaultReferenceKindV1::Callable,
            Target::Constructor(_) => ExportDefaultReferenceKindV1::Constructor,
            Target::Type(_) => ExportDefaultReferenceKindV1::Type,
            Target::Global(_) => ExportDefaultReferenceKindV1::Global,
            Target::Singleton(_) => ExportDefaultReferenceKindV1::Singleton,
            Target::Field(_) => ExportDefaultReferenceKindV1::Field,
        };
        let index = match occurrence.attachment {
            Attachment::Expression { index, .. } => Some(index),
            Attachment::Metadata(Metadata::TemplateLocal { index, .. }) => {
                self.local_indices.push(index);
                None
            }
            Attachment::Metadata(Metadata::Iterator(plan)) => {
                if matches!(occurrence.target, Target::Callable(_)) {
                    self.iterator_receivers
                        .push(plan.conformance().iterator().value_type().clone());
                }
                None
            }
            Attachment::Metadata(Metadata::BindingAction(action)) => {
                if matches!(occurrence.target, Target::Field(_)) {
                    let crate::DefaultBindingActionViewV1::Project { source, .. } = action.view()
                    else {
                        panic!("field projection must retain its source action");
                    };
                    self.binding_receivers.push(source.value_type().clone());
                }
                None
            }
            Attachment::Metadata(_) => None,
        };
        self.references
            .push((kind, index, occurrence.definition_origin.clone()));
        Ok(())
    }
}

fn collect(
    body: &ExportDefaultBodyV1,
    locals: &CanonicalTemplateLocalTableV1,
    origin: &ExportDefinitionSourceV1,
) -> Recorder {
    let mut recorder = Recorder::default();
    body.visit_direct_references(
        locals,
        origin,
        &mut recorder,
        &mut BudgetMeter::new(DecodeLimits::default()),
        &WirePath::root(),
    )
    .unwrap();
    recorder
}

#[test]
fn assigns_expression_indices_in_source_order_and_retains_parent_attachment() {
    let fixture = Fixture::new();
    let origin = fixture.origin();
    let value = expression(
        DefaultExpressionKindV1::TupleLiteral(vec![
            expression(
                DefaultExpressionKindV1::FieldAccess {
                    receiver: Box::new(expression(
                        DefaultExpressionKindV1::GlobalRead(fixture.property),
                        binder(),
                        origin.clone(),
                    )),
                    field: DefaultFieldRefV1::Struct {
                        declaration: fixture.field,
                        owner_type: SignatureTypeKey::Nominal(fixture.type_id),
                    },
                },
                binder(),
                origin.clone(),
            ),
            expression(
                DefaultExpressionKindV1::SingletonValue(fixture.object),
                binder(),
                origin.clone(),
            ),
        ]),
        binder(),
        origin.clone(),
    );
    let statement = DefaultStatementV1::try_new(
        DefaultStatementKindV1::Expr(Box::new(expression(
            DefaultExpressionKindV1::UnitLiteral,
            binder(),
            origin.clone(),
        ))),
        origin.clone(),
    )
    .unwrap();
    let body = ExportDefaultBodyV1::try_new(vec![statement], value).unwrap();
    let records = collect(
        &body,
        &CanonicalTemplateLocalTableV1::try_new(Vec::new()).unwrap(),
        &origin,
    );
    assert_eq!(records.expressions, [0, 1, 2, 3, 4]);
    let actual = records
        .references
        .iter()
        .filter(|(kind, _, _)| *kind != ExportDefaultReferenceKindV1::Type)
        .map(|(kind, index, _)| (*kind, *index))
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        [
            (ExportDefaultReferenceKindV1::Global, Some(3)),
            (ExportDefaultReferenceKindV1::Field, Some(2)),
            (ExportDefaultReferenceKindV1::Singleton, Some(4)),
        ]
    );
}

#[test]
fn visitor_sees_binder_only_expressions_without_fabricating_type_references() {
    let fixture = Fixture::new();
    let body = body(
        DefaultExpressionKindV1::UnitLiteral,
        binder(),
        fixture.origin(),
    );
    let records = collect(
        &body,
        &CanonicalTemplateLocalTableV1::try_new(Vec::new()).unwrap(),
        &fixture.origin(),
    );
    assert_eq!(records.expressions, [0]);
    assert!(records.references.is_empty());
}

#[test]
fn resource_limits_stop_the_shared_walk_before_observation() {
    let fixture = Fixture::new();
    let body = body(
        DefaultExpressionKindV1::UnitLiteral,
        fixture.value_type(),
        fixture.origin(),
    );
    let mut recorder = Recorder::default();
    let limits = DecodeLimits {
        decoded_nodes: 0,
        ..DecodeLimits::default()
    };
    let error = body
        .visit_direct_references(
            &CanonicalTemplateLocalTableV1::try_new(Vec::new()).unwrap(),
            &fixture.origin(),
            &mut recorder,
            &mut BudgetMeter::new(limits),
            &WirePath::root(),
        )
        .unwrap_err();
    assert!(matches!(
        error.kind(),
        WireErrorKind::LimitExceeded {
            resource: ResourceKind::DecodedNodes,
            ..
        }
    ));
    assert!(recorder.expressions.is_empty());
    assert!(recorder.references.is_empty());
}

#[test]
fn shared_walk_preserves_structural_recursion_limit() {
    let fixture = Fixture::new();
    let body = body(
        DefaultExpressionKindV1::Box(Box::new(expression(
            DefaultExpressionKindV1::UnitLiteral,
            binder(),
            fixture.origin(),
        ))),
        binder(),
        fixture.origin(),
    );
    let mut recorder = Recorder::default();
    let limits = DecodeLimits {
        semantic_recursion: 2,
        ..DecodeLimits::default()
    };
    let error = body
        .visit_direct_references(
            &CanonicalTemplateLocalTableV1::try_new(Vec::new()).unwrap(),
            &fixture.origin(),
            &mut recorder,
            &mut BudgetMeter::new(limits),
            &WirePath::root(),
        )
        .unwrap_err();
    assert!(matches!(
        error.kind(),
        WireErrorKind::LimitExceeded {
            resource: ResourceKind::SemanticRecursion,
            ..
        }
    ));
}
