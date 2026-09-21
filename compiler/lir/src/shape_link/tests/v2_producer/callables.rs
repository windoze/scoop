use super::lir_fixture::function;
use super::*;
use scoop_identity::{
    CanonicalScoopAbiFunctionSignature, CoreBuiltinNominal, DependencyCallableDeclarationId,
    Effect, ExactCallableSignature, ExactTypeKey, ScoopAbiReturn, StrongCallableDefinitionOwner,
};

#[test]
fn mixed_external_callables_project_only_the_explicit_initialization_protocol() {
    let mut module = consumer_module(&ConeCoordinate::reserved_single_file());
    let ordinary = ordinary_callable(ConeIdentity::CORE);
    let ordinary_id = module.meta.external_callables.alloc(ordinary.clone());
    let protocol = crate::core_lir_cycle_thrower_for_test();
    let callable = ExternalCallable::initialization_cycle(
        protocol.target(),
        protocol.abi_signature().clone(),
        cycle_signature(),
        protocol.root_plan(),
    )
    .unwrap();
    let protocol_body = callable.body();
    let protocol_id = module.meta.external_callables.alloc(callable);
    let other = ConeCoordinate::new("test", "helper", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    module
        .meta
        .external_callables
        .alloc(ordinary_callable(other));
    assert_ne!(ordinary_id, protocol_id);
    let bridges = StrongExternalLirBridgeSurfaceV1::from_module(&module).unwrap();
    assert!(
        matches!(bridges.bridges(), [StrongExternalLirBridgeV1::Callable(found)]
        if found.target() == protocol.target() && found.required_definition() == protocol.required_definition())
    );

    set_dispatch(&mut module, protocol_id);
    let semantic = StrongTypeDescriptorSemanticPlanSetV1::from_module(&module).unwrap();
    assert_eq!(
        semantic.descriptors()[0].vtable().slots(),
        &[StrongTypeDispatchCallableRefV1::CoreExternal(protocol_body)]
    );

    set_dispatch(&mut module, ordinary_id);
    assert!(
        matches!(StrongTypeDescriptorSemanticPlanSetV1::from_module(&module),
        Err(StrongTypeDescriptorSemanticPlanBuildError::DependencyCallableInV1 { index, .. })
            if index == ordinary_id.into_raw().into_u32())
    );
    let empty =
        StrongProductionDependencySelectionV2::empty(module.cone, TARGET, &mut meter()).unwrap();
    assert!(
        matches!(StrongTypeDescriptorSemanticPlanSetV2::from_module(&module, &empty, &mut meter()),
        Err(StrongTypeDescriptorSemanticPlanBuildError::ExternalMaterialization(
            LayoutExternalMaterializationError::MissingCallable { provider, target }
        )) if provider == ConeIdentity::CORE && target == ordinary.target())
    );
}

#[test]
fn compiler_protocol_uses_the_common_abi_and_gc_validation() {
    let protocol = crate::core_lir_cycle_thrower_for_test();
    assert!(matches!(
        ExternalCallable::initialization_cycle(
            protocol.target(),
            protocol.abi_signature().clone(),
            cycle_signature(),
            ExternalCallableRootPlan::NoGc,
        ),
        Err(ExternalCallableBuildError::RootProtocolMismatch)
    ));
    assert!(matches!(
        ExternalCallable::initialization_cycle(
            protocol.target(),
            protocol.abi_signature().clone(),
            ScoopAbiSignature::new(Vec::new(), AbiReturn::UnitVoid, CallingConvention::Cdecl),
            protocol.root_plan(),
        ),
        Err(ExternalCallableBuildError::AbiArgumentCount {
            expected: 1,
            actual: 0
        })
    ));
}

fn set_dispatch(module: &mut Module, callable: ExternalCallableId) {
    let descriptor = module.meta.type_descriptors.iter_mut().next().unwrap().1;
    descriptor.vtable = VtableRecord::new(
        &descriptor.identity,
        vec![DispatchEntry {
            callable: CallableRef::External(callable),
        }],
    )
    .unwrap();
}

fn cycle_signature() -> ScoopAbiSignature {
    ScoopAbiSignature::new(
        vec![AbiArgument::Direct(
            AbiValue::new(
                MANAGED_PTR,
                AbiNonZeroLayout::new(8, 8).unwrap(),
                RefScan::References(vec![0]),
            )
            .unwrap(),
        )],
        AbiReturn::UnitVoid,
        CallingConvention::Cdecl,
    )
}

fn ordinary_callable(provider: ConeIdentity) -> ExternalCallable {
    let function = function(provider, "ordinary");
    let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap();
    let canonical = CanonicalScoopAbiFunctionSignature::new(
        ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit),
        Vec::new(),
        ScoopAbiReturn::unit_void(),
        scoop_identity::GcEffect::Managed,
    )
    .unwrap();
    SelectedDependencyLirCallableV1::new(
        provider,
        DependencyCallableDeclarationId::Function(function),
        StrongCallableDefinitionOwner::Function(function),
        canonical,
        CallingConvention::Cdecl,
        DependencyExternalCallableRootPlanV1::ManagedStatepoint,
    )
    .unwrap()
    .materialize(ScoopAbiSignature::new(
        Vec::new(),
        AbiReturn::UnitVoid,
        CallingConvention::Cdecl,
    ))
    .unwrap()
}
