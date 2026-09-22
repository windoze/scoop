use super::*;

#[test]
fn actual_default_publication_reuses_source_profiles_for_generic_static_and_inherited_members() {
    let (root, source) = fixture("publication");
    with_hir_source(&source, |output, _| {
        let source = Production::from_dependency_hir(output, &mut meter()).unwrap();
        assert_eq!(
            published_profiles(output, source.profiles()),
            std::fs::read_to_string(root.join("publication.snap")).unwrap()
        );
    });
}

fn published_profiles(output: &hir::DependencyHirOutput, profiles: &Profiles) -> String {
    let candidate = produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
    let export = output.output().export.module();
    let labels = export
        .source_parameter_interfaces
        .iter()
        .map(|p| (declaration(export, p.owner), owner_name(export, p.owner)))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut lines = Vec::new();
    for template in candidate.section().protected_defaults().records() {
        let expected = profiles.get(template.key()).unwrap().profile();
        let r = template.references();
        let witnesses = r
            .callables()
            .iter()
            .map(|r| r.witness())
            .chain(r.constructors().iter().map(|r| r.witness()))
            .chain(r.types().iter().map(|r| r.witness()))
            .chain(r.globals().iter().map(|r| r.witness()))
            .chain(r.singleton_values().iter().map(|r| r.witness()))
            .chain(r.fields().iter().map(|r| r.witness()))
            .collect::<Vec<_>>();
        assert!(!witnesses.is_empty(), "{:?}", template.key());
        for witness in witnesses {
            let actual = match witness.view() {
                hir::ProtectedDefaultAccessWitnessViewV1::ParamFree(_) => {
                    hir::ProtectedDefaultWitnessSourceProfileV1::ParamFree
                }
                hir::ProtectedDefaultAccessWitnessViewV1::GenericSourceMetadata { .. } => {
                    hir::ProtectedDefaultWitnessSourceProfileV1::GenericSourceMetadata
                }
            };
            assert_eq!(actual, expected, "{:?}", template.key());
        }
        lines.push(format!(
            "{}[{}]: {:?}",
            labels[&template.key().owner()],
            template.key().parameter_position(),
            expected
        ));
    }
    lines.sort();
    lines.join("\n") + "\n"
}
