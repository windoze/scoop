//! Independently replay the Link view and retain its unresolved object inputs.

use super::*;

mod corruption;

pub(super) fn check(
    provider: &scoop_slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &scoop_slib::AssembledCrossConeLayoutStrongArtifactV1,
    layout: &lir::CrossConeLayoutAbiSectionV1<'_>,
    original: &scoop_slib::DecodedCrossConeLayoutLinkSections<'_>,
) -> String {
    let provider_wire = open_link(provider);
    let dump = read_link(provider, artifact)
        .with_replayed_physical_imports(|physical| {
            assert_eq!(physical.dependency_first().count(), 2);
            assert_eq!(physical.direct_providers(), &[provider_wire.identity()]);
            for original in [&provider_wire, original] {
                let current = physical.artifact(original.identity()).unwrap();
                let link = current.link_sections().unwrap();
                assert_eq!(
                    [
                        encode(link.production_manifest_wire()).unwrap(),
                        encode(link.link_identity_closure_wire()).unwrap(),
                        encode(link.cross_cone_link_closure_wire()).unwrap(),
                        encode(link.layout_link_closure_wire()).unwrap(),
                    ],
                    [
                        encode(original.production_manifest_wire()).unwrap(),
                        encode(original.link_identity_closure_wire()).unwrap(),
                        encode(original.cross_cone_link_closure_wire()).unwrap(),
                        encode(original.layout_link_closure_wire()).unwrap(),
                    ]
                );
            }
            let current = physical.artifact(layout.provider()).unwrap();
            assert_eq!(current.lir_exports(), layout.exports());
            let imports = current.lir_physical_imports();
            assert_eq!(
                encode(imports).unwrap(),
                encode(layout.selected().physical_imports()).unwrap(),
            );
            format!(
                "link-providers={}\nlink-physical={}\nlink-only-sections={}\n",
                physical.dependency_first().count(),
                imports.records().len(),
                physical
                    .dependency_first()
                    .filter(|provider| provider.link_sections().is_some())
                    .count()
                    * 4,
            )
        })
        .unwrap();
    corruption::check(provider, artifact, layout);
    dump
}
