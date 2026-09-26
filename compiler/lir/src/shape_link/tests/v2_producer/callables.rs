use super::lir_fixture::function;
use super::*;
use scoop_identity::{
    CanonicalScoopAbiFunctionSignature, CoreBuiltinNominal, DependencyCallableDeclarationId,
    Effect, ExactCallableSignature, ExactTypeKey, ScoopAbiReturn, StrongCallableDefinitionOwner,
};

#[test]
fn dependency_dispatch_requires_the_selected_callable_definition() {
    let mut module = consumer_module(&ConeCoordinate::reserved_single_file());
    let ordinary = ordinary_callable(ConeIdentity::CORE);
    let target = ordinary.target();
    let id = module.meta.external_callables.alloc(ordinary);
    let descriptor = module.meta.type_descriptors.iter_mut().next().unwrap().1;
    descriptor.vtable = VtableRecord::new(
        &descriptor.identity,
        vec![DispatchEntry {
            callable: CallableRef::External(id),
        }],
    )
    .unwrap();
    let empty = StrongProductionDependencySelectionV2::empty(module.cone, TARGET).unwrap();
    assert!(
        matches!(StrongTypeDescriptorSemanticPlanSetV2::from_module(&module, &empty),
        Err(StrongTypeDescriptorSemanticPlanBuildError::ExternalMaterialization(
            LayoutExternalMaterializationError::MissingCallable { provider, target: actual }
        )) if provider == ConeIdentity::CORE && actual == target)
    );
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
        ExternalCallableRootPlan::ManagedStatepoint,
    )
    .unwrap()
    .materialize(ScoopAbiSignature::new(
        Vec::new(),
        AbiReturn::UnitVoid,
        CallingConvention::Cdecl,
    ))
    .unwrap()
}
