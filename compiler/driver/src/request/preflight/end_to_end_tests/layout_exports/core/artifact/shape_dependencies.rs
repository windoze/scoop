//! Actual HIR operations select foreign helpers before machine consumption.

use super::*;
use std::convert::Infallible;

mod lir_reader;
mod lower;
mod machine;
mod mir_reader;
mod provider;

pub(super) fn check(
    producer: &lir::SingleConeStrongLirOutput,
    ordinary: &lir::CrossConeLirBridgeSectionV1,
    core_mir: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    core_lir: &lir::CrossConeLayoutAbiSectionV1<'_>,
    artifact: &scoop_slib::AssembledCrossConeLayoutStrongArtifactV1,
    target: &scoop_toolchain::ResolvedTargetProfile,
) {
    let prepared = provider::objects(producer, core_lir, target);
    let published = super::lir_dependencies::reader::open(artifact);
    assert_eq!(
        encode(prepared.foundation.as_canonical()).unwrap(),
        encode(published.lir_foundation_wire()).unwrap(),
    );
    let owners = [
        scoop_slib::CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(
            prepared.patch_sites.builtins().strong_relocations(),
        )
        .unwrap(),
    ];
    let provider = lir::ShapeLinkProviderV1::try_new(
        lir::ShapeLinkProviderPartsV1 {
            foundation: &prepared.foundation,
            production: lir::ShapeLinkProductionV1::Reader(&prepared.production),
            ordinary,
            layouts: core_lir.layouts(),
            callables: core_lir.callables(),
            descriptors: core_lir.descriptors(),
            dispatch: core_lir.dispatch(),
        },
        &mut meter(),
    )
    .unwrap();
    let sysroot = tempfile::tempdir().unwrap();
    let installed = bootstrap_core(sysroot.path(), target);
    let bytes = std::fs::read(installed.artifact().path()).unwrap();
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-shape-dependency-graph");
    for (name, names) in [
        ("standalone", &["Int"][..]),
        ("combined", &["Int", "String", "Unit"][..]),
    ] {
        let source = std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
        lower::with_mir(
            sysroot.path(),
            target,
            &bytes,
            &source,
            |input, callables| {
                let dependencies = scoop_mir_lower::MirTypeBridgeDependencyTablesV1 {
                    types: &[core_mir.types()],
                    callables: &[core_mir.callables()],
                    dispatch: &[core_mir.dispatch()],
                };
                let initialization =
                    mir::CanonicalMirExternalInitializationUsesV1::try_new(vec![], &mut meter())
                        .unwrap();
                let exports = scoop_mir_lower::lower_type_bridge_exports(
                    input,
                    dependencies,
                    initialization.clone(),
                    &mut meter(),
                )
                .unwrap();
                let projection = scoop_mir_lower::MirTypeBridgeSourceProjectionV1::from_input(
                    input,
                    dependencies,
                    initialization,
                    &mut meter(),
                )
                .unwrap();
                let uses = mir::MirTypeBridgeSectionSourceAuthorityV1::committed_external_uses(
                    &projection,
                )
                .unwrap();
                let mut shapes = uses
                    .iter()
                    .filter_map(|usage| match usage.target() {
                        mir::MirTypeBridgeTargetV1::ShapeSupport(owner) => {
                            Some((usage.provider(), owner))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let mut expected = names
                    .iter()
                    .map(|name| {
                        (
                            core_mir.provider(),
                            source_named(core_mir, input.identities, name),
                        )
                    })
                    .collect::<Vec<_>>();
                shapes.sort_unstable();
                expected.sort_unstable();
                assert_eq!(shapes, expected);
                let section = mir::CrossConeMirTypeBridgeSectionV1::try_new(
                    mir::MirTypeBridgeLocalAuthorityV1::Producer {
                        provider: input.mir.module().cone,
                        input: input.mir,
                        ordinary: input.ordinary,
                    },
                    exports,
                    &[core_mir],
                    &projection,
                    input.identities,
                    &mut meter(),
                )
                .unwrap();
                mir_reader::check(name, &fixtures, input, &section, core_mir, &expected);
                lir_reader::check(name, &fixtures, input, core_lir, &expected);
                machine::check(
                    name,
                    &fixtures,
                    input,
                    callables,
                    &expected,
                    machine::Provider {
                        view: &provider,
                        layout: core_lir,
                        target,
                        artifact,
                        owners: &owners,
                        string: core_lir
                            .shape_support()
                            .records()
                            .iter()
                            .find(|shape| {
                                shape.source_nominal()
                                    == source_named(core_mir, input.identities, "String")
                            })
                            .unwrap()
                            .exact(),
                    },
                    machine::PublicationInput {
                        bridge: &section,
                        source: &projection,
                    },
                );
            },
        );
    }
    assert_eq!(core_lir.target_profile(), target.lir_target());
    assert_eq!(
        encode(&prepared.production.into_section()).unwrap(),
        encode(published.lir_strong_production_wire()).unwrap(),
    );
}

fn source_named(
    core: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    identities: &ValidatedIdentityGraph,
    name: &str,
) -> scoop_identity::PersistentTypeId {
    core.shape_support().records().iter().find_map(|record| {
        let source = identities.canonical_key::<_, scoop_identity::SourceDeclarationKey>(record.source()).unwrap();
        matches!(source.name(), scoop_identity::DeclarationName::Named(actual) if actual.as_str() == name).then_some(record.source())
    }).unwrap_or_else(|| panic!("missing provider shape {name}"))
}

fn snapshot(path: &Path, text: &str) {
    if std::env::var_os("SCOOP_UPDATE_SHAPE_DEPENDENCY_GRAPH").is_some() {
        std::fs::write(path, text).unwrap();
    }
    assert_eq!(text, std::fs::read_to_string(path).unwrap());
}
