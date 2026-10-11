use super::*;

#[test]
fn equivalent_function_adapters_preserve_the_input_inference_shape() {
    let mut lowerer = nominals::lowerer();
    let int = lowerer.integer_type(hir::IntegerKind::SIGNED_32);
    let receiver = lowerer.intern_receiver_function_type(false, Some(int), vec![], int);
    let flat = lowerer.intern_function_type(false, vec![int], int);
    let receiver = lowerer.function_types[receiver].canonical_type;
    let flat = lowerer.function_types[flat].canonical_type;
    assert!(lowerer.is_subtype(receiver, flat));
    assert!(lowerer.is_subtype(flat, receiver));
    assert!(!lowerer.types_equal(receiver, flat));
    assert_eq!(lowerer.least_upper_bound(&[receiver, receiver]), receiver);
    assert_eq!(lowerer.least_upper_bound(&[flat, flat]), flat);
    assert_eq!(lowerer.least_upper_bound(&[receiver, flat]), lowerer.any);

    for source in [receiver, flat] {
        for lower_bound in [true, false] {
            let parameter = parameter(80, 0);
            let mut session = InferenceSession::new();
            let environment = session.add_environment(&[], &[parameter]);
            let variable = session.callable_variables(environment)[0];
            let constraint = if lower_bound {
                Constraint::Subtype(source.into(), variable.into())
            } else {
                Constraint::Subtype(variable.into(), source.into())
            };
            session.push(constraint, ConstraintOrigin::Declaration);
            assert_eq!(
                lowerer
                    .solve_constraints(&session)
                    .unwrap()
                    .type_for(variable),
                source
            );
        }
    }
}

#[test]
fn mixed_receiver_shapes_need_an_explicit_inference_choice() {
    let mut lowerer = nominals::lowerer();
    let int = lowerer.integer_type(hir::IntegerKind::SIGNED_32);
    let receiver = lowerer.intern_receiver_function_type(false, Some(int), vec![], int);
    let flat = lowerer.intern_function_type(false, vec![int], int);
    let mut session = InferenceSession::new();
    let environment = session.add_environment(&[], &[parameter(81, 0)]);
    let variable = session.callable_variables(environment)[0];
    for signature in [receiver, flat] {
        session.push(
            Constraint::Subtype(
                lowerer.function_types[signature].canonical_type.into(),
                variable.into(),
            ),
            ConstraintOrigin::Declaration,
        );
    }
    let failure = lowerer.solve_constraints(&session).unwrap_err();
    let ConstraintFailureKind::NoUniqueSolution {
        solution_frontier, ..
    } = failure.kind
    else {
        panic!("ambiguous function shape");
    };
    assert_eq!(solution_frontier.len(), 2);
}
