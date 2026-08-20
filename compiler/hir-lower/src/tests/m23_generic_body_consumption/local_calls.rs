use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-shared-local-calls")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn local_calls_keep_complete_capture_arguments_after_recursive_and_default_expansion() {
    use hir::concrete::{Callable, CallableTarget, ExprKind};

    for case in ["standalone", "combined"] {
        with_provider_consumer(
            &fixture("provider"),
            &fixture(case),
            |output, _, _, _, _| {
                let module = output.output().local.module();
                let mut observed = 0;
                module
                    .visit_executable_expressions(|occurrence| {
                        if let ExprKind::Call {
                            callee: CallableTarget::Local(Callable::Function(function)),
                            args,
                            ..
                        } = &occurrence.expression.kind
                        {
                            let function = &module.functions[*function];
                            if !function.capture_parameters.is_empty() {
                                assert_eq!(
                                    args.len(),
                                    function.params.len(),
                                    "{case}: {}",
                                    function.name
                                );
                                for (argument, parameter) in args.iter().zip(&function.params) {
                                    assert_eq!(
                                        argument.ty, parameter.ty,
                                        "{case}: {}",
                                        function.name
                                    );
                                }
                                observed += 1;
                            }
                        }
                        Ok::<_, ()>(())
                    })
                    .unwrap();
                assert!(
                    observed > 0,
                    "{case}: local direct calls retain their hidden captures"
                );
            },
        )
        .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
}
