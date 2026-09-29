use scoop_ast::Span;
use scoop_hir as hir;

use super::constraints::{
    CallableCategory, CallableParameter, CallableReturn, CallableShape, CallableShapeMismatch,
    Constraint, ConstraintFailureKind, ConstraintOrigin, InferenceSession, InferenceVariableId,
    TypeTerm,
};
use crate::Type;

fn parameter(identity: u32, slot: u32) -> hir::TypeParamDecl {
    hir::TypeParamDecl {
        id: hir::TypeParamId::with_substitution_slot(identity, slot),
        name: format!("T{identity}"),
        bounds: hir::TypeParamBounds::Unconstrained,
        span: Span::new(0, 0),
    }
}

mod nominals;
use nominals::{add_generic_struct, add_interface};

#[test]
fn inference_variables_are_fresh_and_keep_owner_callable_groups_distinct() {
    let owner = parameter(10, 0);
    let callable = parameter(11, 1);
    let mut first = InferenceSession::new();
    let first_environment = first.add_environment(
        std::slice::from_ref(&owner),
        std::slice::from_ref(&callable),
    );
    let mut second = InferenceSession::new();
    let second_environment = second.add_environment(&[], std::slice::from_ref(&callable));

    let first_owner = first.owner_variables(first_environment)[0];
    let first_callable = first.callable_variables(first_environment)[0];
    let second_callable = second.callable_variables(second_environment)[0];
    assert_ne!(first.id(), second.id());
    assert_ne!(
        InferenceVariableId::from(first_owner),
        InferenceVariableId::from(first_callable)
    );
    assert_ne!(
        InferenceVariableId::from(first_callable),
        InferenceVariableId::from(second_callable)
    );
    assert_eq!(first.variable_for(owner.id), Some(first_owner.into()));
}

#[test]
fn exact_variable_relations_reach_a_fixed_point() {
    let mut lowerer = nominals::lowerer();
    let owner = parameter(20, 0);
    let callable = parameter(21, 1);
    let mut session = InferenceSession::new();
    let environment = session.add_environment(
        std::slice::from_ref(&owner),
        std::slice::from_ref(&callable),
    );
    let owner_variable = session.owner_variables(environment)[0];
    let callable_variable = session.callable_variables(environment)[0];
    session.push(
        Constraint::Equal(owner_variable.into(), callable_variable.into()),
        ConstraintOrigin::Declaration,
    );
    session.push(
        Constraint::Equal(
            owner_variable.into(),
            lowerer.integer_type(hir::IntegerKind::SIGNED_32).into(),
        ),
        ConstraintOrigin::ExplicitTypeArgument(0),
    );

    let solution = lowerer
        .solve_constraints(&session)
        .expect("variable equality propagates");
    assert_eq!(
        solution.type_for(owner_variable),
        lowerer.integer_type(hir::IntegerKind::SIGNED_32)
    );
    assert_eq!(
        solution.type_for(callable_variable),
        lowerer.integer_type(hir::IntegerKind::SIGNED_32)
    );
    assert_eq!(
        solution.arguments_for(&session, environment),
        super::solver::ConcreteInferenceArguments {
            owner: vec![lowerer.integer_type(hir::IntegerKind::SIGNED_32)],
            callable: vec![lowerer.integer_type(hir::IntegerKind::SIGNED_32)],
        }
    );
}

#[test]
fn subtype_bounds_choose_the_unique_expressible_minimum() {
    let mut lowerer = nominals::lowerer();
    let callable = parameter(30, 0);
    let mut session = InferenceSession::new();
    let environment = session.add_environment(&[], std::slice::from_ref(&callable));
    let variable = session.callable_variables(environment)[0];
    for ty in [
        lowerer.integer_type(hir::IntegerKind::SIGNED_32),
        lowerer.integer_type(hir::IntegerKind::UNSIGNED_32),
    ] {
        session.push(
            Constraint::Subtype(ty.into(), variable.into()),
            ConstraintOrigin::Argument(super::arguments::SourceInputId::from_test_index(0)),
        );
    }

    let solution = lowerer
        .solve_constraints(&session)
        .expect("Any is the actual unique minimum in the primitive test lattice");
    assert_eq!(solution.type_for(variable), lowerer.any);
}

#[test]
fn incomparable_minimal_upper_bounds_are_not_collapsed_to_any() {
    let mut lowerer = nominals::lowerer();
    let left_parent = add_interface(&mut lowerer, "Left", Vec::new());
    let right_parent = add_interface(&mut lowerer, "Right", Vec::new());
    let parents = vec![left_parent, right_parent];
    let first_child = add_interface(&mut lowerer, "FirstChild", parents.clone());
    let second_child = add_interface(&mut lowerer, "SecondChild", parents);
    let callable = parameter(40, 0);
    let mut session = InferenceSession::new();
    let environment = session.add_environment(&[], std::slice::from_ref(&callable));
    let variable = session.callable_variables(environment)[0];
    for (index, ty) in [first_child, second_child].into_iter().enumerate() {
        session.push(
            Constraint::Subtype(ty.into(), variable.into()),
            ConstraintOrigin::Argument(super::arguments::SourceInputId::from_test_index(index)),
        );
    }

    let failure = lowerer
        .solve_constraints(&session)
        .expect_err("two incomparable interface minima are ambiguous");
    let ConstraintFailureKind::NoUniqueSolution {
        solution_frontier, ..
    } = failure.kind
    else {
        panic!("expected a no-unique-solution failure")
    };
    assert_eq!(solution_frontier.len(), 2);
    assert!(solution_frontier.contains(&left_parent));
    assert!(solution_frontier.contains(&right_parent));
    assert!(!solution_frontier.contains(&lowerer.any));
}

#[test]
fn function_variance_generates_bidirectional_bounds() {
    let mut lowerer = nominals::lowerer();
    let callable = parameter(50, 0);
    let parameter_ty = lowerer.intern_type(Type::Param(callable.id));
    let int = lowerer.integer_type(hir::IntegerKind::SIGNED_32);
    let actual = lowerer.intern_function_type(false, vec![int], int);
    let expected = lowerer.intern_function_type(false, vec![parameter_ty], parameter_ty);
    let mut session = InferenceSession::new();
    let environment = session.add_environment(&[], std::slice::from_ref(&callable));
    let variable = session.callable_variables(environment)[0];
    session.push(
        Constraint::Subtype(actual.into(), TypeTerm::Type(expected)),
        ConstraintOrigin::Argument(super::arguments::SourceInputId::from_test_index(0)),
    );

    let solution = lowerer
        .solve_constraints(&session)
        .expect("function parameter and return variance agree on Int");
    assert_eq!(
        solution.type_for(variable),
        lowerer.integer_type(hir::IntegerKind::SIGNED_32)
    );
}

#[test]
fn duplicate_lower_bounds_are_complete_constraints() {
    let mut lowerer = nominals::lowerer();
    let callable = parameter(51, 0);
    let mut session = InferenceSession::new();
    let environment = session.add_environment(&[], std::slice::from_ref(&callable));
    let variable = session.callable_variables(environment)[0];
    for index in 0..2 {
        session.push(
            Constraint::Subtype(lowerer.string.into(), variable.into()),
            ConstraintOrigin::Argument(super::arguments::SourceInputId::from_test_index(index)),
        );
    }

    let solution = lowerer
        .solve_constraints(&session)
        .expect("duplicate materialized bounds do not mean an unresolved bound");
    assert_eq!(solution.type_for(variable), lowerer.string);
}

#[test]
fn an_upper_only_constraint_chooses_its_unique_greatest_solution() {
    let mut lowerer = nominals::lowerer();
    let callable = parameter(55, 0);
    let mut session = InferenceSession::new();
    let environment = session.add_environment(&[], std::slice::from_ref(&callable));
    let variable = session.callable_variables(environment)[0];
    session.push(
        Constraint::Subtype(
            variable.into(),
            lowerer.integer_type(hir::IntegerKind::SIGNED_32).into(),
        ),
        ConstraintOrigin::Argument(super::arguments::SourceInputId::from_test_index(0)),
    );

    let solution = lowerer
        .solve_constraints(&session)
        .expect("a contravariant occurrence can determine its upper bound");
    assert_eq!(
        solution.type_for(variable),
        lowerer.integer_type(hir::IntegerKind::SIGNED_32)
    );
}

#[test]
fn rigid_outer_parameters_survive_nested_application_inference() {
    let mut lowerer = nominals::lowerer();
    let callable = parameter(52, 0);
    let outer = parameter(53, 0);
    lowerer.type_params_in_scope = vec![outer.clone()];
    let structure = add_generic_struct(&mut lowerer, "Box", callable.clone());
    let expected = lowerer.structs[structure].self_application;
    let expected = lowerer.struct_applications[expected].canonical_type;
    let outer_ty = lowerer.intern_type(Type::Param(outer.id));
    let actual = lowerer.struct_application(structure, vec![outer_ty]);
    let mut session = InferenceSession::new();
    let environment = session.add_environment(&[], std::slice::from_ref(&callable));
    let variable = session.callable_variables(environment)[0];
    session.push(
        Constraint::Subtype(TypeTerm::Rigid(actual), TypeTerm::Type(expected)),
        ConstraintOrigin::Argument(super::arguments::SourceInputId::from_test_index(0)),
    );

    let solution = lowerer
        .solve_constraints(&session)
        .expect("an outer parameter is a rigid input, not the candidate variable");
    assert_eq!(solution.type_for(variable), outer_ty);
}

#[test]
fn lower_bound_solution_is_not_widened_to_satisfy_a_kind() {
    let mut lowerer = nominals::lowerer();
    let callable = parameter(54, 0);
    let mut session = InferenceSession::new();
    let environment = session.add_environment(&[], std::slice::from_ref(&callable));
    let variable = session.callable_variables(environment)[0];
    session.push(
        Constraint::Subtype(
            lowerer.integer_type(hir::IntegerKind::SIGNED_32).into(),
            variable.into(),
        ),
        ConstraintOrigin::Argument(super::arguments::SourceInputId::from_test_index(0)),
    );
    session.push(
        Constraint::Kind(variable.into(), hir::TypeParamKind::Ref),
        ConstraintOrigin::TypeParameterBound(callable.id),
    );

    let failure = lowerer
        .solve_constraints(&session)
        .expect_err("a value argument cannot infer boxed Any for a ref parameter");
    assert!(matches!(
        failure.kind,
        ConstraintFailureKind::Kind { solution, .. }
            if solution == lowerer.integer_type(hir::IntegerKind::SIGNED_32)
    ));
}

#[test]
fn exact_solution_still_has_to_satisfy_kind_bounds() {
    let mut lowerer = nominals::lowerer();
    let callable = parameter(60, 0);
    let mut session = InferenceSession::new();
    let environment = session.add_environment(&[], std::slice::from_ref(&callable));
    let variable = session.callable_variables(environment)[0];
    session.push(
        Constraint::Equal(variable.into(), lowerer.string.into()),
        ConstraintOrigin::ExplicitTypeArgument(0),
    );
    session.push(
        Constraint::Kind(variable.into(), hir::TypeParamKind::Value),
        ConstraintOrigin::TypeParameterBound(callable.id),
    );

    let failure = lowerer
        .solve_constraints(&session)
        .expect_err("String does not satisfy a value bound");
    assert!(matches!(failure.kind, ConstraintFailureKind::Kind { .. }));
}

#[test]
fn exact_solution_still_has_to_satisfy_interface_bounds() {
    let mut lowerer = nominals::lowerer();
    let required = add_interface(&mut lowerer, "Required", Vec::new());
    let callable = parameter(61, 0);
    let mut session = InferenceSession::new();
    let environment = session.add_environment(&[], std::slice::from_ref(&callable));
    let variable = session.callable_variables(environment)[0];
    session.push(
        Constraint::Equal(variable.into(), lowerer.string.into()),
        ConstraintOrigin::ExplicitTypeArgument(0),
    );
    session.push(
        Constraint::Implements(variable.into(), required.into()),
        ConstraintOrigin::TypeParameterBound(callable.id),
    );

    let failure = lowerer
        .solve_constraints(&session)
        .expect_err("String does not implement the test interface");
    assert!(matches!(
        failure.kind,
        ConstraintFailureKind::InterfaceBound { .. }
    ));
}

#[test]
fn concrete_application_materializes_every_argument() {
    let mut lowerer = nominals::lowerer();
    let callable = parameter(62, 0);
    let structure = add_generic_struct(&mut lowerer, "Box", callable.clone());
    let mut session = InferenceSession::new();
    let environment = session.add_environment(&[], std::slice::from_ref(&callable));
    let variable = session.callable_variables(environment)[0];
    session.push(
        Constraint::Equal(
            variable.into(),
            lowerer.integer_type(hir::IntegerKind::SIGNED_32).into(),
        ),
        ConstraintOrigin::Receiver,
    );
    session.push(
        Constraint::ConcreteApplication(super::constraints::NominalApplication::Struct(
            structure,
            vec![variable.into()],
        )),
        ConstraintOrigin::Specificity,
    );

    lowerer
        .solve_constraints(&session)
        .expect("the complete Box<Int> application materializes");
    assert!(lowerer.struct_application_by_key.contains_key(&(
        structure,
        vec![lowerer.integer_type(hir::IntegerKind::SIGNED_32)],
    )));
}

#[test]
fn pointer_concrete_application_uses_the_typed_pointer_representation() {
    let mut lowerer = nominals::lowerer();
    let callable = parameter(63, 0);
    let pointer = add_generic_struct(&mut lowerer, "Ptr", callable.clone());
    lowerer.ffi_ptr = Some(pointer);
    let mut session = InferenceSession::new();
    let environment = session.add_environment(&[], std::slice::from_ref(&callable));
    let variable = session.callable_variables(environment)[0];
    session.push(
        Constraint::Equal(
            variable.into(),
            lowerer.integer_type(hir::IntegerKind::SIGNED_32).into(),
        ),
        ConstraintOrigin::Receiver,
    );
    session.push(
        Constraint::ConcreteApplication(super::constraints::NominalApplication::Struct(
            pointer,
            vec![variable.into()],
        )),
        ConstraintOrigin::Specificity,
    );

    lowerer
        .solve_constraints(&session)
        .expect("the complete Ptr<Int> application materializes");
    assert!(
        lowerer
            .types
            .iter()
            .any(|(_, ty)| matches!(ty, Type::Ptr(pointee)
                if *pointee == lowerer.integer_type(hir::IntegerKind::SIGNED_32)))
    );
    assert!(!lowerer.struct_application_by_key.contains_key(&(
        pointer,
        vec![lowerer.integer_type(hir::IntegerKind::SIGNED_32)],
    )));
}

#[test]
fn function_pointer_concrete_application_requires_a_function_type() {
    let mut lowerer = nominals::lowerer();
    let callable = parameter(64, 0);
    let pointer = add_generic_struct(&mut lowerer, "FunPtr", callable.clone());
    lowerer.ffi_fun_ptr = Some(pointer);
    let int = lowerer.integer_type(hir::IntegerKind::SIGNED_32);
    let function = lowerer.intern_function_type(false, vec![int], int);
    let mut session = InferenceSession::new();
    let environment = session.add_environment(&[], std::slice::from_ref(&callable));
    let variable = session.callable_variables(environment)[0];
    session.push(
        Constraint::Equal(variable.into(), function.into()),
        ConstraintOrigin::Receiver,
    );
    session.push(
        Constraint::ConcreteApplication(super::constraints::NominalApplication::Struct(
            pointer,
            vec![variable.into()],
        )),
        ConstraintOrigin::Specificity,
    );

    lowerer
        .solve_constraints(&session)
        .expect("the complete FunPtr application materializes");
    let Type::Function(signature) = lowerer.types[function] else {
        panic!("the test function type is canonical")
    };
    assert!(
        lowerer
            .types
            .iter()
            .any(|(_, ty)| matches!(ty, Type::FunPtr(found) if *found == signature))
    );
}

#[test]
fn callable_shape_keeps_managed_and_native_categories_separate() {
    let mut lowerer = nominals::lowerer();
    let int = lowerer.integer_type(hir::IntegerKind::SIGNED_32);
    let signature = lowerer.intern_function_type(false, vec![int], int);
    let Type::Function(signature) = lowerer.types[signature] else {
        panic!("interned signature is a managed function")
    };
    let native = lowerer.intern_type(Type::FunPtr(signature));
    let mut session = InferenceSession::new();
    session.push(
        Constraint::CallableShape(
            CallableShape {
                category: CallableCategory::Managed,
                is_suspend: false,
                parameters: vec![CallableParameter::Contextual],
                return_type: CallableReturn::Contextual,
            },
            TypeTerm::Type(native),
        ),
        ConstraintOrigin::ExpectedResult,
    );

    let failure = lowerer
        .solve_constraints(&session)
        .expect_err("managed callable shape cannot target FunPtr");
    assert_eq!(
        failure.kind,
        ConstraintFailureKind::CallableShape(CallableShapeMismatch::ExpectedCallable)
    );
}

#[test]
fn native_callable_shape_checks_explicit_parameter_and_return_types() {
    let mut lowerer = nominals::lowerer();
    let int = lowerer.integer_type(hir::IntegerKind::SIGNED_32);
    let signature = lowerer.intern_function_type(false, vec![int], int);
    let Type::Function(signature) = lowerer.types[signature] else {
        panic!("interned signature is a managed function")
    };
    let native = lowerer.intern_type(Type::FunPtr(signature));
    let mut session = InferenceSession::new();
    session.push(
        Constraint::CallableShape(
            CallableShape {
                category: CallableCategory::Native,
                is_suspend: false,
                parameters: vec![CallableParameter::Explicit(
                    lowerer.integer_type(hir::IntegerKind::SIGNED_32).into(),
                )],
                return_type: CallableReturn::Explicit(
                    lowerer.integer_type(hir::IntegerKind::SIGNED_32).into(),
                ),
            },
            TypeTerm::Type(native),
        ),
        ConstraintOrigin::ExpectedResult,
    );

    lowerer
        .solve_constraints(&session)
        .expect("an exact native signature is applicable");
}

#[test]
fn a_variable_from_another_session_is_rejected() {
    let mut lowerer = nominals::lowerer();
    let callable = parameter(70, 0);
    let mut first = InferenceSession::new();
    first.add_environment(&[], std::slice::from_ref(&callable));
    let mut second = InferenceSession::new();
    let environment = second.add_environment(&[], std::slice::from_ref(&callable));
    let foreign = second.callable_variables(environment)[0];
    first.push(
        Constraint::Equal(
            foreign.into(),
            lowerer.integer_type(hir::IntegerKind::SIGNED_32).into(),
        ),
        ConstraintOrigin::Declaration,
    );

    let failure = lowerer
        .solve_constraints(&first)
        .expect_err("sessions do not share inference variables");
    assert!(matches!(
        failure.kind,
        ConstraintFailureKind::ForeignVariable(_)
    ));
}
