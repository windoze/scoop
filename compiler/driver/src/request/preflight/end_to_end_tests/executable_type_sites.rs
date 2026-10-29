use super::*;
use scoop_hir::{ExternalHirReferenceRoleV1, HirExpressionTypeRoleV1};

#[test]
fn published_type_occurrences_retain_imported_evaluation_and_definition_sites() {
    let target = resolved_target().expect("type occurrence artifacts require a host target");
    let sysroot = tempfile::tempdir().unwrap();
    let fixtures = crate::workspace_root().join("tests/fixtures/language/executable-type-sites");
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
    for name in [
        "standalone",
        "combined",
        "declaration-standalone",
        "declaration-combined",
        "storage-standalone",
        "storage-combined",
    ] {
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
        .build_and_publish()
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
            &mut session,
        )
        .unwrap();
        let provider_identity = coordinate.identity().unwrap();
        let provider_bodies = closure
            .semantic()
            .direct_provider(provider_identity)
            .unwrap()
            .production()
            .hir_interface()
            .generic_callable_bodies()
            .records()
            .iter()
            .map(|body| body.owner().template_owner())
            .collect::<std::collections::BTreeSet<_>>();
        let production = closure.current_compile().production();
        let references = production.hir_interface().external_references().records();
        let mut roles = std::collections::BTreeSet::new();
        let mut foreign_definitions = 0;
        let mut imported_body_sites = 0;
        let mut sites = 0;
        let mut declarations = [0_usize; 8];
        for reference in references {
            assert_eq!(
                reference
                    .roles()
                    .contains(ExternalHirReferenceRoleV1::ExecutableTypeDependency),
                !reference.type_sites().is_empty()
            );
            for site in reference.type_sites().records() {
                use scoop_hir::HirDependencyTypeSiteV1 as Site;
                let expression = site.as_expression();
                if let Some(expression) = expression {
                    if provider_bodies.contains(&expression.position().root.template()) {
                        assert_eq!(
                            expression.origin().evaluation().source().cone(),
                            provider_identity
                        );
                        assert_eq!(
                            expression.origin().definition().source().cone(),
                            provider_identity
                        );
                        imported_body_sites += 1;
                        continue;
                    }
                    // Materialized core generic bodies retain their provider
                    // positions. Count the fixture's own evaluation sites below.
                    if expression.origin().evaluation().source().cone() == ConeIdentity::CORE {
                        assert_eq!(
                            expression.origin().definition().source().cone(),
                            ConeIdentity::CORE
                        );
                        continue;
                    }
                    roles.insert(expression.role());
                } else {
                    if core_declaration(
                        site,
                        production.layout().hir_foundation(),
                        closure
                            .semantic()
                            .direct_provider(ConeIdentity::CORE)
                            .unwrap()
                            .production()
                            .layout()
                            .hir_foundation(),
                    ) {
                        continue;
                    }
                    let index = match site {
                        Site::CallableSignature { .. } => 0,
                        Site::LocalValue { .. } => 1,
                        Site::BackingStorage { .. } => 2,
                        Site::DelegateStorage { .. } | Site::GenericDelegateStorage { .. } => 3,
                        Site::FieldStorage { .. } => 4,
                        Site::EnumVariantFieldStorage { .. } => 5,
                        Site::ConstructorInitializerResult { .. } => 6,
                        Site::InitializationCycleMessage { .. } => 7,
                        Site::Expression(_) => unreachable!("expression branch handled above"),
                    };
                    declarations[index] += 1;
                    continue;
                }
                let site = expression.unwrap();
                sites += 1;
                let foreign = site.origin().definition().source().cone() != current;
                foreign_definitions += usize::from(foreign);
                assert_eq!(site.origin().evaluation().source().cone(), current);
            }
        }
        assert_eq!(imported_body_sites > 0, name == "combined");
        if name == "declaration-standalone" {
            assert_eq!(sites, 0);
            assert_eq!(declarations, [2, 1, 0, 0, 0, 0, 0, 0]);
        } else if name == "declaration-combined" {
            assert!(sites > 0);
            assert!(declarations[0] > 2 && declarations[1] > 1 && declarations[2] > 0);
            assert!(declarations[3] > 0);
        } else if name == "storage-standalone" {
            assert_eq!(declarations[4..], [0, 0, 1, 0]);
        } else if name == "storage-combined" {
            assert!(declarations[4..].iter().all(|count| *count > 0));
        } else if name == "standalone" {
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
    }
}

fn core_declaration(
    site: &scoop_hir::HirDependencyTypeSiteV1,
    current: &scoop_hir::CanonicalHirFoundation,
    core: &scoop_hir::CanonicalHirFoundation,
) -> bool {
    use scoop_hir::HirDependencyTypeSiteV1 as Site;
    use scoop_identity::{
        CallableTemplateOwner as Owner, DefinitionOriginSubject as Subject, PropertyOwner,
    };
    let callable = |root: scoop_identity::CallableMaterialization| match root.template() {
        Owner::Function(id) => Some(Subject::Function(id)),
        Owner::GenericFunction(id) => Some(Subject::GenericFunction(id)),
        Owner::Constructor(id) => Some(Subject::Constructor(id)),
        Owner::Accessor(id) => Some(Subject::PropertyAccessor(id)),
        Owner::Generated(id) => Some(Subject::GeneratedCallable(id)),
        Owner::VariantConstructor(id) => Some(Subject::EnumVariant(id)),
        Owner::ReleaseHook(_) => None,
    };
    let subject = match site {
        Site::CallableSignature { root, .. } => callable(*root),
        Site::ConstructorInitializerResult { constructor, .. } => callable(*constructor),
        Site::LocalValue { local, .. } => Some(Subject::LocalValue(*local)),
        Site::BackingStorage { property, .. } | Site::DelegateStorage { property, .. } => {
            Some(match property {
                PropertyOwner::Property(id) => Subject::Property(*id),
                PropertyOwner::ExtensionProperty(id) => Subject::ExtensionProperty(*id),
            })
        }
        Site::FieldStorage { field, .. } => Some(Subject::Field(*field)),
        Site::EnumVariantFieldStorage { field, .. } => Some(Subject::EnumVariantField(*field)),
        Site::InitializationCycleMessage { unit, .. }
        | Site::GenericDelegateStorage { unit, .. } => Some(Subject::InitializationUnit(*unit)),
        Site::Expression(_) => None,
    };
    subject
        .and_then(|subject| {
            current
                .definition_origin(subject)
                .or_else(|| core.definition_origin(subject))
        })
        .is_some_and(|origin| origin.origin().source().cone() == ConeIdentity::CORE)
}
