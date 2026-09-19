use std::collections::BTreeMap;

use scoop_wire::encode;

use super::{CanonicalExactTypeDiagnosticName, ExactTypeDiagnosticGraph, ExactTypeKey};
use crate::{
    CanonicalIdentifier, ConeCoordinate, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    Effect, NonEmptyVec, PackagePath, PersistentExactTypeId, PersistentGenericTypeId,
    PersistentTypeId, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};

mod diagnostic;

#[derive(Default)]
struct Graph {
    exact: BTreeMap<PersistentExactTypeId, ExactTypeKey>,
    types: BTreeMap<PersistentTypeId, SourceDeclarationKey>,
    generated: BTreeMap<PersistentTypeId, crate::GeneratedNominalKey>,
    generic_types: BTreeMap<PersistentGenericTypeId, SourceDeclarationKey>,
    cones: BTreeMap<ConeIdentity, ConeCoordinate>,
}

impl ExactTypeDiagnosticGraph for Graph {
    fn generated_nominal_key(&self, id: PersistentTypeId) -> Option<&crate::GeneratedNominalKey> {
        self.generated.get(&id)
    }
    fn exact_type_key(&self, id: PersistentExactTypeId) -> Option<&ExactTypeKey> {
        self.exact.get(&id)
    }

    fn source_type_declaration(&self, id: PersistentTypeId) -> Option<&SourceDeclarationKey> {
        self.types.get(&id)
    }

    fn source_generic_type_declaration(
        &self,
        id: PersistentGenericTypeId,
    ) -> Option<&SourceDeclarationKey> {
        self.generic_types.get(&id)
    }

    fn cone_coordinate(&self, id: ConeIdentity) -> Option<&ConeCoordinate> {
        self.cones.get(&id)
    }
}

fn nominal_graph() -> (Graph, PersistentExactTypeId) {
    let coordinate = ConeCoordinate::new("org.example", "demo", "1.2.3").unwrap();
    let cone = coordinate.identity().unwrap();
    let declaration = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            cone,
            PackagePath::from_segments(vec![CanonicalIdentifier::new("app").unwrap()]),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("User").unwrap(),
        SourceNominalKind::Class,
        0,
    );
    let type_id = PersistentTypeId::from_source_declaration(&declaration).unwrap();
    let exact_key = ExactTypeKey::Nominal(type_id);
    let exact_id = PersistentExactTypeId::from_key(&exact_key).unwrap();
    let mut graph = Graph::default();
    graph.cones.insert(cone, coordinate);
    graph.types.insert(type_id, declaration);
    graph.exact.insert(exact_id, exact_key);
    (graph, exact_id)
}

#[test]
fn exact_type_variants_have_distinct_fixed_wire_and_hash() {
    let (mut graph, nominal) = nominal_graph();
    let tuple_key = ExactTypeKey::Tuple(NonEmptyVec::from_first(nominal, [nominal]));
    let tuple = PersistentExactTypeId::from_key(&tuple_key).unwrap();
    graph.exact.insert(tuple, tuple_key.clone());
    let function_key = ExactTypeKey::Function {
        effect: Effect::Suspend,
        parameters: vec![nominal],
        result: tuple,
    };
    let function = PersistentExactTypeId::from_key(&function_key).unwrap();
    graph.exact.insert(function, function_key);

    let encoded = encode(&tuple_key).unwrap();
    assert_eq!(
        hex(&encoded),
        format!("a2000301825820{}5820{}", nominal, nominal)
    );
    assert_eq!(
        PersistentExactTypeId::hash_stream_length(&tuple_key).unwrap(),
        8 + "scoop-exact-type-v1".len() as u64 + encoded.len() as u64
    );
    assert_eq!(
        tuple.to_string(),
        "8567e8450c021da219bdedb25eed21433bacff159b424ed4bd45bcc7cd95135d"
    );
    assert_ne!(tuple, function);
}

#[test]
fn all_six_exact_type_variants_have_fixed_wire_shapes() {
    let type_id = PersistentTypeId(ConeIdentity::CORE.0);
    let generic_id = PersistentGenericTypeId(ConeIdentity::SINGLE_FILE.0);
    let child = PersistentExactTypeId(ConeIdentity::CORE.0);
    let vectors = [
        (
            ExactTypeKey::Nominal(type_id),
            format!("a20001015820{type_id}"),
        ),
        (
            ExactTypeKey::NominalApplication {
                origin: generic_id,
                arguments: NonEmptyVec::from_first(child, []),
            },
            format!("a30002015820{generic_id}02815820{child}"),
        ),
        (
            ExactTypeKey::Tuple(NonEmptyVec::from_first(child, [])),
            format!("a2000301815820{child}"),
        ),
        (
            ExactTypeKey::Function {
                effect: Effect::Ordinary,
                parameters: vec![child],
                result: child,
            },
            format!("a40004010102815820{child}035820{child}"),
        ),
        (
            ExactTypeKey::RawPointer(child),
            format!("a20005015820{child}"),
        ),
        (
            ExactTypeKey::NativeFunctionPointer {
                calling_convention: crate::CallingConvention::C,
                parameters: vec![child],
                result: child,
            },
            format!("a40006010102815820{child}035820{child}"),
        ),
    ];
    for (key, expected) in vectors {
        assert_eq!(hex(&encode(&key).unwrap()), expected);
    }
}

#[test]
fn canonical_diagnostic_name_uses_only_identity_graph_spelling() {
    let (mut graph, nominal) = nominal_graph();
    let tuple_key = ExactTypeKey::Tuple(NonEmptyVec::from_first(nominal, [nominal]));
    let tuple = PersistentExactTypeId::from_key(&tuple_key).unwrap();
    graph.exact.insert(tuple, tuple_key);
    let function_key = ExactTypeKey::Function {
        effect: Effect::Suspend,
        parameters: vec![nominal],
        result: tuple,
    };
    let function = PersistentExactTypeId::from_key(&function_key).unwrap();
    graph.exact.insert(function, function_key);

    let atom = "n(c=org.example%3Ademo%3A1.2.3;p=app;o=-;k=C;x=User)";
    assert_eq!(
        CanonicalExactTypeDiagnosticName::from_validated_graph(function, &graph)
            .unwrap()
            .as_str(),
        format!("f(s;[{atom}]->t([{atom},{atom}]))")
    );
}

#[test]
fn missing_child_is_a_typed_error_instead_of_a_panic() {
    let (mut graph, nominal) = nominal_graph();
    let pointer_key = ExactTypeKey::RawPointer(nominal);
    let pointer = PersistentExactTypeId::from_key(&pointer_key).unwrap();
    graph.exact.remove(&nominal);
    graph.exact.insert(pointer, pointer_key);
    assert!(CanonicalExactTypeDiagnosticName::from_validated_graph(pointer, &graph).is_err());
}

#[test]
fn shared_dag_cost_is_recounted_before_any_large_allocation() {
    let (mut graph, mut root) = nominal_graph();
    for _ in 0..20 {
        let key = ExactTypeKey::Tuple(NonEmptyVec::from_first(root, [root]));
        let id = PersistentExactTypeId::from_key(&key).unwrap();
        graph.exact.insert(id, key);
        root = id;
    }
    assert!(matches!(
        CanonicalExactTypeDiagnosticName::from_validated_graph(root, &graph),
        Err(super::ExactTypeDiagnosticError::NameTooLong { .. })
    ));
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
