use super::*;
pub(super) use crate::cross_cone_interface::expression_test_support::Fixture;

pub(super) fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
pub(super) fn key(f: &Fixture) -> ProtectedDefaultTemplateKeyV1 {
    ProtectedDefaultTemplateKeyV1::try_new(CallableTemplateOrigin::Function(f.function), 0).unwrap()
}
pub(super) fn expression(f: &Fixture, kind: DefaultExpressionKindV1) -> DefaultExpressionV1 {
    DefaultExpressionV1::try_new(kind, f.value_type(), f.origin()).unwrap()
}
pub(super) fn body(f: &Fixture, kind: DefaultExpressionKindV1) -> ExportDefaultBodyV1 {
    ExportDefaultBodyV1::try_new(vec![], expression(f, kind)).unwrap()
}
pub(super) fn empty_locals() -> CanonicalTemplateLocalTableV1 {
    CanonicalTemplateLocalTableV1::try_new(vec![]).unwrap()
}
pub(super) fn reference<T>(
    f: &Fixture,
    target: T,
    uses: Vec<ProtectedDefaultExpressionUseV1>,
) -> ProtectedDefaultReferenceV1<T> {
    let domain = PersistentLookupDomainV1::new(PersistentAccessDomainV1::universal());
    ProtectedDefaultReferenceV1::new(
        target,
        f.origin(),
        ProtectedDefaultAccessWitnessV1::param_free(
            key(f).owner(),
            domain.clone(),
            CanonicalProtectedDefaultSlotCallDomainsV1::try_new(vec![]).unwrap(),
            domain,
        )
        .unwrap(),
        CanonicalProtectedDefaultExpressionUsesV1::try_new(uses).unwrap(),
    )
}
pub(super) fn use_at(
    index: u32,
    receiver: ProtectedDefaultReceiverUseV1,
) -> ProtectedDefaultExpressionUseV1 {
    ProtectedDefaultExpressionUseV1::new(index, receiver)
}
pub(super) fn empty_set() -> ProtectedDefaultReferenceSetV1 {
    ProtectedDefaultReferenceSetV1::try_new(vec![], vec![], vec![], vec![], vec![], vec![]).unwrap()
}
#[derive(Default)]
pub(super) struct Authority {
    pub metadata: usize,
    pub members: Vec<(bool, bool)>,
    pub reject_metadata: bool,
    pub assignments: usize,
}
impl ProtectedDefaultReferenceBodySemanticAuthority<&'static str> for Authority {
    fn validate_default_reference_occurrence(
        &mut self,
        _: ProtectedDefaultTemplateKeyV1,
        occurrence: DefaultBodyReferenceOccurrenceV1<'_>,
        _: &ProtectedDefaultAccessWitnessV1,
        receiver: ProtectedDefaultReferenceReceiverV1<'_>,
        _: &mut BudgetMeter,
        _: &WirePath,
    ) -> Result<(), &'static str> {
        match receiver {
            ProtectedDefaultReferenceReceiverV1::Metadata(metadata) => {
                assert!(matches!(
                    occurrence.attachment,
                    DefaultBodyReferenceAttachmentV1::Metadata(_)
                ));
                self.metadata += 1;
                if let DefaultBodyReferenceMetadataV1::Assignment(DefaultAssignTargetV1::Field {
                    receiver,
                    ..
                }) = metadata
                {
                    assert!(matches!(
                        receiver.kind(),
                        DefaultExpressionKindV1::GlobalRead(_)
                    ));
                    self.assignments += 1;
                }
                if self.reject_metadata {
                    return Err("definition-side metadata access rejected");
                }
            }
            ProtectedDefaultReferenceReceiverV1::Member {
                implicit_this,
                direct_super,
                ..
            } => {
                self.members.push((implicit_this, direct_super));
            }
            ProtectedDefaultReferenceReceiverV1::None => {
                assert!(matches!(
                    occurrence.attachment,
                    DefaultBodyReferenceAttachmentV1::Expression { .. }
                ));
            }
        }
        Ok(())
    }
}
pub(super) struct Input {
    pub f: Fixture,
    pub body: ExportDefaultBodyV1,
    pub locals: CanonicalTemplateLocalTableV1,
    pub receiver: OptionalTemplateReceiverV1,
    pub refs: ProtectedDefaultReferenceSetV1,
}
impl Input {
    pub fn validate(
        &self,
        authority: &mut Authority,
        meter: &mut BudgetMeter,
    ) -> Result<(), ProtectedDefaultBodyClosureError<&'static str>> {
        self.refs
            .validate_body_closure(
                key(&self.f),
                &self.body,
                &self.locals,
                &self.f.origin(),
                &self.receiver,
                authority,
                meter,
                &WirePath::root(),
            )
            .map(|_| ())
    }
}
pub(super) fn metadata_input() -> Input {
    let f = Fixture::new();
    let local_type = SignatureTypeKey::Nominal(f.type_id);
    let locals = CanonicalTemplateLocalTableV1::try_new(vec![
        TemplateLocalRecordV1::try_new(
            f.local(),
            local_type.clone(),
            CanonicalBooleanV1::False,
            TemplateLocalDefinitionV1::Source(f.origin()),
        )
        .unwrap(),
    ])
    .unwrap();
    let mut refs = empty_set();
    refs.types = vec![reference(&f, local_type, vec![])];
    Input {
        body: body(&f, DefaultExpressionKindV1::UnitLiteral),
        f,
        locals,
        receiver: OptionalTemplateReceiverV1::Absent,
        refs,
    }
}
