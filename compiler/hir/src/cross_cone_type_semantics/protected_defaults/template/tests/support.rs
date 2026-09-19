use scoop_identity::{
    CallableTemplateOrigin, LocalValueSelector, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
};
use scoop_wire::{BudgetMeter, DecodeLimits, decode_canonical, encode};

pub(super) use super::super::*;
pub(super) use crate::cross_cone_interface::expression_test_support::{Fixture, Resolver};
pub(super) use crate::cross_cone_type_semantics::protected_defaults::{
    CanonicalProtectedDefaultExpressionUsesV1, CanonicalProtectedDefaultSlotCallDomainsV1,
    ProtectedDefaultAccessWitnessV1, ProtectedDefaultExpressionUseV1,
    ProtectedDefaultReceiverUseV1, ProtectedDefaultReferenceKindV1, ProtectedDefaultReferenceSetV1,
    ProtectedDefaultReferenceV1,
};
pub(super) use crate::{
    DefaultExpressionKindV1, DefaultExpressionV1, PersistentAccessDomainV1,
    PersistentLookupDomainV1, TemplateLocalDefinitionV1, TemplateLocalRecordV1, TemplateReceiverV1,
    TemplateValueParameterV1,
};

pub(super) fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
pub(super) fn references(
    f: &Fixture,
    owner: CallableTemplateOrigin,
) -> ProtectedDefaultReferenceSetV1 {
    let domain = PersistentLookupDomainV1::new(PersistentAccessDomainV1::universal());
    let witness = ProtectedDefaultAccessWitnessV1::param_free(
        owner,
        domain.clone(),
        CanonicalProtectedDefaultSlotCallDomainsV1::try_new(vec![]).unwrap(),
        domain,
    )
    .unwrap();
    ProtectedDefaultReferenceSetV1::try_new(
        vec![],
        vec![],
        vec![],
        vec![ProtectedDefaultReferenceV1::new(
            f.property,
            f.origin(),
            witness,
            CanonicalProtectedDefaultExpressionUsesV1::try_new(vec![
                ProtectedDefaultExpressionUseV1::new(0, ProtectedDefaultReceiverUseV1::None),
            ])
            .unwrap(),
        )],
        vec![],
        vec![],
    )
    .unwrap()
}
pub(super) fn local(f: &Fixture, selector: LocalValueSelector) -> TemplateLocalRecordV1 {
    TemplateLocalRecordV1::try_new(
        selector,
        f.value_type(),
        CanonicalBooleanV1::False,
        TemplateLocalDefinitionV1::Source(f.origin()),
    )
    .unwrap()
}
pub(super) fn expression(f: &Fixture, kind: DefaultExpressionKindV1) -> DefaultExpressionV1 {
    DefaultExpressionV1::try_new(kind, f.value_type(), f.origin()).unwrap()
}
pub(super) fn template(f: &Fixture) -> ProtectedDefaultTemplateV1 {
    let owner = CallableTemplateOrigin::Function(f.function);
    ProtectedDefaultTemplateV1::try_new(
        ProtectedDefaultTemplateKeyV1::try_new(owner, 1).unwrap(),
        PersistentLexicalRootV1::Function(f.function),
        StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
            [],
        ),
        CanonicalTemplateLocalTableV1::try_new(vec![
            local(f, LocalValueSelector::This),
            local(f, f.local()),
        ])
        .unwrap(),
        ExportDefaultBodyV1::try_new(
            vec![],
            expression(f, DefaultExpressionKindV1::GlobalRead(f.property)),
        )
        .unwrap(),
        f.value_type(),
        CanonicalBooleanV1::False,
        CanonicalBinderUseListV1::try_new(vec![]).unwrap(),
        OptionalTemplateReceiverV1::Present(
            TemplateReceiverV1::try_new(LocalValueSelector::This, f.value_type()).unwrap(),
        ),
        CanonicalTemplateValueParametersV1::try_new(vec![
            TemplateValueParameterV1::try_new(0, f.local()).unwrap(),
        ])
        .unwrap(),
        references(f, owner),
        f.origin(),
    )
    .unwrap()
}
pub(super) fn decoded(value: &ProtectedDefaultTemplateV1) -> DecodedProtectedDefaultTemplateV1 {
    decode_canonical(
        &encode(&value.index_locals().unwrap()).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap()
}
pub(super) fn rebuild(
    value: ProtectedDefaultTemplateV1,
) -> Result<ProtectedDefaultTemplateV1, ProtectedDefaultTemplateBuildError> {
    ProtectedDefaultTemplateV1::try_new(
        value.key,
        value.definition_root,
        value.definition_path,
        value.locals,
        value.body,
        value.result,
        value.allows_suspend,
        value.type_parameters,
        value.receiver,
        value.value_parameters,
        value.references,
        value.definition_origin,
    )
}
