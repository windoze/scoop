use super::*;
use hir::{
    CanonicalDefaultSourceProfilesV1 as Profiles,
    DecodedCanonicalDefaultSourceProfilesV1 as DecodedProfiles,
};

mod publication;
mod wire;

fn fixture(name: &str) -> (std::path::PathBuf, String) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-default-source-profiles");
    let source = std::fs::read_to_string(root.join(format!("{name}.scoop"))).unwrap();
    (root, source)
}

fn restore(output: &hir::DependencyHirOutput, profiles: &Profiles) -> Profiles {
    let bytes = encode(profiles).unwrap();
    let decoded: DecodedProfiles = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    let restored = decoded.resolve(&mut identity_closure(output)).unwrap();
    assert_eq!(encode(&restored).unwrap(), bytes);
    restored
}

#[test]
fn actual_default_source_profiles_preserve_static_nested_and_own_generic_access_proofs() {
    for name in ["standalone", "combined"] {
        let (root, text) = fixture(name);
        with_hir_source(&text, |output, _| {
            let source = Production::from_dependency_hir(output).unwrap();
            assert_eq!(&restore(output, source.profiles()), source.profiles());
            source
                .profiles()
                .validate_template_coverage(source.templates())
                .unwrap();
            assert_eq!(
                source,
                Production::from_export_hir(&output.output().export).unwrap()
            );
            let export = output.output().export.module();
            let labels = export
                .source_parameter_interfaces
                .iter()
                .map(|p| (declaration(export, p.owner), owner_name(export, p.owner)))
                .collect::<std::collections::BTreeMap<_, _>>();
            let mut lines = source
                .profiles()
                .records()
                .iter()
                .map(|record| {
                    let key = record.key();
                    let body = source.templates().get(key).unwrap();
                    format!(
                        "{}[{}]: {:?} inherited={}",
                        labels[&key.owner()],
                        key.parameter_position(),
                        record.profile(),
                        key.owner() != body.definition_root().declaration()
                    )
                })
                .collect::<Vec<_>>();
            lines.sort();
            assert_eq!(
                lines.join("\n") + "\n",
                std::fs::read_to_string(root.join(format!("{name}.snap"))).unwrap()
            );
            wire::check(output, &source);
        });
    }
}

#[test]
fn actual_default_source_profile_inventory_rejects_missing_extra_and_duplicate_records() {
    let (_, text) = fixture("standalone");
    with_hir_source(&text, |output, _| {
        let source = Production::from_dependency_hir(output).unwrap();
        let records = source.profiles().records();
        assert!(matches!(
            Profiles::try_new(vec![records[0], records[0]]),
            Err(hir::SourceInventoryError::NonCanonicalOrder { .. })
        ));
        let incomplete = Profiles::try_new(records[1..].to_vec()).unwrap();
        assert!(
            matches!(incomplete.validate_template_coverage(source.templates()), Err(hir::SourceInventoryError::MissingDefaultProfile(key)) if key == records[0].key())
        );
        assert!(
            matches!(source.profiles().validate_template_coverage(&Table::default()), Err(hir::SourceInventoryError::UnexpectedDefaultProfile(key)) if key == records[0].key())
        );

        Production::from_dependency_hir(output).unwrap();
    });
}
