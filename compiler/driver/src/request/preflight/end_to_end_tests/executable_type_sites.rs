use super::*;
use scoop_hir::{ExternalHirReferenceRoleV1, HirExpressionTypeRoleV1};

#[test]
fn published_type_occurrences_replay_from_bytes_without_dependency_sources() {
    let target = resolved_target().expect("type occurrence artifacts require a host target");
    let sysroot = tempfile::tempdir().unwrap();
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-executable-type-sites");
    let source =
        |name: &str| std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let provider_root = sysroot.path().join("provider");
    let coordinate = ConeCoordinate::new("dev.example", "type-sites-provider", "0.1.0").unwrap();
    write_manifest_cone(
        &provider_root,
        "dev.example",
        "type-sites-provider",
        "library",
        &source("provider"),
    );
    let provider = build_manifest(
        sysroot.path(),
        &target,
        &provider_root,
        &sysroot.path().join("output/provider.slib"),
    );
    let dependency_bytes =
        [&core, &provider].map(|artifact| std::fs::read(artifact.artifact().path()).unwrap());
    std::fs::remove_dir_all(provider_root.join("src")).unwrap();
    for name in ["standalone", "combined"] {
        let root = sysroot.path().join(name);
        write_manifest_cone(&root, "dev.example", name, "library", &source(name));
        write_dependency_manifest(&root, name, &[&coordinate]);
        let artifact = build_manifest_request(
            sysroot.path(),
            &target,
            &root,
            &sysroot.path().join(format!("output/{name}.slib")),
            vec![provider.artifact().path().to_path_buf()],
            vec![],
        )
        .build_and_publish(DecodeLimits::default())
        .unwrap();
        let bytes = std::fs::read(artifact.artifact().path()).unwrap();
        let current = ConeCoordinate::new("dev.example", name, "0.1.0")
            .unwrap()
            .identity()
            .unwrap();
        let mut direct = vec![ConeIdentity::CORE, coordinate.identity().unwrap()];
        direct.sort_unstable();
        let mut session = scoop_identity::SemanticIdentitySession::new();
        let closure = scoop_slib::validate_completed_cross_cone_artifact_closure(
            current,
            target.lir_target_selection(),
            direct,
            dependency_bytes.iter().map(Vec::as_slice).collect(),
            &bytes,
            DecodeLimits::default(),
            target.c_bridge_toolchain().profile(),
            &mut session,
        )
        .unwrap();
        let production = closure.current_compile().production();
        let references = production.hir_interface().external_references().records();
        let mut dump = String::new();
        let mut roles = std::collections::BTreeSet::new();
        let mut foreign_definitions = 0;
        let mut sites = 0;
        for reference in references {
            assert_eq!(
                reference
                    .roles()
                    .contains(ExternalHirReferenceRoleV1::ExecutableTypeDependency),
                !reference.type_sites().is_empty()
            );
            for site in reference.type_sites().records() {
                roles.insert(site.role());
                sites += 1;
                let foreign = site.origin().definition().source().cone() != current;
                foreign_definitions += usize::from(foreign);
                assert_eq!(site.origin().evaluation().source().cone(), current);
                dump.push_str(&format!(
                    "target={:?} position={:?} role={:?} exact={} foreign_definition={foreign}\n",
                    reference.target(),
                    site.position(),
                    site.role(),
                    site.exact()
                ));
            }
        }
        assert!(sites > 0);
        if name == "standalone" {
            assert_eq!(sites, 1);
            assert_eq!(
                roles,
                std::collections::BTreeSet::from([HirExpressionTypeRoleV1::Value])
            );
            assert_eq!(foreign_definitions, 0);
        } else {
            assert!(foreign_definitions > 0);
            assert_eq!(
                roles,
                std::collections::BTreeSet::from([HirExpressionTypeRoleV1::Value])
            );
        }
        dump.push_str(&format!("type_sites={sites}\nforeign_definitions={foreign_definitions}\nMIR selected={}\nLIR selected={}\n",
            production.mir_cross_cone().selected().len(), production.lir_cross_cone().selected().len()));
        let path = fixtures.join(format!("{name}.snap"));
        if std::env::var_os("SCOOP_UPDATE_EXECUTABLE_TYPE_SITES").is_some() {
            std::fs::write(&path, &dump).unwrap();
        }
        assert_eq!(dump, std::fs::read_to_string(path).unwrap());
    }
}
