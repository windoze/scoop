use super::*;
use scoop_hir as hir;
use scoop_identity::{ConeIdentity, PendingIdentityValidation, ValidatedIdentityGraph};
use scoop_wire::{decode_canonical, encode};

mod resources;

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-property-initialization")
}

fn with_shared(
    name: &str,
    run: impl FnOnce(
        &hir::DependencyHirOutput,
        hir::SharedTypeMetadataV1<'_>,
        &[hir::SharedTypeMetadataV1<'_>],
    ),
) {
    let provider = std::fs::read_to_string(
        fixture_path().join("../m23-core-layout-exports/property-initialization-provider.scoop"),
    )
    .unwrap();
    let mut core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "property-provider", "1.0.0").unwrap();
    let (foundation, interface) =
        crate::tests::m23_ordinary_dependencies::support::project_dependency_text(
            &core,
            &coordinate,
            &provider,
            &["Int"],
        );
    let source_foundation = hir::OdrFreeHirFoundation::try_new(foundation.clone()).unwrap();
    let foundation = core.import_dependency_foundation(&coordinate, &foundation, 57);
    let fixture = DependencyPropertyFixture {
        core,
        coordinate,
        foundation,
        source_foundation,
        interface,
        aliases: empty_alias_expansions(),
    };
    let source = std::fs::read_to_string(fixture_path().join(format!("{name}.scoop"))).unwrap();
    let consumer = crate::tests::m23_ordinary_core_only::support::parsed_ordinary_text(&source);
    fixture.inspect_parsed(&consumer, |output| {
        let output = output.unwrap_or_else(|errors| panic!("{name}: {errors:#?}"));
        let provider = fixture.coordinate.identity().unwrap();
        let local = output.output().export.module();
        let world = hir::ImportedSemanticWorld::from_validated_closure(
            local.cone,
            vec![
                fixture.core.provider(),
                hir::DirectImportedProviderInput::from_validated(
                    certificate(&fixture.coordinate, 57),
                    &fixture.foundation,
                    &fixture.interface,
                    &fixture.aliases,
                ),
            ],
            vec![],
        )
        .unwrap();
        let mut foundation =
            hir::CanonicalHirFoundation::from_type_semantics_output(&output).unwrap();
        let public = hir::CrossConeHirInterfaceSectionV1::from_dependency_hir(
            &output,
            &[],
            &mut hir::CrossConeHirProductionAuthority::new(
                &foundation,
                &local.public_export_bindings,
                &world,
            ),
        )
        .unwrap();
        foundation
            .complete_cross_cone_interface_source_points(local, &public)
            .unwrap();
        let foundation = hir::OdrFreeHirFoundation::try_new(foundation).unwrap();
        let graph = identities(
            &[
                fixture.core.source_foundation.as_canonical(),
                fixture.source_foundation.as_canonical(),
                foundation.as_canonical(),
            ],
            &[ConeIdentity::CORE, provider, local.cone],
        );
        let current = hir::SharedTypeMetadataV1 {
            provider: local.cone,
            identities: &graph,
            foundation: &foundation,
            public: &public,
        };
        let dependencies = [
            hir::SharedTypeMetadataV1 {
                provider: ConeIdentity::CORE,
                identities: &graph,
                foundation: &fixture.core.source_foundation,
                public: fixture.core.general_interface(),
            },
            hir::SharedTypeMetadataV1 {
                provider,
                identities: &graph,
                foundation: &fixture.source_foundation,
                public: &fixture.interface,
            },
        ];
        run(&output, current, &dependencies);
    });
}

fn identities(
    foundations: &[&hir::CanonicalHirFoundation],
    providers: &[ConeIdentity],
) -> ValidatedIdentityGraph {
    let decoded = foundations
        .iter()
        .map(|foundation| {
            decode_canonical::<hir::DecodedHirFoundation>(&encode(*foundation).unwrap()).unwrap()
        })
        .collect::<Vec<_>>();
    let mut graphs = Vec::new();
    for foundation in &decoded {
        let mut pending = PendingIdentityValidation::new();
        for provider in providers {
            pending.register_authority(*provider).unwrap();
        }
        foundation.register_identities(&mut pending).unwrap();
        for previous in &graphs {
            pending
                .register_external_graph_authorities(previous)
                .unwrap();
        }
        foundation.resolve_identities(&mut pending).unwrap();
        graphs.push(pending.finish().unwrap());
    }
    graphs.pop().unwrap()
}

#[test]
fn property_initialization_uses_replay_actual_source_and_expanded_defaults() {
    for (name, count) in [("standalone", 1), ("combined", 4), ("inactive", 0)] {
        with_shared(name, |output, metadata, dependencies| {
            let actual = output
                .materialized_property_initialization_uses(metadata.identities)
                .unwrap();
            let replayed = metadata
                .materialized_property_initialization_uses(dependencies)
                .unwrap();
            assert_eq!(actual, replayed, "{name}");
            assert_eq!(actual.len(), count, "{name}: {actual:#?}");
            for usage in &actual {
                assert_eq!(usage.provider(), dependencies[1].provider);
                assert!(
                    dependencies[1]
                        .source_initialization_units()
                        .iter()
                        .any(|unit| unit.id() == usage.dependency_unit())
                );
                assert!(
                    output
                        .output()
                        .local
                        .module()
                        .initialization_units
                        .iter()
                        .any(|(_, unit)| unit.identity.id() == usage.local_unit())
                );
            }
            let mut dump = format!("uses={count}\n");
            for usage in &actual {
                dump.push_str(&format!(
                    "local={}\nprovider={}\ndependency={}\naccessor={}\n",
                    usage.local_unit(),
                    usage.provider(),
                    usage.dependency_unit(),
                    usage.accessor(),
                ));
            }
            let path = fixture_path().join(format!("{name}.hir-uses.snap"));
            if std::env::var_os("SCOOP_UPDATE_PROPERTY_INITIALIZATION").is_some() {
                std::fs::write(&path, &dump).unwrap();
            }
            assert_eq!(dump, std::fs::read_to_string(path).unwrap());
        });
    }
}
