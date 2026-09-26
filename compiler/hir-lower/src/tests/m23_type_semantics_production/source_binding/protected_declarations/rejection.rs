use super::*;

#[test]
fn protected_wrappers_reject_other_declared_visibilities() {
    with_hir_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let mut rejected = [0; 2];
        for source in sources.constructors.records() {
            if source.declaration_access().declared_visibility()
                != hir::DeclaredVisibilityV1::Protected
            {
                assert!(hir::ProtectedConstructorInterfaceV1::try_from(source.clone()).is_err());
                rejected[0] += 1;
            }
        }
        for source in sources.members.properties.records() {
            if source.declaration_access().declared_visibility()
                != hir::DeclaredVisibilityV1::Protected
            {
                assert!(hir::ProtectedPropertyInterfaceV1::try_from(source.clone()).is_err());
                rejected[1] += 1;
            }
        }
        assert!(rejected.into_iter().all(|count| count > 0));
    });
}

#[test]
fn protected_production_has_an_explicit_empty_surface() {
    with_hir_source("public class Plain {}", |output, _| {
        let produced = Production::from_export_hir(&output.output().export).unwrap();
        assert!(produced.required().values().is_empty());
        assert!(produced.declarations().records().is_empty());
        assert!(produced.protocols().records().is_empty());
    });
}
