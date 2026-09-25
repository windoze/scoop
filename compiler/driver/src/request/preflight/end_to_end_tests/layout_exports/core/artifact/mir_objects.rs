use super::*;
use scoop_identity::{GeneratedCallableKey, PersistentTypeId, SourceDeclarationKey};
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
    let mut rows = Vec::new();
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
        let unit = units[&owner];
        let kind = metadata
            .identities
            .canonical_key::<_, scoop_identity::InitializationUnitKey>(unit)
            .unwrap();
        rows.push(format!(
            "{}: {}\n",
            object_name(metadata, owner),
            match kind.as_ref() {
                scoop_identity::InitializationUnitKey::Object(_) => "object",
                scoop_identity::InitializationUnitKey::Companion(_) => "companion",
                _ => panic!("the object index contains only object initialization units"),
            }
        ));
        assert_eq!(key.origin(), source.provider());
        objects += 1;
    }
    assert_eq!(objects, if name.ends_with("combined") { 3 } else { 2 });
    rows.sort();
    let snapshot = crate::workspace_root().join(format!(
        "tests/fixtures/m23-core-layout-exports/{name}.hir.snap"
    ));
    if std::env::var_os("SCOOP_UPDATE_CORE_LAYOUT_EXPORTS").is_some() {
        std::fs::write(&snapshot, rows.concat()).unwrap();
    }
    assert_eq!(rows.concat(), std::fs::read_to_string(snapshot).unwrap());
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

    let mut missing = metadata.foundation.clone().into_canonical();
    missing.set_initialization_units(vec![]).unwrap();
    let missing = hir::OdrFreeHirFoundation::try_new(missing).unwrap();
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

fn object_name(metadata: hir::SharedTypeMetadataV1<'_>, owner: PersistentTypeId) -> String {
    let key = metadata
        .identities
        .canonical_key::<_, SourceDeclarationKey>(owner)
        .unwrap();
    let mut parts = Vec::new();
    for owner in key.owners().owners() {
        if let scoop_identity::DefinitionOwnerAtom::Type(owner) = owner {
            parts.push(object_name(metadata, *owner));
        }
    }
    let scoop_identity::DeclarationName::Named(name) = key.name() else {
        panic!("object and companion declarations have names")
    };
    parts.push(name.as_str().to_owned());
    parts.join(".")
}
