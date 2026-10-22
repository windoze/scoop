use super::*;
use scoop_hir as hir;
use scoop_identity::*;
use serde_json::{Value, json};

mod annotations;
mod body;
mod constants;
mod requirements;
mod types;

fn nominal_key(kind: SourceNominalKind, arity: u32) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("Example").unwrap(),
        kind,
        arity,
    )
}

#[test]
fn identity_references_round_trip_without_becoming_other_declaration_kinds() {
    let id = PersistentTypeId::from_source_declaration(&nominal_key(SourceNominalKind::Struct, 0))
        .unwrap();
    let reference = Reference::from(id);
    let value = serde_json::to_value(reference).unwrap();
    assert_eq!(value, id.to_string());
    assert_eq!(
        serde_json::from_value::<Reference<PersistentTypeId>>(value).unwrap(),
        reference
    );
    assert_eq!(
        id.to_string()
            .to_uppercase()
            .parse::<Reference<PersistentTypeId>>()
            .unwrap(),
        reference
    );
    assert_eq!(reference.as_array(), id.as_array());
    for invalid in [
        String::new(),
        "0".repeat(63),
        "0".repeat(65),
        "g".repeat(64),
        "中".repeat(21) + "0",
    ] {
        assert!(serde_json::from_value::<Reference<PersistentTypeId>>(json!(invalid)).is_err());
    }
    assert!(serde_json::from_value::<Reference<PersistentTypeId>>(json!(5)).is_err());
    let nominal = serde_json::to_value(NominalId::Concrete(reference)).unwrap();
    assert_eq!(nominal, json!({"kind": "concrete", "id": id.to_string()}));
    assert!(serde_json::from_value::<MethodId>(nominal).is_err());
}

#[test]
fn source_and_definition_origins_keep_exact_logical_paths_ranges_and_contexts() {
    let source = SourceIdentity::new(
        ConeIdentity::CORE,
        NormalizedSourcePath::new("src/类型.scoop").unwrap(),
    )
    .unwrap();
    let span = SourceSpan::new(3, 10).unwrap();
    let context_key = SourceContextKey::File {
        source: source.clone(),
    };
    let context = PersistentSourceContextId::from_key(&context_key).unwrap();
    let definition = DefinitionOrigin::new(source.clone(), span, &context_key).unwrap();
    let value = serde_json::to_value(QueryJson(&definition)).unwrap();
    assert_eq!(
        value,
        json!({
            "source": {"cone": ConeIdentity::CORE.to_string(), "path": "src/类型.scoop"},
            "span": {"startByte": 3, "endByte": 10},
            "context": context.to_string(),
        })
    );
    assert_eq!(
        serde_json::to_value(QueryJson(&EvaluationOrigin::at_definition(&definition))).unwrap(),
        value
    );
    assert_eq!(
        serde_json::to_value(QueryJson(&hir::ExportDefinitionSourceV1::new(definition))).unwrap(),
        value
    );
}
