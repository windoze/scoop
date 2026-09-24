use super::*;
use scoop_identity::{
    CanonicalExactTypeDiagnosticName, CoreBuiltinNominal, ExactTypeDiagnosticCatalog,
    SignatureTypeKey,
};

pub(super) fn check(replay: &Replay<'_>, name: &str) {
    let metadata = replay.source.metadata();
    let coordinates = [ConeCoordinate::reserved_core()];
    let names =
        ExactTypeDiagnosticCatalog::try_new(metadata.identities, &coordinates, &mut meter())
            .unwrap();
    let exact = |id| {
        CanonicalExactTypeDiagnosticName::from_validated_graph(id, &names)
            .unwrap()
            .into_string()
    };
    let unit = metadata
        .signature_exact_type(
            &SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
            &mut meter(),
        )
        .unwrap();
    let applications = metadata
        .derived_equality_applications(&mut meter())
        .unwrap();
    let mut rows = Vec::new();
    let mut exports = 0;
    let mut deferred = 0;
    let mut private = 0;
    for (callable, owner) in applications {
        let owner_name = exact(owner);
        if !owner_name.contains("SharedEquality") && owner != unit {
            continue;
        }
        let definition = replay.strong.get(CallableOwner::Generated(callable));
        let binding = replay
            .section
            .callables()
            .get(StrongCallableDefinitionOwner::GeneratedCallable(callable));
        rows.push(format!(
            "hir {owner_name}: application={callable}, strong={}, exported={}\n",
            definition.is_some(),
            binding.is_some()
        ));
        if owner_name.contains("SharedEqualityDeferred") {
            assert!(definition.is_none() && binding.is_none());
            deferred += 1;
        }
        if owner_name.contains("SharedEqualityHidden") {
            assert!(definition.is_some() && binding.is_none());
            private += 1;
        }
        assert!(!owner_name.contains("SharedEqualityUnused"));
        assert!(!owner_name.contains("SharedEqualityNotComparable"));
        if let Some(binding) = binding {
            let signature = binding.semantic_signature();
            rows.push(format!(
                "mir {owner_name}: {:?} {:?}, receiver={}, parameter={}, result={}\n",
                signature.gc_effect(),
                signature.exact().effect(),
                exact(signature.exact().receiver().into_option().unwrap()),
                exact(signature.exact().parameters()[0]),
                exact(signature.exact().result())
            ));
            exports += 1;
        }
    }
    if name.ends_with("standalone") {
        assert_eq!((exports, deferred, private), (2, 1, 1));
    } else {
        assert_eq!((exports, deferred, private), (6, 0, 0));
    }
    rows.sort();
    let snapshot = crate::workspace_root().join(format!(
        "tests/fixtures/m23-core-layout-exports/{name}.equality.snap"
    ));
    if std::env::var_os("SCOOP_UPDATE_CORE_LAYOUT_EXPORTS").is_some() {
        std::fs::write(&snapshot, rows.concat()).unwrap();
    }
    assert_eq!(rows.concat(), std::fs::read_to_string(snapshot).unwrap());
}
