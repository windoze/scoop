use scoop_hir as hir;

use super::*;

#[test]
fn function_types_are_canonical_and_keep_suspend_identity() {
    let ordinary = ty_function(false, vec![ty_named("Int")], ty_named("String"));
    let suspend = ty_function(true, vec![ty_named("Int")], ty_named("String"));
    let module = lower_user(file(vec![
        fun_sig(
            "use",
            vec![],
            vec![
                ("first", ordinary.clone()),
                ("second", ordinary),
                ("task", suspend),
            ],
            None,
            vec![],
        ),
        fun("main", vec![]),
    ]))
    .expect("function types should lower");

    let use_ = module
        .top_level
        .iter()
        .map(|id| &module.functions[*id])
        .find(|function| function.name == "use")
        .expect("use function");
    assert_eq!(use_.params[0].ty, use_.params[1].ty);
    assert_ne!(use_.params[0].ty, use_.params[2].ty);
    assert_eq!(module.function_types.len(), 2);
    assert_eq!(
        hir::type_name(&module, use_.params[0].ty),
        "(Int) -> String"
    );
    assert_eq!(
        hir::type_name(&module, use_.params[2].ty),
        "suspend (Int) -> String"
    );
}

#[test]
fn function_types_substitute_type_parameters_recursively() {
    let generic = ty_function(false, vec![ty_named("T")], ty_named("T"));
    let module = lower_user(file(vec![
        fun_sig("consume", vec!["T"], vec![("op", generic)], None, vec![]),
        fun("main", vec![]),
    ]))
    .expect("generic function type should lower");

    let consume = module
        .top_level
        .iter()
        .map(|id| &module.functions[*id])
        .find(|function| function.name == "consume")
        .expect("consume function");
    let hir::Type::Function(id) = module.types[consume.params[0].ty] else {
        panic!("expected function type");
    };
    let signature = &module.function_types[id];
    assert!(matches!(
        module.types[signature.parameter_types[0]],
        hir::Type::Param(_)
    ));
    assert_eq!(signature.parameter_types[0], signature.return_type);
}

#[test]
fn function_types_are_reference_types_for_gc_constraints() {
    let mut lowerer = Lowerer::new();
    let ty = lowerer.intern_function_type(false, vec![lowerer.int], lowerer.string);
    assert!(lowerer.is_ref_ty(ty));
    assert!(!lowerer.is_value_ty(ty));
}
