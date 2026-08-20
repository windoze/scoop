use super::*;

fn trailing() -> ast::CallArgument {
    let mut input = argument(None, false);
    input.name = ast::CallArgumentName::TrailingLambda;
    input
}

#[test]
fn trailing_lambda_reserves_the_last_parameter_after_a_named_jump() {
    let mut candidate = view(3, ReceiverShape::None);
    candidate.signature.value_parameters[0].calling = ValueParameterCalling::Default(template(0));
    let arguments = [argument(Some("p1"), false), trailing()];
    let map = CandidateArgumentMap::source(&candidate, &arguments).unwrap();
    assert_eq!(map.explicit_default_count(), 1);
    assert_eq!(
        map.source_binding(SourceInputId::from_index(0)).0.index(),
        1
    );
    assert_eq!(
        map.source_binding(SourceInputId::from_index(1)).0.index(),
        2
    );
    assert_eq!(
        map.source_order,
        vec![SourceInputId::from_index(0), SourceInputId::from_index(1)]
    );
}

#[test]
fn a_trailing_lambda_does_not_overwrite_an_explicit_last_argument() {
    for first in [argument(None, false), argument(Some("p0"), false)] {
        assert_eq!(
            CandidateArgumentMap::source(&view(1, ReceiverShape::None), &[first, trailing()]),
            Err(ArgumentShapeFailure::DuplicateParameter { name: "p0".into() }),
        );
    }
}

#[test]
fn vararg_elements_and_a_trailing_lambda_use_different_parameters() {
    let parameters = [
        ParameterShape {
            name: "values",
            calling: ParameterCalling::<()>::Vararg { default: None },
        },
        ParameterShape {
            name: "action",
            calling: ParameterCalling::Required,
        },
    ];
    let source = [argument(None, false), argument(None, true), trailing()];
    let arguments = source.iter().map(ArgumentShape::from).collect::<Vec<_>>();
    let map = CandidateArgumentMap::map(
        &parameters,
        ArgumentMode::Mixed,
        &arguments,
        ReceiverInput::Absent,
    )
    .unwrap();
    assert_eq!(
        map.source_binding(SourceInputId::from_index(0)).1,
        SourceInputKind::VarargElement
    );
    assert_eq!(
        map.source_binding(SourceInputId::from_index(1)).1,
        SourceInputKind::VarargArray
    );
    assert_eq!(
        map.source_binding(SourceInputId::from_index(2)).0.index(),
        1
    );
    assert_eq!(
        CandidateArgumentMap::map(
            &parameters[..1],
            ArgumentMode::Mixed,
            &arguments[2..],
            ReceiverInput::Absent
        ),
        Err(ArgumentShapeFailure::TrailingForVararg {
            name: "values".into()
        }),
    );
}
