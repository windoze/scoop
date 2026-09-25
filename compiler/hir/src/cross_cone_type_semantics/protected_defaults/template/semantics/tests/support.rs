use super::*;
use crate::cross_cone_type_semantics::inheritance::tests::support::site;
pub(super) use crate::cross_cone_type_semantics::protected_interfaces::tests::support::{
    Fixture, nominal,
};

mod builders;

pub(super) fn binder(depth: u32, index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth, index }
}
pub(super) fn local(
    selector: LocalValueSelector,
    ty: SignatureTypeKey,
    origin: &ExportDefinitionSourceV1,
) -> TemplateLocalRecordV1 {
    TemplateLocalRecordV1::try_new(
        selector,
        ty,
        CanonicalBooleanV1::False,
        TemplateLocalDefinitionV1::Source(origin.clone()),
    )
    .unwrap()
}
pub(super) enum Record {
    Method(ProtectedCallableInterfaceV1),
    Constructor(ProtectedConstructorInterfaceV1),
    Support(NominalSupportCallableInterfaceV1),
}
impl Record {
    pub fn payload(&self) -> &NominalSourceCallablePayloadV1 {
        match self {
            Self::Method(record) => record.payload(),
            Self::Constructor(record) => record.payload(),
            Self::Support(record) => record.payload(),
        }
    }
    pub fn check<'a>(
        &'a self,
        graph: &CheckedNominalInheritanceGraphV1<'_>,
        fixture: &'a mut Fixture,
    ) -> ProtectedDefaultOwnerSourceV1<'a> {
        match self {
            Self::Method(record) => ProtectedDefaultOwnerSourceV1::Protected(
                record.validate_source(graph, fixture).unwrap(),
            ),
            Self::Constructor(record) => ProtectedDefaultOwnerSourceV1::Protected(
                record.validate_source(graph, fixture).unwrap(),
            ),
            Self::Support(record) => ProtectedDefaultOwnerSourceV1::NominalSupport(
                record.validate_source(graph, fixture).unwrap(),
            ),
        }
    }
}
pub(super) struct Case {
    pub fixture: Fixture,
    pub record: Record,
    pub source: ProtectedCallableSourceInterfaceV1,
    pub key: ProtectedDefaultTemplateKeyV1,
    pub provider: DefaultTemplateProviderShapeV1,
    pub expected_receiver: Option<SignatureTypeKey>,
    pub origin: ExportDefinitionSourceV1,
}
impl Case {
    pub fn template(&self) -> ProtectedDefaultTemplateV1 {
        let position = self.key.parameter_position();
        let parameters = self.source.parameters().parameters();
        let result = parameters[position as usize].value_type().clone();
        let mut locals = Vec::new();
        let receiver = self.expected_receiver.as_ref().map(|ty| {
            locals.push(local(LocalValueSelector::This, ty.clone(), &self.origin));
            TemplateReceiverV1::try_new(LocalValueSelector::This, ty.clone()).unwrap()
        });
        let values = (0..position)
            .map(|index| {
                let selector = LocalValueSelector::Parameter {
                    declaration_index: index,
                };
                locals.push(local(
                    selector.clone(),
                    parameters[index as usize].value_type().clone(),
                    &self.origin,
                ));
                TemplateValueParameterV1::try_new(index, selector).unwrap()
            })
            .collect();
        let mut mapping = Vec::new();
        if self.provider.nominal_owner_binder_arity() != 0 {
            mapping.push(binder(
                u32::from(self.provider.callable_own_binder_arity() != 0),
                0,
            ));
        }
        if self.provider.callable_own_binder_arity() != 0 {
            mapping.push(binder(0, 0));
        }
        let kind = if position == 0 {
            DefaultExpressionKindV1::UnitLiteral
        } else {
            DefaultExpressionKindV1::Local(LocalValueSelector::Parameter {
                declaration_index: 0,
            })
        };
        ProtectedDefaultTemplateV1::try_new(
            self.key,
            root(self.key.owner()),
            StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, position),
                [],
            ),
            CanonicalTemplateLocalTableV1::try_new(locals).unwrap(),
            ExportDefaultBodyV1::try_new(
                vec![],
                DefaultExpressionV1::try_new(kind, result.clone(), self.origin.clone()).unwrap(),
            )
            .unwrap(),
            result,
            CanonicalBooleanV1::False,
            CanonicalBinderUseListV1::try_new(mapping).unwrap(),
            receiver.map_or(
                OptionalTemplateReceiverV1::Absent,
                OptionalTemplateReceiverV1::Present,
            ),
            CanonicalTemplateValueParametersV1::try_new(values).unwrap(),
            ProtectedDefaultReferenceSetV1::try_new(vec![], vec![], vec![], vec![], vec![], vec![])
                .unwrap(),
            self.origin.clone(),
        )
        .unwrap()
    }
    pub fn validate(
        &self,
        template: &ProtectedDefaultTemplateV1,
        authority: &mut Authority,
    ) -> Result<(), ProtectedDefaultTemplateContractSemanticError<&'static str>> {
        let mut fixture = self.fixture.clone();
        let graph_source = fixture.graph.clone();
        let graph = CheckedNominalInheritanceGraphV1::validate_with_source_roots(
            graph_source.records.values(),
            graph_source.keys.keys().copied(),
            &graph_source,
        )
        .unwrap();
        let owner = self.record.check(&graph, &mut fixture);
        let mut source_authority = super::authority::ProtocolAuthority::new(self);
        let protocol = match owner {
            ProtectedDefaultOwnerSourceV1::Protected(checked) => self
                .source
                .validate_protected(checked, &mut source_authority)
                .unwrap(),
            ProtectedDefaultOwnerSourceV1::NominalSupport(checked) => self
                .source
                .validate_nominal_support(checked, &mut source_authority)
                .unwrap(),
        };
        template.validate_contract_semantics(owner, protocol, authority)
    }
}
fn root(owner: CallableTemplateOrigin) -> PersistentLexicalRootV1 {
    PersistentLexicalRootV1::try_from(owner).unwrap()
}
