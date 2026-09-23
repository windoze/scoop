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
}

pub(super) fn fixture(case: Case) -> CallableSourceSurface {
    let mut fixture = CallableSourceSurface::new(SourceInterfaceCase::Complete);
    let interface = &fixture.interface;
    let source = interface.source_interfaces().get(fixture.owner).unwrap();
    let parameter = &source.parameters().parameters()[0];
    let origin = parameter.definition_origin().clone();
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
    )
    .unwrap();
    let read = DefaultExpressionV1::try_new(
        DefaultExpressionKindV1::Local(selector.clone()),
        ty.clone(),
        origin.clone(),
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
        Case::Defined => vec![declaration],
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
    let reference = ExportDefaultReferenceV1::new(
        ty.clone(),
        origin.clone(),
        ExportDefaultAccessWitnessV1::new(fixture.owner, ExportDefaultCallDomainV1::DirectPublic),
    );
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
    );
    fixture
}
