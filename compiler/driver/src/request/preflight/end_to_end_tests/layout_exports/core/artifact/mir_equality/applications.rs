use super::*;
use scoop_identity::{
    CanonicalExactTypeDiagnosticName, CoreBuiltinNominal, ExactTypeDiagnosticCatalog,
    SignatureTypeKey,
};

pub(super) fn check(replay: &Replay<'_>, name: &str) {
    let metadata = replay.source.metadata();
    let coordinates = [ConeCoordinate::reserved_core()];
    let names = ExactTypeDiagnosticCatalog::try_new(metadata.identities, &coordinates).unwrap();
    let exact = |id| {
        CanonicalExactTypeDiagnosticName::from_validated_graph(id, &names)
            .unwrap()
            .into_string()
    };
    let unit = metadata
        .signature_exact_type(&SignatureTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap();
    let applications = metadata.derived_equality_applications().unwrap();
    let mut exports = 0;
    let mut unit_exports = 0;
    let mut deferred = 0;
    let mut private = 0;
    let mut unused = 0;
    for (generated, owner) in applications {
        let owner_name = exact(owner);
        if !owner_name.contains("SharedEquality") && owner != unit {
            continue;
        }
        let definition = replay.strong.get(CallableOwner::Generated(generated));
        let binding = replay
            .section
            .callables()
            .entries()
            .iter()
            .find(|binding| super::callable(binding) == Some(generated));
        if owner == unit {
            assert!(definition.is_none());
            assert!(matches!(
                binding.unwrap().implementation(),
                scoop_identity::CallableDefinitionOwner::Odr(_)
            ));
            unit_exports += 1;
        }
        if owner_name.contains("SharedEqualityDeferred") {
            assert!(definition.is_some() && binding.is_some());
            deferred += 1;
        }
        if owner_name.contains("SharedEqualityHidden") {
            assert!(definition.is_some() && binding.is_none());
            private += 1;
        }
        if owner_name.contains("SharedEqualityUnused") {
            assert!(definition.is_some() && binding.is_some());
            unused += 1;
        }
        assert!(!owner_name.contains("SharedEqualityNotComparable"));
        if binding.is_some() {
            exports += 1;
        }
    }
    if name.ends_with("standalone") {
        assert_eq!(
            (exports, deferred, private, unused, unit_exports),
            (4, 1, 1, 1, 1)
        );
    } else {
        assert_eq!(
            (exports, deferred, private, unused, unit_exports),
            (6, 0, 0, 0, 0)
        );
    }
}
