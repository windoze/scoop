use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-shared-source-calls")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn checked_source_and_decoded_bodies_share_direct_call_targets() {
    for case in ["standalone", "combined"] {
        with_provider_consumer(
            &fixture("provider"),
            &fixture(case),
            |output, _, _, _, _| {
                let export = output.output().export.module();
                let implementations = export
                    .functions
                    .values()
                    .map(|function| &function.kind)
                    .chain(
                        export
                            .imported_generic_templates
                            .values()
                            .map(|template| &template.implementation),
                    );
                let mut targets = [false; 3];
                let mut bindings = [false; 2];
                for implementation in implementations {
                    let hir::FunctionKind::User(body) = implementation else {
                        continue;
                    };
                    for statement in &body.statements {
                        let expression = match &statement.kind {
                            hir::StatementKind::ValDecl { init, .. } => init,
                            hir::StatementKind::Return { value: Some(value) }
                            | hir::StatementKind::Expr(value) => value,
                            _ => continue,
                        };
                        let hir::ExprKind::Call {
                            callee, binding, ..
                        } = &expression.kind
                        else {
                            continue;
                        };
                        match callee {
                            hir::CallableTarget::Local(_) => targets[0] = true,
                            hir::CallableTarget::Application(application) => {
                                targets[1] = true;
                                bindings[usize::from(binding.is_some())] = true;
                                assert!(matches!(
                                    export.imported_generic_applications[*application].arguments,
                                    hir::ImportedCallableArguments::Function(_),
                                ));
                            }
                            hir::CallableTarget::Dependency(_) => targets[2] = true,
                        }
                    }
                }
                assert_eq!(
                    targets, [true; 3],
                    "{case}: all selected target kinds remain explicit"
                );
                assert_eq!(
                    bindings, [true; 2],
                    "{case}: direct imports and bound template calls remain distinct uses"
                );
            },
        )
        .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
}

#[test]
fn shared_call_targets_preserve_managed_and_generic_gc_diagnostics() {
    for (case, message) in [
        ("bad-managed", "NoGC"),
        ("bad-ordinary", "NoGC"),
        ("bad-constraint", "GC-free"),
    ] {
        let errors =
            with_provider_consumer(&fixture("provider"), &fixture(case), |_, _, _, _, _| ())
                .expect_err(case);
        assert!(
            errors.iter().any(|error| error.message.contains(message)),
            "{case}: {errors:?}"
        );
        assert!(
            errors
                .iter()
                .all(|error| error.file == 0 && error.span.is_some())
        );
    }
}
