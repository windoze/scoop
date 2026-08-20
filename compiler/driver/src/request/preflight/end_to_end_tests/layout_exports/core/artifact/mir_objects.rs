use super::*;
use scoop_identity::{GeneratedCallableKey, SourceDeclarationKey};
use scoop_slib::SharedMirObjectValidationError as Error;

pub(super) fn check(
    name: &str,
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
) {
    let validate = |callables: &_, objects: &_| {
        scoop_slib::validate_shared_mir_objects(source, callables, objects)
    };
    validate(section.callables(), section.object_values()).unwrap();
    if !name.starts_with("shared-objects-") {
        return;
    }
    let metadata = source.metadata();
    let units = metadata.object_initialization_units().unwrap();
    let mut objects = 0;
    for representation in source.representations().table().records() {
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
        assert_eq!(key.origin(), source.provider());
        objects += 1;
    }
    // Include scalar companions and the independent UnitEncoder/UnitDecoder objects.
    assert_eq!(objects, if name.ends_with("combined") { 18 } else { 17 });
    for object in section.object_values().records() {
        let remaining = mir::CanonicalMirObjectValuesV1::try_new(
            section
                .object_values()
                .records()
                .iter()
                .filter(|record| record.value() != object.value())
                .cloned()
                .collect(),
        )
        .unwrap();
        assert!(
            matches!(validate(section.callables(), &remaining), Err(Error::MissingObject(value)) if value == object.value())
        );
    }
    let mut entries = 0;
    for binding in section.callables().entries() {
        let mir::MirCallableOriginV1::Generated {
            callable,
            role: GeneratedCallableKey::Initialization { .. },
        } = binding.origin()
        else {
            continue;
        };
        let remaining = mir::CanonicalMirCallableBindingsV1::try_new(
            section
                .callables()
                .entries()
                .iter()
                .filter(|record| record.implementation() != binding.implementation())
                .cloned()
                .collect(),
        )
        .unwrap();
        assert!(
            matches!(validate(&remaining, section.object_values()), Err(Error::MissingCallable(id)) if id == *callable)
        );
        entries += 1;
    }
    assert_eq!(entries, objects * 2);

    validate(section.callables(), section.object_values()).unwrap();

    let mut missing = metadata.foundation.clone();
    missing.set_initialization_units(vec![]).unwrap();
    let missing = source
        .section()
        .validate_shared_foundation(
            hir::SharedTypeMetadataV1 {
                foundation: &missing,
                ..metadata
            },
            &[],
        )
        .unwrap();
    assert!(matches!(
        scoop_slib::validate_shared_mir_objects(
            missing,
            section.callables(),
            section.object_values()
        ),
        Err(Error::MissingUnit(_))
    ));
}
