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
        let body = &export.export_default_exprs
            [export.export_default_sources[source].declared().unwrap().0];
        let mut declarations = std::collections::HashSet::new();
        for reference in &body.references.callables {
            if let hir::ExportDefaultCallableTarget::LocalFunction(local) = reference.target {
                declarations.insert(export.local_functions[local].source_function());
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
        assert_eq!(callees[0].target_domain, hir::AccessDomain::universal());
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
    scoop_mir_lower::lower(&output.local).unwrap();
}
