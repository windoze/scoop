use super::*;
use crate::cross_cone_type_semantics::inheritance::tests::support::site;
use crate::cross_cone_type_semantics::protected_interfaces::tests::support::{Fixture, nominal};

pub(super) fn fixture(
    fixture: &mut Fixture,
    outer: SourceNominalId,
) -> (ProtectedDeclarationInterfaceV1, Vec<CallableTemplateOrigin>) {
    let (source, owners) = nested(fixture, vec![outer], "OuterGeneric", true);
    (
        ProtectedDeclarationInterfaceV1::NestedNominal(Box::new(
            ProtectedNestedNominalInterfaceV1::try_new(
                source.declaration(),
                source.declaration_access().clone(),
                source.payload().clone(),
            )
            .unwrap(),
        )),
        owners,
    )
}
fn nested(
    fixture: &mut Fixture,
    parents: Vec<SourceNominalId>,
    name: &str,
    child: bool,
) -> (NominalSupportNestedInterfaceV1, Vec<CallableTemplateOrigin>) {
    let key = SourceDeclarationKey::nominal(
        site(&parents),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        1,
    );
    let owner = SourceNominalId::from_source_declaration(&key).unwrap();
    let origin = fixture.graph.origins[&parents[0]].clone();
    let access = DeclarationAccessSourceV1::try_new(
        if child {
            DeclaredVisibilityV1::Protected
        } else {
            DeclaredVisibilityV1::Private
        },
        parents.clone(),
        origin.clone(),
    )
    .unwrap();
    fixture.graph.keys.insert(owner, key);
    fixture.graph.access.insert(owner, access.clone());
    fixture.graph.origins.insert(owner, origin.clone());
    let mut chain = parents;
    chain.push(owner);
    let function_key = SourceDeclarationKey::function(
        site(&chain),
        CanonicalIdentifier::new("privateHelper").unwrap(),
        0,
        None,
        vec![],
    );
    let function = PersistentFunctionId::from_source_declaration(&function_key).unwrap();
    let declaration = CallableTemplateOrigin::Function(function);
    fixture.declarations.insert(declaration, function_key);
    let effects = fixture
        .payload(
            fixture.unit,
            declaration,
            vec![],
            SignatureTypeKey::Nominal(nominal(fixture.unit)),
        )
        .effects();
    let callable = NominalSupportCallableInterfaceV1::try_new(
        declaration,
        DeclarationAccessSourceV1::try_new(DeclaredVisibilityV1::Private, chain.clone(), origin)
            .unwrap(),
        NominalSourceCallablePayloadV1::try_new(
            declaration,
            owner,
            CanonicalBinderListV1::try_new(vec![]).unwrap(),
            CanonicalSourceParameterShapesV1::try_new(vec![]).unwrap(),
            SignatureTypeKey::Nominal(nominal(fixture.unit)),
            effects,
            CallableModalityV1::Final,
            CanonicalProtectedSlotRefsV1::try_new(vec![]).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let mut support = vec![NestedSourceSupportV1::Callable(Box::new(callable))];
    let mut owners = vec![declaration];
    let children = if child {
        let (nested, child_owners) = nested(fixture, chain, "InnerGeneric", false);
        owners.extend(child_owners);
        let children = vec![nested.declaration()];
        support.push(NestedSourceSupportV1::NestedNominal(Box::new(nested)));
        children
    } else {
        vec![]
    };
    let source = ProtectedNestedSourceInterfaceV1::try_new(
        PublicNominalKindV1::Class,
        NominalInheritanceModalityV1::Open,
        CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(
            CanonicalIdentifier::new("T").unwrap(),
            TypeParameterBoundsV1::Unconstrained,
        )])
        .unwrap(),
        CanonicalSignatureTypesV1::try_new(vec![]).unwrap(),
        CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
        CanonicalNestedMemberRefsV1::try_new(vec![NestedSourceMemberRefV1::Function(function)])
            .unwrap(),
        CanonicalNestedNominalRefsV1::try_new(children).unwrap(),
        NominalSourceShapeV1::Class,
        CanonicalNestedSourceSupportV1::try_new(support).unwrap(),
    )
    .unwrap();
    fixture.nominal_sources.insert(owner, source.clone());
    (
        NominalSupportNestedInterfaceV1::try_new(
            owner,
            access,
            ProtectedNestedNominalPayloadV1::try_new(
                owner,
                source,
                NestedNominalSupportV1::GenericTemplate,
            )
            .unwrap(),
        )
        .unwrap(),
        owners,
    )
}
