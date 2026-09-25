use super::*;
use crate::cross_cone_type_semantics::protected_interfaces::tests::support::Fixture;
use crate::*;
use scoop_identity::SignatureTypeKey;
use scoop_wire::{decode_canonical, encode};
mod owners;
mod support;
mod wire_tests;
use support::*;

#[test]
fn protected_source_protocol_roundtrips_required_default_and_both_vararg_categories() {
    for vararg_default in [false, true] {
        let (mut fixture, callable, source, keys, mut authority) = fixture(vararg_default);
        let table =
            CanonicalProtectedCallableSourceInterfacesV1::try_new(vec![source.clone()]).unwrap();
        let bytes = encode(&table.index_templates(&keys).unwrap()).unwrap();
        let decoded: DecodedCanonicalProtectedCallableSourceInterfacesV1 =
            decode_canonical(&bytes).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        assert_eq!(decoded.resolve(&mut fixture, &keys).unwrap(), table);
        let graph_source = fixture.graph.clone();
        let graph = CheckedNominalInheritanceGraphV1::validate(
            graph_source.records.values(),
            &graph_source,
        )
        .unwrap();
        let checked = callable.validate_source(&graph, &mut fixture).unwrap();
        let protocol = source.validate_protected(checked, &mut authority).unwrap();
        assert_eq!(protocol.record(), &source);
        authority.calling[1] = ProtectedParameterCallingKindV1::Required;
        assert!(matches!(
            source.validate_protected(checked, &mut authority),
            Err(ProtectedSourceSemanticError::Calling { position: 1 })
        ));
    }
}

#[test]
fn protected_default_keys_reject_wrong_position_missing_keys_and_unreferenced_templates() {
    let (mut fixture, _, source, keys, _) = fixture(false);
    let wrong = ProtectedDefaultTemplateKeyV1::try_new(source.owner(), 0).unwrap();
    let first = &source.parameters().parameters()[1];
    let bad_parameters = CanonicalProtectedSourceParametersV1::try_new(vec![
        source.parameters().parameters()[0].clone(),
        ProtectedSourceParameterV1::new(
            first.name().clone(),
            first.value_type().clone(),
            ProtectedParameterCallingV1::Default { template: wrong },
            first.definition_origin().clone(),
        ),
    ])
    .unwrap();
    assert!(matches!(
        ProtectedCallableSourceInterfaceV1::try_new(source.owner(), bad_parameters),
        Err(ProtectedSourceBuildError::TemplateOwner { position: 1 })
    ));
    let bytes = encode(&source.index_templates(&keys).unwrap()).unwrap();
    let decoded: DecodedProtectedCallableSourceInterfaceV1 = decode_canonical(&bytes).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture, &ProtectedDefaultKeyIndexV1::default()),
        Err(ProtectedSourceResolutionError::Build(
            ProtectedSourceBuildError::DefaultIndex
        ))
    ));
    let table = CanonicalProtectedCallableSourceInterfacesV1::try_new(vec![source]).unwrap();
    let extra = ProtectedDefaultKeyIndexV1::try_new(vec![keys.keys()[0], wrong]).unwrap();
    assert!(matches!(
        table.validate_default_closure(&extra),
        Err(ProtectedSourceIndexError::Build(
            ProtectedSourceBuildError::DefaultClosure
        ))
    ));
}

#[test]
fn source_protocol_uses_actual_parameter_shape_and_core_array_identity() {
    let (mut fixture, callable, source, _, mut authority) = fixture(false);
    let graph_source = fixture.graph.clone();
    let graph =
        CheckedNominalInheritanceGraphV1::validate(graph_source.records.values(), &graph_source)
            .unwrap();
    let checked = callable.validate_source(&graph, &mut fixture).unwrap();
    authority.shapes[0] = SourceParameterShapeV1::new(
        scoop_identity::CanonicalIdentifier::new("forged").unwrap(),
        authority.shapes[0].value_type().clone(),
    );
    assert!(matches!(
        source.validate_protected(checked, &mut authority),
        Err(ProtectedSourceSemanticError::Shape { position: 0 })
    ));
    authority.shapes[0] = callable.payload().parameters().parameters()[0].clone();
    let mut parameters = source.parameters().parameters().to_vec();
    let vararg = &parameters[2];
    parameters[2] = ProtectedSourceParameterV1::new(
        vararg.name().clone(),
        vararg.value_type().clone(),
        ProtectedParameterCallingV1::VarargEmpty {
            element_type: vararg.value_type().clone(),
        },
        vararg.definition_origin().clone(),
    );
    let bad = ProtectedCallableSourceInterfaceV1::try_new(
        source.owner(),
        CanonicalProtectedSourceParametersV1::try_new(parameters).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        bad.validate_protected(checked, &mut authority),
        Err(ProtectedSourceSemanticError::Vararg { position: 2 })
    ));
}
