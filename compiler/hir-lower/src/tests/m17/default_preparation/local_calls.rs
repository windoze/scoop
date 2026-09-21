use super::*;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/nested-local-calls.scoop"
));

#[test]
fn defaults_carry_their_own_local_callables_without_requiring_external_lookup() {
    let output = lower_source(SOURCE).unwrap();
    let export = output.export.module();
    for name in ["localCall", "localCallGeneric"] {
        let (id, _) = export
            .functions
            .iter()
            .find(|(_, f)| f.name == name)
            .unwrap();
        let interface = export
            .source_parameter_interfaces
            .iter()
            .find(|i| i.owner == hir::ExportParameterOwner::Function(id))
            .unwrap();
        let hir::ExportParameterCalling::Default { source, .. } = interface.parameters[1].calling
        else {
            panic!("expected default source");
        };
        let body = &export.export_default_exprs[export.export_default_sources[source].expression];
        let mut declarations = std::collections::HashSet::new();
        for reference in &body.references.callables {
            if let hir::ExportDefaultCallableTarget::LocalFunction(local) = reference.target {
                declarations.insert(export.local_functions[local].function);
            }
        }
        assert_eq!(declarations.len(), 1, "{name}");
        let callees = body
            .references
            .callables
            .iter()
            .filter(|reference| {
                matches!(
                    reference.target,
                    hir::ExportDefaultCallableTarget::Callable(_)
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(callees.len(), 1, "{name}");
        assert_eq!(
            callees[0].witness.target_domain,
            hir::AccessDomain::universal()
        );
        let hir::ExportDefaultCallableTarget::Callable(callee) = callees[0].target else {
            unreachable!()
        };
        let function = match callee {
            hir::Callable::Function(f) => f,
            hir::Callable::Generic(i) => {
                export.generic_functions[export.instantiations[i].generic].function
            }
            _ => panic!("expected a local source callable"),
        };
        assert!(declarations.contains(&function));
        assert_ne!(
            export.functions[function].access.lookup.0,
            hir::AccessDomain::universal()
        );
    }
    let mir = scoop_mir_lower::lower(&output.local).unwrap();
    assert_eq!(
        selected(&hir::dump(&output.export)),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/nested-local-calls.hir.snap"
        )),
    );
    assert_eq!(
        selected(&scoop_mir::dump(&mir)),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/nested-local-calls.mir.snap"
        )),
    );
}

#[test]
fn a_local_default_declaration_does_not_grant_access_to_external_or_same_named_callables() {
    for source in [
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/errors/local-call-external.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/errors/local-call-same-name.scoop"
        )),
    ] {
        let diagnostics = lower_source(source).unwrap_err();
        let failures = diagnostics.iter().filter(|d| d.message == "default expression references a callable outside the callable's complete call domain").collect::<Vec<_>>();
        assert_eq!(failures.len(), 1, "{diagnostics:?}");
        let use_text = if source.contains("hidden()") {
            "hidden()"
        } else {
            "copy()"
        };
        let start = source.rfind(use_text).unwrap() as u32;
        assert_eq!(failures[0].file, 1);
        assert_eq!(
            failures[0].span,
            Some(ast::Span::new(start, start + use_text.len() as u32))
        );
    }
}

fn selected(dump: &str) -> String {
    let mut keep = false;
    let mut result = String::new();
    for line in dump.lines() {
        if line.starts_with("  ") && !line.starts_with("   ") {
            keep = line.contains("localCall")
                || line.contains("LocalCallHost")
                || line.contains("$local.")
                || line.starts_with("  fun main");
        }
        if keep {
            result.push_str(line);
            result.push('\n');
        }
    }
    result
}
