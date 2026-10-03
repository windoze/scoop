use super::*;
use scoop_identity::{DeclarationName, SignatureTypeKey, SourceDeclarationKey};

pub(super) fn check(
    artifact: &scoop_slib::AssembledCrossConeLayoutArtifactV1,
    target: &scoop_toolchain::ResolvedTargetProfile,
) {
    let bytes = artifact.as_bytes().to_vec();
    let read = scoop_slib::read_cross_cone_layout_artifact_closure(
        scoop_slib::CrossConeArtifactClosureInput::completed(
            ConeIdentity::CORE,
            target.lir_target_selection(),
            vec![],
            vec![],
            &bytes,
        ),
        target.c_bridge_toolchain().profile(),
    )
    .unwrap();
    drop(bytes);
    let (semantic, _) = read.artifact(ConeIdentity::CORE).unwrap();
    let identities = semantic.identity_graph();
    let nominal = semantic
        .mir_type_bridge()
        .exports()
        .shapes()
        .records()
        .iter()
        .find_map(|shape| {
            let key = identities.canonical_key::<_, SourceDeclarationKey>(shape.source()).unwrap();
            matches!(key.name(), DeclarationName::Named(name) if name.as_str() == "SharedAliasValue")
                .then_some(shape.source())
        })
        .unwrap();
    let expected = SignatureTypeKey::Nominal(nominal);
    let mut selected = Vec::new();
    for record in semantic.hir_interface().type_aliases().records() {
        let key = identities
            .canonical_key::<_, SourceDeclarationKey>(record.alias())
            .unwrap();
        if matches!(key.name(), DeclarationName::Named(name) if matches!(name.as_str(), "SharedAlias" | "SharedAliasAgain"))
        {
            let expansion = semantic
                .type_alias_expansions()
                .get(record.alias())
                .unwrap();
            assert_eq!(expansion.target(), &expected);
            selected.push(expansion);
        }
    }
    assert_eq!(selected.len(), 2);
    assert!(std::ptr::eq(selected[0].target(), selected[1].target()));
}
