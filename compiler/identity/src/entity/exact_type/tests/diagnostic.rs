use super::*;
use crate::{
    CallableAdapterEnvironmentKey, CallableMaterialization, CallableMaterializationContext,
    CallableTemplateOwner, ClosureEnvironmentRole, ExactCallableSignature, GeneratedNominalKey,
    PersistentFunctionId, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};
use scoop_wire::{BudgetMeter, DecodeLimits};

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

fn generated(
    graph: &mut Graph,
    key: GeneratedNominalKey,
) -> (PersistentTypeId, PersistentExactTypeId) {
    let nominal = PersistentTypeId::from_generated_key(&key).unwrap();
    let exact_key = ExactTypeKey::Nominal(nominal);
    let exact = PersistentExactTypeId::from_key(&exact_key).unwrap();
    graph.generated.insert(nominal, key);
    graph.exact.insert(exact, exact_key);
    (nominal, exact)
}

#[test]
fn all_generated_roles_print_the_frozen_role_and_nominal_id() {
    let (mut graph, payload) = nominal_graph();
    let ExactTypeKey::Nominal(object) = graph.exact[&payload] else {
        unreachable!()
    };
    let function = PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("worker").unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap();
    let callable = CallableMaterialization::new(
        CallableTemplateOwner::Function(function),
        CallableMaterializationContext::NoSubstitution,
    );
    let keys = [
        GeneratedNominalKey::ClosureEnvironment {
            callable,
            role: ClosureEnvironmentRole::Lambda,
        },
        GeneratedNominalKey::CallableAdapterEnvironment {
            key: CallableAdapterEnvironmentKey::Dynamic {
                target: ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), payload),
            },
        },
        GeneratedNominalKey::CoroutineFrame {
            source_callable: callable,
        },
        GeneratedNominalKey::ContinuationAdapterEnvironment {
            source_callable: callable,
            suspension_site: StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, 0),
                [],
            ),
        },
        GeneratedNominalKey::CoroutineStep { result: payload },
        GeneratedNominalKey::BoxedValue { payload },
        GeneratedNominalKey::CoroutineSlot { value: payload },
        GeneratedNominalKey::ObjectBackingClass { object },
    ];
    for (index, key) in keys.into_iter().enumerate() {
        let (nominal, exact) = generated(&mut graph, key);
        let name = CanonicalExactTypeDiagnosticName::from_validated_graph_metered(
            exact,
            &graph,
            &mut meter(),
        )
        .unwrap();
        assert_eq!(name.as_str(), format!("g(r={:08x};i={nominal})", index + 1));
        assert_eq!(name.as_str().len(), 80);
    }
}

#[test]
fn generated_atoms_compose_with_source_and_structural_exact_types() {
    let (mut graph, payload) = nominal_graph();
    let (nominal, boxed) = generated(&mut graph, GeneratedNominalKey::BoxedValue { payload });
    let pointer_key = ExactTypeKey::RawPointer(boxed);
    let pointer = PersistentExactTypeId::from_key(&pointer_key).unwrap();
    graph.exact.insert(pointer, pointer_key);
    let tuple_key = ExactTypeKey::Tuple(NonEmptyVec::from_first(pointer, [boxed, payload]));
    let tuple = PersistentExactTypeId::from_key(&tuple_key).unwrap();
    graph.exact.insert(tuple, tuple_key);
    let source = CanonicalExactTypeDiagnosticName::from_validated_graph(payload, &graph).unwrap();
    let atom = format!("g(r=00000006;i={nominal})");
    assert_eq!(
        CanonicalExactTypeDiagnosticName::from_validated_graph(tuple, &graph)
            .unwrap()
            .as_str(),
        format!("t([r({atom}),{atom},{}])", source.as_str())
    );
}

#[test]
fn generated_definition_conflicts_and_relabeling_are_rejected() {
    let (mut graph, payload) = nominal_graph();
    let (nominal, exact) = generated(&mut graph, GeneratedNominalKey::BoxedValue { payload });
    let source = graph.types.values().next().unwrap().clone();
    graph.types.insert(nominal, source);
    assert!(
        matches!(CanonicalExactTypeDiagnosticName::from_validated_graph(exact, &graph),
        Err(super::super::ExactTypeDiagnosticError::ConflictingNominalDefinitions(id)) if id == nominal)
    );
    graph.types.remove(&nominal);
    graph.generated.insert(
        nominal,
        GeneratedNominalKey::CoroutineStep { result: payload },
    );
    assert!(
        matches!(CanonicalExactTypeDiagnosticName::from_validated_graph(exact, &graph),
        Err(super::super::ExactTypeDiagnosticError::InvalidGeneratedNominal(id)) if id == nominal)
    );
}

#[test]
fn diagnostic_allocation_and_rendering_charge_the_shared_budget() {
    let (mut graph, payload) = nominal_graph();
    let (_, exact) = generated(&mut graph, GeneratedNominalKey::BoxedValue { payload });
    let mut baseline = meter();
    CanonicalExactTypeDiagnosticName::from_validated_graph_metered(exact, &graph, &mut baseline)
        .unwrap();
    let usage = baseline.usage();
    for limits in [
        DecodeLimits {
            logical_heap_bytes: usage.logical_heap_bytes - 1,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            validation_work_units: usage.validation_work_units - 1,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            owned_bytes: usage.owned_bytes - 1,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_leaf_bytes: 79,
            ..DecodeLimits::default()
        },
    ] {
        assert!(matches!(
            CanonicalExactTypeDiagnosticName::from_validated_graph_metered(
                exact,
                &graph,
                &mut BudgetMeter::new(limits)
            ),
            Err(super::super::ExactTypeDiagnosticError::Resource(_))
        ));
    }
    let mut shared = BudgetMeter::new(DecodeLimits {
        logical_heap_bytes: usage.logical_heap_bytes,
        validation_work_units: usage.validation_work_units,
        owned_bytes: usage.owned_bytes,
        ..DecodeLimits::default()
    });
    CanonicalExactTypeDiagnosticName::from_validated_graph_metered(exact, &graph, &mut shared)
        .unwrap();
    assert!(
        CanonicalExactTypeDiagnosticName::from_validated_graph_metered(exact, &graph, &mut shared)
            .is_err()
    );
}

#[test]
fn shared_generated_dag_is_costed_before_expanded_output_allocation() {
    let (mut graph, payload) = nominal_graph();
    let (_, mut exact) = generated(&mut graph, GeneratedNominalKey::BoxedValue { payload });
    for _ in 0..20 {
        let key = ExactTypeKey::Tuple(NonEmptyVec::from_first(exact, [exact]));
        exact = PersistentExactTypeId::from_key(&key).unwrap();
        graph.exact.insert(exact, key);
    }
    let mut budget = meter();
    assert!(matches!(
        CanonicalExactTypeDiagnosticName::from_validated_graph_metered(exact, &graph, &mut budget),
        Err(super::super::ExactTypeDiagnosticError::NameTooLong { .. })
    ));
    // Only the small canonical generated key is copied, never the expanded name.
    assert!(budget.usage().owned_bytes < 1024);
}
