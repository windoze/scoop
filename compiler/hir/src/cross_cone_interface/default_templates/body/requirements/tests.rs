use super::*;
use crate::cross_cone_interface::default_templates::body::expressions::test_support::Fixture;
use crate::*;
use scoop_identity::{MemberRequirementKey, RequiredEffectsKey, RequiredMemberKey};
use scoop_wire::{decode_canonical, encode};

fn reference(fixture: &Fixture) -> DefaultMemberRequirementRefV1 {
    DefaultMemberRequirementRefV1 {
        declaration: DefaultCallableDeclarationV1::Function(fixture.function),
        parameter: 0,
        requirement: 1,
    }
}

fn value(fixture: &Fixture) -> DefaultExpressionV1 {
    DefaultExpressionV1::try_new(
        DefaultExpressionKindV1::UnitLiteral,
        fixture.value_type(),
        fixture.origin(),
        scoop_identity::EvaluationOrigin::at_definition(fixture.origin().origin()),
    )
    .unwrap()
}

#[test]
fn required_call_statement_and_callable_binding_round_trip() {
    let fixture = Fixture::new();
    let binding = DefaultRequirementBindingValueV1::Forwarded(reference(&fixture));
    let statement = DefaultStatementV1::try_new(
        DefaultStatementKindV1::MemberRequirementCall {
            contract: Box::new(MemberRequirementKey {
                subject: fixture.value_type(),
                call: RequiredMemberKey::Method {
                    name: "read".into(),
                    type_arguments: Vec::new(),
                },
                arguments: Vec::new(),
                result: fixture.value_type(),
                effects: RequiredEffectsKey {
                    is_suspend: false,
                    is_unsafe: false,
                    is_no_gc: true,
                },
            }),
            binding: Box::new(binding.clone()),
            arguments: vec![value(&fixture)],
            result: Box::new(DefaultPatternV1::wildcard()),
        },
        fixture.origin(),
    )
    .unwrap();
    let bytes = encode(&statement.index_locals(&mut fixture.locals()).unwrap()).unwrap();
    let decoded: DecodedDefaultStatementV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(
        decoded
            .resolve(&mut fixture.resolver(), &mut fixture.locals())
            .unwrap(),
        statement
    );

    let callable = fixture
        .callable()
        .with_requirements(vec![DefaultRequirementBindingV1 {
            requirement: reference(&fixture),
            value: binding,
        }]);
    let bytes = encode(&callable).unwrap();
    let decoded: DecodedDefaultCallableRefV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(decoded.resolve(&mut fixture.resolver()).unwrap(), callable);
}

#[test]
fn required_implementation_checks_result_arity_and_input_indices() {
    let fixture = Fixture::new();
    for (results, inputs, valid) in [
        (vec![value(&fixture)], Vec::new(), true),
        (Vec::new(), Vec::new(), false),
        (vec![value(&fixture), value(&fixture)], Vec::new(), false),
        (vec![value(&fixture)], vec![0], false),
    ] {
        let binding = DefaultRequirementBindingValueV1::Implementation {
            body: ExportTemplateFragmentV1::new(
                CanonicalTemplateLocalTableV1::try_new(Vec::new()).unwrap(),
                Vec::new(),
                results,
            ),
            inputs,
            type_arguments: vec![fixture.value_type()],
        };
        let bytes = encode(&binding).unwrap();
        let decoded: DecodedDefaultRequirementBindingValueV1 = decode_canonical(&bytes).unwrap();
        let result = decoded.resolve(&mut fixture.resolver());
        if valid {
            assert_eq!(result.unwrap(), binding);
        } else {
            assert!(matches!(
                result,
                Err(RequirementBindingResolutionError::Shape)
            ));
        }
    }
}

#[test]
fn lexical_substitutions_preserve_original_requirement_bindings() {
    let fixture = Fixture::new();
    let reference = reference(&fixture);
    let bindings = vec![DefaultRequirementBindingV1 {
        requirement: reference,
        value: DefaultRequirementBindingValueV1::Forwarded(reference),
    }];
    for arguments in [
        DefaultCallableBodyTypeArgumentsV1::lexical(),
        DefaultCallableBodyTypeArgumentsV1::try_explicit(vec![fixture.value_type()]).unwrap(),
    ] {
        let arguments = arguments.with_requirements(bindings.clone());
        let bytes = encode(&arguments).unwrap();
        let decoded: DecodedDefaultCallableBodyTypeArgumentsV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        assert_eq!(decoded.resolve(&mut fixture.resolver()).unwrap(), arguments);
    }
}
