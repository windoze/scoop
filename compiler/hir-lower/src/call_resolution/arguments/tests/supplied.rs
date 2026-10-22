use super::*;

fn positional(spread: bool) -> ArgumentShape<'static> {
    ArgumentShape {
        name: None,
        spread,
        trailing: false,
    }
}

#[test]
fn a_named_prefix_keeps_the_other_required_parameters_unfilled() {
    let parameters: &[ParameterShape<'_, ()>] = &[
        ParameterShape {
            name: "first",
            calling: ParameterCalling::Required,
        },
        ParameterShape {
            name: "second",
            calling: ParameterCalling::Required,
        },
    ];
    let arguments = [ArgumentShape {
        name: Some("second"),
        ..positional(false)
    }];
    let supplied = SuppliedArgumentMap::bind(
        parameters,
        ArgumentMode::Mixed,
        &arguments,
        ReceiverInput::Absent,
    )
    .unwrap();
    assert_eq!(supplied.parameters[0], None);
    assert_eq!(
        supplied.parameters[1],
        Some(ResolvedParameterInput::Explicit(SourceInputId::from_index(
            0
        )))
    );
    assert_eq!(
        CandidateArgumentMap::map(
            parameters,
            ArgumentMode::Mixed,
            &arguments,
            ReceiverInput::Absent
        ),
        Err(ArgumentShapeFailure::MissingRequired {
            name: "first".into()
        })
    );
}

#[test]
fn defaults_and_vararg_omission_only_apply_at_the_complete_call_boundary() {
    let parameters = [
        ParameterShape {
            name: "first",
            calling: ParameterCalling::Required,
        },
        ParameterShape {
            name: "values",
            calling: ParameterCalling::Vararg {
                default: Some(1_u8),
            },
        },
        ParameterShape {
            name: "last",
            calling: ParameterCalling::Default(2),
        },
    ];
    let arguments = [positional(false)];
    let supplied = SuppliedArgumentMap::bind(
        &parameters,
        ArgumentMode::Mixed,
        &arguments,
        ReceiverInput::Present,
    )
    .unwrap();
    assert_eq!(supplied.receiver, ReceiverInput::Present);
    assert_eq!(&supplied.parameters[1..], &[None, None]);
    let complete = CandidateArgumentMap::map(
        &parameters,
        ArgumentMode::Mixed,
        &arguments,
        ReceiverInput::Present,
    )
    .unwrap();
    assert_eq!(
        complete.parameters[1].input,
        ResolvedParameterInput::Vararg(ResolvedVarargInput::Default(1))
    );
    assert_eq!(
        complete.parameters[2].input,
        ResolvedParameterInput::Default(2)
    );
    assert_eq!(complete.source_order.len(), 1);
}

#[test]
fn a_partial_vararg_and_trailing_lambda_keep_their_actual_source_inputs() {
    let parameters: &[ParameterShape<'_, ()>] = &[
        ParameterShape {
            name: "missing",
            calling: ParameterCalling::Required,
        },
        ParameterShape {
            name: "values",
            calling: ParameterCalling::Vararg { default: None },
        },
        ParameterShape {
            name: "block",
            calling: ParameterCalling::Required,
        },
    ];
    let arguments = [
        ArgumentShape {
            name: Some("values"),
            ..positional(true)
        },
        ArgumentShape {
            trailing: true,
            ..positional(false)
        },
    ];
    let supplied = SuppliedArgumentMap::bind(
        parameters,
        ArgumentMode::Mixed,
        &arguments,
        ReceiverInput::Absent,
    )
    .unwrap();
    assert_eq!(supplied.parameters[0], None);
    assert_eq!(
        supplied.parameters[1],
        Some(ResolvedParameterInput::Vararg(
            ResolvedVarargInput::WholeArray(SourceInputId::from_index(0))
        ))
    );
    assert_eq!(
        supplied.parameters[2],
        Some(ResolvedParameterInput::Explicit(SourceInputId::from_index(
            1
        )))
    );
    assert_eq!(
        supplied.source_order,
        vec![SourceInputId::from_index(0), SourceInputId::from_index(1)]
    );
}

#[test]
fn partial_mapping_still_rejects_already_decidable_argument_errors() {
    let parameters: &[ParameterShape<'_, ()>] = &[
        ParameterShape {
            name: "first",
            calling: ParameterCalling::Required,
        },
        ParameterShape {
            name: "second",
            calling: ParameterCalling::Required,
        },
    ];
    for (arguments, expected) in [
        (
            vec![ArgumentShape {
                name: Some("unknown"),
                ..positional(false)
            }],
            ArgumentShapeFailure::UnknownName {
                name: "unknown".into(),
            },
        ),
        (
            vec![
                ArgumentShape {
                    name: Some("second"),
                    ..positional(false)
                },
                positional(false),
            ],
            ArgumentShapeFailure::PositionalAfterNamed,
        ),
        (
            vec![positional(true)],
            ArgumentShapeFailure::SpreadForRegular {
                name: "first".into(),
            },
        ),
    ] {
        assert_eq!(
            SuppliedArgumentMap::bind(
                parameters,
                ArgumentMode::Mixed,
                &arguments,
                ReceiverInput::Absent
            ),
            Err(expected)
        );
    }
}
