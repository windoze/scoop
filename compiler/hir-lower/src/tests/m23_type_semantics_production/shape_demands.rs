use super::*;
use scoop_identity::DeclarationName;
use source_dispatch::with_hir_source;

const FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-source-only-nominals/shape-demand.scoop"
));

#[test]
fn shape_demands_select_the_actual_exporter_in_aggregated_source_graphs() {
    let internal = file(vec![struct_decl("InternalShape", Vec::new())]);
    let mut public = file(vec![struct_decl("LocalShape", Vec::new())]);
    make_core_public(&mut public);
    for (mut source, expected) in [(internal, 0), (public, 1)] {
        source.declarations.push(fun("main", Vec::new()));
        let output = lower(&[core_file(), source]).unwrap();
        let roots = output.local.materialization().roots();
        assert_eq!(roots.len(), expected);
        for root in roots {
            assert_eq!(root.declaration().origin(), output.export.cone);
            assert!(matches!(
                root.declaration().name(),
                DeclarationName::Named(name) if name.as_str() == "LocalShape"
            ));
        }
    }
}

#[test]
fn source_only_shape_demands_replay_shared_declarations_before_and_after_concretization() {
    for source in [
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-source-only-nominals/standalone.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-source-only-nominals/combined.scoop"
        )),
        FIXTURE,
    ] {
        with_hir_source(source, |output, _| {
            let public = public_interface(output);
            let hir = output.output();
            let foundation = hir::CanonicalHirFoundation::from_modules(
                &hir.export,
                &hir.local,
                &hir.native_boundary_types,
            )
            .unwrap();
            let direct =
                hir::CanonicalDirectPublicSurfaceV1::from_export_hir(hir.export.module()).unwrap();
            let decoded = hir::PublicNominalShapeRequirementsV1::from_shared_surface(
                hir.export.cone,
                &direct,
                &foundation,
                public.nominal_interfaces(),
                public.callable_interfaces(),
            )
            .unwrap();
            let projected =
                hir::PublicNominalShapeRequirementsV1::from_export_hir(hir.export.module())
                    .unwrap();
            assert_eq!(projected, decoded);
            assert_eq!(
                projected
                    .roots()
                    .iter()
                    .map(|root| root.source())
                    .collect::<Vec<_>>(),
                hir.local
                    .materialization()
                    .roots()
                    .iter()
                    .map(|root| root.source())
                    .collect::<Vec<_>>()
            );
            let mut names = projected
                .source_declarations(&foundation)
                .unwrap()
                .iter()
                .map(|declaration| match declaration.name() {
                    DeclarationName::Named(name) => name.as_str().to_owned(),
                    _ => panic!("source nominal name"),
                })
                .collect::<Vec<_>>();
            assert!(!names.is_empty());
            assert!(
                names.iter().all(|name| name.starts_with("Ready")),
                "{names:?}"
            );
            if source == FIXTURE {
                names.sort();
                assert_eq!(
                    names.join("\n") + "\n",
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../tests/fixtures/m23-source-only-nominals/shape-demand.snap"
                    ))
                );
            }
        });
    }
}

#[test]
fn source_only_shape_demands_reject_missing_public_declarations() {
    with_hir_source(FIXTURE, |output, _| {
        let hir = output.output();
        let public = public_interface(output);
        let foundation = hir::CanonicalHirFoundation::from_modules(
            &hir.export,
            &hir.local,
            &hir.native_boundary_types,
        )
        .unwrap();
        let direct =
            hir::CanonicalDirectPublicSurfaceV1::from_export_hir(hir.export.module()).unwrap();
        assert!(matches!(
            hir::PublicNominalShapeRequirementsV1::from_shared_surface(
                hir.export.cone,
                &direct,
                &foundation,
                &hir::CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap(),
                public.callable_interfaces(),
            ),
            Err(hir::PublicNominalShapeProjectionError::Materialization(
                hir::NominalMaterializationClosureError::MissingNominal(_)
            ))
        ));
    });
}
