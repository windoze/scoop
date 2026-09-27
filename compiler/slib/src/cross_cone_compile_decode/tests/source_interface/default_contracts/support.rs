use scoop_hir::*;
use scoop_identity::{
    LocalValueSelector, NonEmptyVec, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};

use super::super::default_fixture::{Case, fixture as base};
use super::*;

#[derive(Clone, Copy)]
pub(super) enum Change {
    Ordinal,
    Mapping,
    Result,
    Receiver,
    Prefix,
    Suspend,
}

pub(super) fn fixture(change: Change) -> CallableSourceSurface {
    let mut fixture = base(Case::Defined);
    let interface = &fixture.interface;
    let template = &interface.default_templates().records()[0];
    let ty = template.result().clone();
    let mut locals = template.locals().records().to_vec();
    let mut receiver = template.receiver().clone();
    let mut parameters = template.value_parameters().clone();
    let mut mapping = template.type_parameters().clone();
    let mut path = template.definition_path().clone();
    let mut result = ty.clone();
    let mut body = template.body().clone();
    let mut suspend = template.allows_suspend();
    match change {
        Change::Ordinal => {
            path = StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 1),
                [],
            )
        }
        Change::Mapping => mapping = CanonicalBinderUseListV1::try_new(vec![ty.clone()]).unwrap(),
        Change::Result => {
            result = SignatureTypeKey::Tuple(NonEmptyVec::from_first(ty.clone(), [ty.clone()]));
            let value = DefaultExpressionV1::try_new(
                DefaultExpressionKindV1::StructConstruct {
                    owner_type: result.clone(),
                    fields: vec![],
                },
                result.clone(),
                template.definition_origin().clone(),
            )
            .unwrap();
            body = ExportDefaultBodyV1::try_new(vec![], value).unwrap();
        }
        Change::Receiver => {
            locals.push(
                TemplateLocalRecordV1::try_new(
                    LocalValueSelector::This,
                    ty.clone(),
                    CanonicalBooleanV1::False,
                    TemplateLocalDefinitionV1::Source(template.definition_origin().clone()),
                )
                .unwrap(),
            );
            receiver = OptionalTemplateReceiverV1::Present(
                TemplateReceiverV1::try_new(LocalValueSelector::This, ty.clone()).unwrap(),
            );
        }
        Change::Prefix => {
            let selector = LocalValueSelector::Parameter {
                declaration_index: 0,
            };
            locals.push(
                TemplateLocalRecordV1::try_new(
                    selector.clone(),
                    ty.clone(),
                    CanonicalBooleanV1::False,
                    TemplateLocalDefinitionV1::Source(template.definition_origin().clone()),
                )
                .unwrap(),
            );
            parameters = CanonicalTemplateValueParametersV1::try_new(vec![
                TemplateValueParameterV1::try_new(0, selector).unwrap(),
            ])
            .unwrap();
        }
        Change::Suspend => suspend = CanonicalBooleanV1::True,
    }
    let replacement = ExportDefaultTemplateV1::try_new(
        template.key(),
        template.definition_root(),
        path,
        CanonicalTemplateLocalTableV1::try_new(locals).unwrap(),
        body,
        result,
        suspend,
        mapping,
        receiver,
        parameters,
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
    );
    fixture
}
