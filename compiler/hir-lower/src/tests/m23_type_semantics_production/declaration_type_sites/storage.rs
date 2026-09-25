use super::*;
use scoop_identity::{ExactTypeKey, SourceDeclarationKey};

#[test]
fn storage_and_generated_type_sites_cover_actual_materialized_dependencies() {
    for name in [
        "storage-standalone",
        "storage-combined",
        "declaration-combined",
    ] {
        with_hir_source(&source(name), |output, _| {
            let local = &output.output().local;
            let interface = public_interface(output);
            let mut canonical =
                hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
            canonical
                .complete_cross_cone_interface_source_points(
                    output.output().export.module(),
                    &interface,
                )
                .unwrap();
            let mut identities =
                source_inventory::identity_closure_for_foundation(output, canonical.clone());
            let foundation = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
            let mut counts = [0; 4];
            for site in interface
                .external_references()
                .records()
                .iter()
                .flat_map(|r| r.type_sites().records())
            {
                let index = match site {
                    Site::FieldStorage { .. } => 0,
                    Site::EnumVariantFieldStorage { .. } => 1,
                    Site::ConstructorInitializerResult { .. } => 2,
                    Site::InitializationCycleMessage { .. } => 3,
                    _ => continue,
                };
                counts[index] += 1;
                foundation
                    .validate_declaration_type_position(local.cone, site.position())
                    .unwrap();
                assert!(
                    foundation
                        .validate_declaration_type_position(ConeIdentity::CORE, site.position())
                        .is_err()
                );
                let bytes = scoop_wire::encode(site).unwrap();
                let raw: hir::DecodedHirDependencyTypeSiteV1 =
                    scoop_wire::decode_canonical(&bytes).unwrap();
                assert_eq!(raw.resolve(&mut identities).unwrap(), *site);
            }
            if name == "storage-standalone" {
                assert_eq!(counts, [0, 0, 1, 0]);
            } else if name == "storage-combined" {
                assert!(counts.iter().all(|count| *count > 0), "{counts:?}");
            }
            let actual = local
                .materialized_type_closure()
                .unwrap()
                .into_iter()
                .filter_map(|ty| {
                    let exact = &local.exact_type_identities[ty];
                    let ExactTypeKey::Nominal(owner) = exact.key() else {
                        return None;
                    };
                    let source = identities
                        .canonical_key::<_, SourceDeclarationKey>(*owner)
                        .unwrap();
                    (source.origin() != local.cone).then_some((source.origin(), exact.id()))
                })
                .collect::<BTreeSet<_>>();
            let shared = interface
                .external_references()
                .materialized_type_dependencies(local.cone, &identities)
                .unwrap();
            assert_eq!(
                shared.into_iter().collect::<BTreeSet<_>>(),
                actual,
                "{name}"
            );
        });
    }
}

#[test]
fn initializer_type_position_rejects_a_function_and_a_value_constructor() {
    with_hir_source(&source("storage-combined"), |output, _| {
        let local = &output.output().local;
        let foundation = hir::OdrFreeHirFoundation::try_new(
            hir::CanonicalHirFoundation::from_dependency_output(output).unwrap(),
        )
        .unwrap();
        let roots = local
            .functions
            .iter()
            .map(|(_, f)| f.materialization)
            .chain(
                local
                    .struct_constructors
                    .iter()
                    .map(|(_, c)| c.materialization),
            );
        for root in roots {
            assert!(
                foundation
                    .validate_declaration_type_position(
                        local.cone,
                        Position::ConstructorInitializerResult(root)
                    )
                    .is_err()
            );
        }
    });
}
