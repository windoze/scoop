use super::*;

pub(super) fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
pub(super) fn path() -> WirePath {
    WirePath::root().field(6)
}
pub(super) fn unit() -> SignatureTypeKey {
    SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id())
}
pub(super) fn empty_public(
    callables: Vec<CallableInterfaceRecordV1>,
) -> CrossConeHirInterfaceSectionV1 {
    public_with_properties(callables, vec![])
}
pub(super) fn public_with_properties(
    callables: Vec<CallableInterfaceRecordV1>,
    properties: Vec<PropertyInterfaceRecordV1>,
) -> CrossConeHirInterfaceSectionV1 {
    CrossConeHirInterfaceSectionV1::new(
        Default::default(),
        Default::default(),
        CanonicalCallableInterfacesV1::try_new(callables).unwrap(),
        CanonicalPropertyInterfacesV1::try_new(properties).unwrap(),
        Default::default(),
        CanonicalCallableSourceInterfacesV1::try_new(vec![]).unwrap(),
        CanonicalExportDefaultTemplatesV1::try_new(vec![]).unwrap(),
        Default::default(),
        Default::default(),
        Default::default(),
    )
}
pub(super) fn callable(
    declaration: CallableTemplateOrigin,
    payload: &NominalSourceCallablePayloadV1,
) -> CallableInterfaceRecordV1 {
    CallableInterfaceRecordV1::try_new(
        declaration,
        PublicDeclarationOwnerV1::Nominal(payload.owner()),
        payload.type_parameters().clone(),
        None,
        payload.parameters().clone(),
        payload.result().clone(),
        payload.effects(),
        payload.modality(),
        if payload.slot_relations().is_empty() {
            PublicLookupAccessV1::DirectOnly
        } else {
            PublicLookupAccessV1::PublicSlot
        },
    )
    .unwrap()
}
pub(super) fn default_path() -> StructuralDefinitionPath {
    StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 1),
        [],
    )
}
pub(super) fn owner() -> CallableTemplateOrigin {
    let key = SourceDeclarationKey::function(
        crate::cross_cone_type_semantics::inheritance::tests::support::site(&[]),
        CanonicalIdentifier::new("overlapDefault").unwrap(),
        0,
        None,
        vec![unit(), unit()],
    );
    CallableTemplateOrigin::Function(PersistentFunctionId::from_source_declaration(&key).unwrap())
}
pub(super) fn locals() -> CanonicalTemplateLocalTableV1 {
    CanonicalTemplateLocalTableV1::try_new(vec![
        TemplateLocalRecordV1::try_new(
            LocalValueSelector::Parameter {
                declaration_index: 0,
            },
            unit(),
            CanonicalBooleanV1::False,
            TemplateLocalDefinitionV1::Source(ExpressionFixture::new().origin()),
        )
        .unwrap(),
    ])
    .unwrap()
}
pub(super) fn body(local: bool) -> ExportDefaultBodyV1 {
    ExportDefaultBodyV1::try_new(
        vec![],
        DefaultExpressionV1::try_new(
            if local {
                DefaultExpressionKindV1::Local(LocalValueSelector::Parameter {
                    declaration_index: 0,
                })
            } else {
                DefaultExpressionKindV1::UnitLiteral
            },
            unit(),
            ExpressionFixture::new().origin(),
        )
        .unwrap(),
    )
    .unwrap()
}
pub(super) fn prefix() -> CanonicalTemplateValueParametersV1 {
    CanonicalTemplateValueParametersV1::try_new(vec![
        TemplateValueParameterV1::try_new(
            0,
            LocalValueSelector::Parameter {
                declaration_index: 0,
            },
        )
        .unwrap(),
    ])
    .unwrap()
}
pub(super) fn new_default(local: bool, suspend: CanonicalBooleanV1) -> ProtectedDefaultTemplateV1 {
    let owner = owner();
    let CallableTemplateOrigin::Function(function) = owner else {
        unreachable!()
    };
    ProtectedDefaultTemplateV1::try_new(
        ProtectedDefaultTemplateKeyV1::try_new(owner, 1).unwrap(),
        PersistentLexicalRootV1::Function(function),
        default_path(),
        locals(),
        body(local),
        unit(),
        suspend,
        CanonicalBinderUseListV1::try_new(vec![]).unwrap(),
        OptionalTemplateReceiverV1::Absent,
        prefix(),
        ProtectedDefaultReferenceSetV1::try_new(vec![], vec![], vec![], vec![], vec![], vec![])
            .unwrap(),
        ExpressionFixture::new().origin(),
    )
    .unwrap()
}
pub(super) fn old_default(local: bool) -> ExportDefaultTemplateV1 {
    let owner = owner();
    let CallableTemplateOrigin::Function(function) = owner else {
        unreachable!()
    };
    ExportDefaultTemplateV1::try_new(
        ExportDefaultTemplateKeyV1::new(owner, 1),
        PersistentLexicalRootV1::Function(function),
        default_path(),
        locals(),
        body(local),
        unit(),
        CanonicalBooleanV1::False,
        CanonicalBinderUseListV1::try_new(vec![]).unwrap(),
        OptionalTemplateReceiverV1::Absent,
        prefix(),
        ExportDefaultReferenceSetV1::try_new(vec![], vec![], vec![], vec![], vec![], vec![])
            .unwrap(),
        ExpressionFixture::new().origin(),
    )
    .unwrap()
}
pub(super) fn limit(error: WireError, expected: ResourceKind) {
    assert!(
        matches!(error.kind(), WireErrorKind::LimitExceeded { resource, .. } if *resource == expected),
        "{error:?}"
    );
    assert_eq!(error.byte_offset(), None);
}
