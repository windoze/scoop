use scoop_hir::*;
use scoop_identity::{
    LocalValueSelector, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};

use super::*;

pub(super) fn local_path(default_ordinal: u32, local_ordinal: u32) -> StructuralDefinitionPath {
    StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, default_ordinal),
        [StructuralPathSegment::new(
            StructuralDefinitionSiteRole::LocalDeclaration,
            local_ordinal,
        )],
    )
}

pub(super) fn add_local(
    fixture: &mut CallableSourceSurface,
    path: StructuralDefinitionPath,
    ty: SignatureTypeKey,
) {
    let template = &fixture.interface.default_templates().records()[0];
    let mut locals = template.locals().records().to_vec();
    locals.push(
        TemplateLocalRecordV1::try_new(
            LocalValueSelector::LocalDeclaration { path },
            ty,
            CanonicalBooleanV1::False,
            TemplateLocalDefinitionV1::Source(template.definition_origin().clone()),
        )
        .unwrap(),
    );
    let statements = template.body().statements().to_vec();
    replace_contents(fixture, locals, statements);
}

pub(super) fn add_owner_expression(
    fixture: &mut CallableSourceSurface,
    owner_type: SignatureTypeKey,
) {
    let template = &fixture.interface.default_templates().records()[0];
    let expression = DefaultExpressionV1::try_new(
        DefaultExpressionKindV1::StructConstruct {
            owner_type,
            fields: vec![],
        },
        template.result().clone(),
        template.definition_origin().clone().clone(),
        scoop_identity::EvaluationOrigin::at_definition(
            (template.definition_origin().clone()).origin(),
        ),
    )
    .unwrap();
    let mut statements = template.body().statements().to_vec();
    statements.push(
        DefaultStatementV1::try_new(
            DefaultStatementKindV1::Expr(Box::new(expression)),
            template.definition_origin().clone(),
        )
        .unwrap(),
    );
    let locals = template.locals().records().to_vec();
    replace_contents(fixture, locals, statements);
}

pub(in super::super) fn replace_contents(
    fixture: &mut CallableSourceSurface,
    locals: Vec<TemplateLocalRecordV1>,
    statements: Vec<DefaultStatementV1>,
) {
    let interface = &fixture.interface;
    let template = &interface.default_templates().records()[0];
    let replacement = ExportDefaultTemplateV1::try_new(
        template.key(),
        template.definition_root(),
        template.definition_path().clone(),
        CanonicalTemplateLocalTableV1::try_new(locals).unwrap(),
        ExportDefaultBodyV1::try_new(statements, template.body().value().clone()).unwrap(),
        template.result().clone(),
        template.allows_suspend(),
        template.type_parameters().clone(),
        template.receiver().clone(),
        template.value_parameters().clone(),
        template.references().clone(),
        template.definition_origin().clone(),
    )
    .unwrap();
    fixture.interface = CrossConeHirInterfaceSectionV1::new(
        interface.public_bindings().clone(),
        interface.nominal_interfaces().clone(),
        interface.callable_interfaces().clone(),
        interface.property_interfaces().clone(),
        interface.type_aliases().clone(),
        interface.source_interfaces().clone(),
        CanonicalExportDefaultTemplatesV1::try_new(vec![replacement]).unwrap(),
        interface.constants().clone(),
        interface.definition_sources().clone(),
        interface.external_references().clone(),
        interface.generic_callable_bodies().clone(),
        interface.generic_initializations().clone(),
        interface.generic_delegates().clone(),
    );
}
