use super::*;
use scoop_identity::{CallableTemplateOrigin, SignatureTypeKey, SourceNominalKind};

pub(in crate::cross_cone_type_semantics::protected_interfaces) fn nested_class(
    fixture: &mut Fixture,
    outer: Node,
    name: &str,
) -> Node {
    let owner = fixture.graph.add(name, SourceNominalKind::Class, &[outer]);
    fixture
        .graph
        .visibility(owner, DeclaredVisibilityV1::Protected);
    let representation = NominalRepresentationSupportV1::try_new(
        &fixture.graph.keys[&owner.source],
        fixture.graph.access[&owner.source].clone(),
        NominalRepresentationShapeV1::Class {
            base: scoop_identity::OptionalSignatureType::Absent,
            declared_fields: vec![],
        },
    )
    .unwrap();
    fixture
        .graph
        .representations
        .insert(nominal(owner), representation);
    owner
}
pub(in crate::cross_cone_type_semantics::protected_interfaces) fn source(
    modality: NominalInheritanceModalityV1,
    constructors: Vec<PersistentConstructorId>,
    members: Vec<NestedSourceMemberRefV1>,
    children: Vec<SourceNominalId>,
    support: Vec<NestedSourceSupportV1>,
) -> ProtectedNestedSourceInterfaceV1 {
    ProtectedNestedSourceInterfaceV1::try_new(
        PublicNominalKindV1::Class,
        modality,
        CanonicalBinderListV1::try_new(vec![]).unwrap(),
        CanonicalSignatureTypesV1::try_new(vec![]).unwrap(),
        CanonicalPersistentIdsV1::try_new(constructors).unwrap(),
        CanonicalNestedMemberRefsV1::try_new(members).unwrap(),
        CanonicalNestedNominalRefsV1::try_new(children).unwrap(),
        NominalSourceShapeV1::Class(Default::default()),
        CanonicalNestedSourceSupportV1::try_new(support).unwrap(),
    )
    .unwrap()
}
pub(in crate::cross_cone_type_semantics::protected_interfaces) fn function(
    fixture: &mut Fixture,
    owner: Node,
    name: &str,
    visibility: DeclaredVisibilityV1,
) -> (NestedSourceMemberRefV1, NestedSourceSupportV1) {
    let declaration = fixture.function(owner, name, false, vec![]);
    let CallableTemplateOrigin::Function(id) = declaration else {
        unreachable!()
    };
    let payload = fixture
        .payload(
            owner,
            declaration,
            vec![],
            SignatureTypeKey::Nominal(nominal(fixture.unit)),
        )
        .source_signature;
    (
        NestedSourceMemberRefV1::Function(id),
        NestedSourceSupportV1::Callable(Box::new(
            NominalSupportCallableInterfaceV1::try_new(
                declaration,
                fixture.access(owner, visibility),
                payload,
            )
            .unwrap(),
        )),
    )
}
pub(in crate::cross_cone_type_semantics::protected_interfaces) fn payload(
    fixture: &mut Fixture,
    owner: Node,
    interface: ProtectedNestedSourceInterfaceV1,
) -> ProtectedNestedNominalPayloadV1 {
    fixture
        .nominal_sources
        .insert(owner.source, interface.clone());
    ProtectedNestedNominalPayloadV1::try_new(
        owner.source,
        interface,
        NestedNominalSupportV1::ParamFree {
            inheritance_exact: owner.exact,
            representation_owner: nominal(owner),
        },
    )
    .unwrap()
}
pub(in crate::cross_cone_type_semantics::protected_interfaces) fn representations(
    fixture: &Fixture,
) -> CanonicalNominalRepresentationSupportV1 {
    CanonicalNominalRepresentationSupportV1::try_new(
        fixture.graph.representations.values().cloned().collect(),
    )
    .unwrap()
}
