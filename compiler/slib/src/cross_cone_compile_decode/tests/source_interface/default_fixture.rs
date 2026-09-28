use scoop_hir::*;
use scoop_identity::{
    LocalValueSelector, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};

use super::{CallableSourceSurface, SourceInterfaceCase};

#[derive(Clone, Copy)]
pub(super) enum Case {
    Defined,
    ReadBeforeDefinition,
    ImmutableAssignment,
    BreakOutsideLoop,
    EvaluationOutsideSource,
}

pub(super) fn fixture(case: Case) -> CallableSourceSurface {
    let mut fixture = CallableSourceSurface::new(SourceInterfaceCase::Complete);
    let interface = &fixture.interface;
    let source = interface.source_interfaces().get(fixture.owner).unwrap();
    let parameter = &source.parameters().parameters()[0];
    let origin = parameter.definition_origin().clone();
    let evaluation = if matches!(case, Case::EvaluationOutsideSource) {
        scoop_identity::EvaluationOrigin::new(
            origin.origin().source().clone(),
            scoop_identity::SourceSpan::new(1_000_000, 1_000_001).unwrap(),
            fixture
                .foundation
                .source_context_key(origin.origin().context())
                .unwrap(),
        )
        .unwrap()
    } else {
        scoop_identity::EvaluationOrigin::at_definition(origin.origin())
    };
    let ty = parameter.value_type().clone();
    let path = StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
        [],
    );
    let selector = LocalValueSelector::LocalDeclaration {
        path: StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
            [StructuralPathSegment::new(
                StructuralDefinitionSiteRole::LocalDeclaration,
                0,
            )],
        ),
    };
    let value = DefaultExpressionV1::try_new(
        DefaultExpressionKindV1::StructConstruct {
            owner_type: ty.clone(),
            fields: vec![],
        },
        ty.clone(),
        origin.clone(),
        evaluation.clone(),
    )
    .unwrap();
    let read = DefaultExpressionV1::try_new(
        DefaultExpressionKindV1::Local(selector.clone()),
        ty.clone(),
        origin.clone(),
        evaluation,
    )
    .unwrap();
    let declaration = DefaultStatementV1::try_new(
        DefaultStatementKindV1::ValDecl {
            pattern: DefaultPatternV1::binding(selector.clone()),
            init: Box::new(value.clone()),
        },
        origin.clone(),
    )
    .unwrap();
    let statements = match case {
        Case::Defined | Case::EvaluationOutsideSource => vec![declaration],
        Case::ReadBeforeDefinition => vec![
            DefaultStatementV1::try_new(
                DefaultStatementKindV1::Expr(Box::new(read.clone())),
                origin.clone(),
            )
            .unwrap(),
            declaration,
        ],
        Case::ImmutableAssignment => vec![
            declaration,
            DefaultStatementV1::try_new(
                DefaultStatementKindV1::Assign {
                    target: Box::new(DefaultAssignTargetV1::Local {
                        local: selector.clone(),
                    }),
                    value: Box::new(value),
                },
                origin.clone(),
            )
            .unwrap(),
        ],
        Case::BreakOutsideLoop => vec![
            declaration,
            DefaultStatementV1::try_new(DefaultStatementKindV1::Break, origin.clone()).unwrap(),
        ],
    };
    let key = ExportDefaultTemplateKeyV1::new(fixture.owner, 0);
    let reference = ExportDefaultReferenceV1::new(ty.clone(), origin.clone());
    let template = ExportDefaultTemplateV1::try_new(
        key,
        PersistentLexicalRootV1::try_from(fixture.owner).unwrap(),
        path,
        CanonicalTemplateLocalTableV1::try_new(vec![
            TemplateLocalRecordV1::try_new(
                selector,
                ty.clone(),
                CanonicalBooleanV1::False,
                TemplateLocalDefinitionV1::Source(origin.clone()),
            )
            .unwrap(),
        ])
        .unwrap(),
        ExportDefaultBodyV1::try_new(statements, read).unwrap(),
        ty,
        CanonicalBooleanV1::False,
        CanonicalBinderUseListV1::try_new(vec![]).unwrap(),
        OptionalTemplateReceiverV1::Absent,
        CanonicalTemplateValueParametersV1::try_new(vec![]).unwrap(),
        ExportDefaultReferenceSetV1::try_new(
            vec![],
            vec![],
            vec![reference],
            vec![],
            vec![],
            vec![],
        )
        .unwrap(),
        origin,
    )
    .unwrap();
    let sources = CanonicalCallableSourceInterfacesV1::try_new(vec![
        CallableSourceInterfaceV1::try_new(
            fixture.owner,
            CanonicalCallableSourceParametersV1::try_new(vec![CallableSourceParameterV1::new(
                parameter.name().clone(),
                parameter.value_type().clone(),
                CallableParameterCallingV1::Default { template: key },
                parameter.definition_origin().clone(),
            )])
            .unwrap(),
        )
        .unwrap(),
    ])
    .unwrap();
    fixture.interface = CrossConeHirInterfaceSectionV1::new(
        interface.public_bindings().clone(),
        interface.nominal_interfaces().clone(),
        interface.callable_interfaces().clone(),
        interface.property_interfaces().clone(),
        interface.type_aliases().clone(),
        sources,
        CanonicalExportDefaultTemplatesV1::try_new(vec![template]).unwrap(),
        interface.constants().clone(),
        interface.definition_sources().clone(),
        interface.external_references().clone(),
        interface.generic_callable_bodies().clone(),
        interface.generic_initializations().clone(),
        interface.generic_delegates().clone(),
    );
    fixture
}

pub(super) fn replace_references(
    fixture: &mut CallableSourceSurface,
    references: ExportDefaultReferenceSetV1,
) {
    let interface = &fixture.interface;
    let original = &interface.default_templates().records()[0];
    let templates = CanonicalExportDefaultTemplatesV1::try_new(vec![
        ExportDefaultTemplateV1::try_new(
            original.key(),
            original.definition_root(),
            original.definition_path().clone(),
            original.locals().clone(),
            original.body().clone(),
            original.result().clone(),
            original.allows_suspend(),
            original.type_parameters().clone(),
            original.receiver().clone(),
            original.value_parameters().clone(),
            references,
            original.definition_origin().clone(),
        )
        .unwrap(),
    ])
    .unwrap();
    let sources = CanonicalExportDefinitionSourcesV1::from_interface_parts(
        interface.type_aliases(),
        interface.source_interfaces(),
        &templates,
        interface.constants(),
        &Default::default(),
        &Default::default(),
        &Default::default(),
    )
    .unwrap();
    fixture.interface = CrossConeHirInterfaceSectionV1::new(
        interface.public_bindings().clone(),
        interface.nominal_interfaces().clone(),
        interface.callable_interfaces().clone(),
        interface.property_interfaces().clone(),
        interface.type_aliases().clone(),
        interface.source_interfaces().clone(),
        templates,
        interface.constants().clone(),
        sources,
        interface.external_references().clone(),
        interface.generic_callable_bodies().clone(),
        interface.generic_initializations().clone(),
        interface.generic_delegates().clone(),
    );
}
