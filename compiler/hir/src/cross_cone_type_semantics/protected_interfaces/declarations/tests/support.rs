use super::*;
pub(super) use crate::cross_cone_type_semantics::protected_interfaces::nested::tests::support::representations;
use crate::cross_cone_type_semantics::protected_interfaces::nested::tests::support::{
    nested_class, payload, source,
};
use crate::cross_cone_type_semantics::protected_interfaces::property::tests::support::setup;

pub(super) fn with_inventory(
    fixture: &mut Fixture,
    records: Vec<ProtectedDeclarationInterfaceV1>,
) -> CanonicalProtectedDeclarationInterfacesV1 {
    fixture.protected_roots = CanonicalProtectedDeclarationRefsV1::try_new(
        records
            .iter()
            .map(ProtectedDeclarationInterfaceV1::reference)
            .collect(),
    )
    .unwrap();
    CanonicalProtectedDeclarationInterfacesV1::try_new(records).unwrap()
}

pub(super) fn complete() -> (Fixture, CanonicalProtectedDeclarationInterfacesV1) {
    let (mut fixture, owner, property, getter, setter) =
        setup(Some(DeclaredVisibilityV1::Protected));
    let mut records = vec![
        ProtectedDeclarationInterfaceV1::Property(Box::new(property)),
        ProtectedDeclarationInterfaceV1::Callable(Box::new(getter)),
        ProtectedDeclarationInterfaceV1::Callable(Box::new(setter.unwrap())),
    ];
    let constructor = fixture.constructor(owner);
    let constructor_payload = fixture.payload(
        owner,
        CallableTemplateOrigin::Constructor(constructor),
        vec![],
        SignatureTypeKey::Nominal(nominal(owner)),
    );
    records.push(ProtectedDeclarationInterfaceV1::Constructor(Box::new(
        ProtectedConstructorInterfaceV1::try_new(
            constructor,
            fixture.access(owner, DeclaredVisibilityV1::Protected),
            constructor_payload,
        )
        .unwrap(),
    )));
    for generic in [false, true] {
        let declaration = fixture.function(
            owner,
            if generic { "generic" } else { "function" },
            generic,
            vec![],
        );
        let value = fixture.record(
            owner,
            declaration,
            fixture.payload(
                owner,
                declaration,
                vec![],
                SignatureTypeKey::Nominal(nominal(fixture.unit)),
            ),
        );
        records.push(ProtectedDeclarationInterfaceV1::Callable(Box::new(value)));
    }
    let nested = nested_class(&mut fixture, owner, "Nested");
    let source = source(
        NominalInheritanceModalityV1::Open,
        vec![],
        vec![],
        vec![],
        vec![],
    );
    let payload = payload(&mut fixture, nested, source);
    records.push(ProtectedDeclarationInterfaceV1::NestedNominal(Box::new(
        ProtectedNestedNominalInterfaceV1::try_new(
            nested.source,
            fixture.graph.access[&nested.source].clone(),
            payload,
        )
        .unwrap(),
    )));
    let table = with_inventory(&mut fixture, records);
    (fixture, table)
}
