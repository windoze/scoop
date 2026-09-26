//! A valid top-level call cannot supply a forged member-use root in either view.

use super::*;
use scoop_identity::{AccessorRole, CallableTemplateOrigin, PropertyAccessorKey};

pub(super) fn check(
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    provider_artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
) {
    let reference = input
        .public
        .external_references()
        .records()
        .iter()
        .find(|reference| {
            matches!(
                reference.target(),
                hir::ExternalHirTargetV1::Callable(CallableTemplateOrigin::Accessor(_))
            ) && !reference.call_sites().is_empty()
                && reference
                    .call_sites()
                    .records()
                    .iter()
                    .all(|call| !call.receiver().has_receiver())
        })
        .expect("property fixtures have actual top-level accessor calls");
    let hir::ExternalHirTargetV1::Callable(CallableTemplateOrigin::Accessor(accessor)) =
        reference.target()
    else {
        panic!("the fixture must select a property accessor");
    };
    let key = input
        .identities
        .canonical_key::<_, PropertyAccessorKey>(accessor)
        .unwrap();
    let declaration = match key.role() {
        AccessorRole::Getter => hir::InheritanceCallableDeclarationV1::Getter(accessor),
        AccessorRole::Setter => hir::InheritanceCallableDeclarationV1::Setter(accessor),
    };
    let original = input.source;
    let mut selected = original.selected().records().to_vec();
    selected.push(hir::SelectedExternalTypeUseV1::new(
        reference.origin(),
        hir::SelectedTypeUseV1::MemberCall {
            receiver: reference.call_sites().records()[0].result(),
            declaration,
        },
    ));
    let candidate = hir::CrossConeTypeSemanticsSectionV1::new(
        original.exact_facts().clone(),
        original.representation_support().clone(),
        original.inheritance().clone(),
        hir::CanonicalSelectedExternalTypeUsesV1::try_new(selected).unwrap(),
    );
    let payload = wire::replace_types(artifact, &candidate);
    for link in [false, true] {
        let mut closure = reader::open(provider_artifact, artifact, &payload, link)
            .validate_hir_declarations()
            .unwrap();
        let error = match closure.validate_type_foundations() {
            Ok(_) => panic!("a member-use claim needs an actual source member call"),
            Err(error) => error,
        };
        assert_eq!(error.provider, input.mir.module().cone);
        assert!(
            matches!(
                *error.source,
                hir::SharedTypeMetadataError::TypeUseInventory
            ),
            "unexpected member-use rejection (link={link}): {error}"
        );
    }
}
