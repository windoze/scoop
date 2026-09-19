use super::*;

pub(super) fn build(
    key: ProtectedDefaultTemplateKeyV1,
    target: SourceNominalId,
    origin: &ExportDefinitionSourceV1,
    witness: ProtectedDefaultAccessWitnessV1,
    case: Case,
) -> ProtectedDefaultTemplateV1 {
    let SourceNominalId::Concrete(target) = target else {
        panic!("concrete target");
    };
    let CallableTemplateOrigin::Function(function) = key.owner() else {
        panic!("ordinary member");
    };
    let ty = SignatureTypeKey::Nominal(target);
    let metadata = matches!(
        case,
        Case::Metadata | Case::RejectMetadata | Case::Generic | Case::GenericReject
    );
    let locals = if metadata {
        vec![
            TemplateLocalRecordV1::try_new(
                LocalValueSelector::Parameter {
                    declaration_index: 0,
                },
                ty.clone(),
                CanonicalBooleanV1::False,
                TemplateLocalDefinitionV1::Source(origin.clone()),
            )
            .unwrap(),
        ]
    } else {
        vec![]
    };
    let types = if matches!(case, Case::EmptyReferences) {
        vec![]
    } else {
        vec![ProtectedDefaultReferenceV1::new(
            ty.clone(),
            origin.clone(),
            witness,
            CanonicalProtectedDefaultExpressionUsesV1::try_new(vec![
                ProtectedDefaultExpressionUseV1::new(0, ProtectedDefaultReceiverUseV1::None),
            ])
            .unwrap(),
        )]
    };
    ProtectedDefaultTemplateV1::try_new(
        key,
        PersistentLexicalRootV1::Function(function),
        StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
            [],
        ),
        CanonicalTemplateLocalTableV1::try_new(locals).unwrap(),
        ExportDefaultBodyV1::try_new(
            vec![],
            DefaultExpressionV1::try_new(
                DefaultExpressionKindV1::StructConstruct {
                    owner_type: ty.clone(),
                    fields: vec![],
                },
                ty.clone(),
                origin.clone(),
            )
            .unwrap(),
        )
        .unwrap(),
        ty,
        CanonicalBooleanV1::False,
        CanonicalBinderUseListV1::try_new(vec![]).unwrap(),
        OptionalTemplateReceiverV1::Absent,
        CanonicalTemplateValueParametersV1::try_new(vec![]).unwrap(),
        ProtectedDefaultReferenceSetV1::try_new(vec![], vec![], types, vec![], vec![], vec![])
            .unwrap(),
        origin.clone(),
    )
    .unwrap()
}
