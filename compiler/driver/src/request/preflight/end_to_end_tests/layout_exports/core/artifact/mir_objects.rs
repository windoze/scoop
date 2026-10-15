use super::*;
use scoop_identity::{GeneratedCallableKey, SourceDeclarationKey};

pub(super) fn check(
    name: &str,
    source: hir::SharedTypeMetadataV1<'_>,
    representations: &hir::CanonicalNominalRepresentationSupportV1,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
) {
    if !name.starts_with("shared-objects-") {
        return;
    }
    let metadata = source;
    let units = metadata.object_initialization_units().unwrap();
    let mut objects = 0;
    let mut named_objects = std::collections::BTreeSet::new();
    for representation in representations.records() {
        let hir::NominalRepresentationShapeV1::Object { .. } = representation.shape() else {
            continue;
        };
        let owner = representation.owner();
        let key = metadata
            .identities
            .canonical_key::<_, SourceDeclarationKey>(owner)
            .unwrap();
        let unit = units[&hir::SourceNominalId::Concrete(owner)];
        let kind = metadata
            .identities
            .canonical_key::<_, scoop_identity::InitializationUnitKey>(unit)
            .unwrap();
        assert!(matches!(
            kind.as_ref(),
            scoop_identity::InitializationUnitKey::Object(_)
                | scoop_identity::InitializationUnitKey::Companion(_)
        ));
        assert_eq!(key.origin(), source.provider);
        if let scoop_identity::DeclarationName::Named(name) = key.name() {
            named_objects.insert(name.as_str().to_owned());
        }
        objects += 1;
    }
    assert!(named_objects.contains("SharedObjectService"));
    if name.ends_with("combined") {
        assert!(named_objects.contains("SharedObjectSupport"));
    }
    let mut entries = 0;
    for binding in section.callables().entries() {
        let mir::MirCallableOriginV1::Generated {
            role: GeneratedCallableKey::Initialization { .. },
            ..
        } = binding.origin()
        else {
            continue;
        };
        entries += 1;
    }
    assert_eq!(entries, objects * 2);
}
