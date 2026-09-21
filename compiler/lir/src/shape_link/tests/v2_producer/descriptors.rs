use super::lir_fixture::{exact, nominal};
use super::*;

#[test]
fn only_the_explicit_runtime_string_enters_the_legacy_descriptor_partition() {
    let mut module = consumer_module(&ConeCoordinate::reserved_single_file());
    let extra = descriptor(ConeIdentity::CORE, "CoreExtension");
    let extra_id = module.meta.external_type_descriptors.alloc(extra);
    let string = descriptor(ConeIdentity::CORE, "String");
    let string_id = module.meta.external_type_descriptors.alloc(string);
    module.meta.well_known_type_descriptors.string = TypeDescriptorRef::External(string_id);
    module
        .meta
        .external_type_descriptors
        .alloc(descriptor(ordinary_provider(), "Other"));
    let bridges = StrongExternalLirBridgeSurfaceV1::from_module(&module).unwrap();
    assert!(
        matches!(bridges.bridges(), [StrongExternalLirBridgeV1::TypeDescriptor(found)]
        if found.target() == string.target()
            && found.required_definition() == string.required_definition())
    );

    module
        .meta
        .type_descriptors
        .iter_mut()
        .next()
        .unwrap()
        .1
        .parent = Some(TypeDescriptorRef::External(string_id));
    let v1 = StrongTypeDescriptorSemanticPlanSetV1::from_module(&module).unwrap();
    assert_eq!(
        v1.descriptors()[0].parent(),
        Some(StrongTypeDescriptorRefV1::CoreExternal(string.target()))
    );
    let empty =
        StrongProductionDependencySelectionV2::empty(module.cone, TARGET, &mut meter()).unwrap();
    let v2 =
        StrongTypeDescriptorSemanticPlanSetV2::from_module(&module, &empty, &mut meter()).unwrap();
    assert_eq!(
        v2.descriptors()[0].parent(),
        Some(StrongTypeDescriptorRefV2::CoreExternal(string.target()))
    );

    module
        .meta
        .type_descriptors
        .iter_mut()
        .next()
        .unwrap()
        .1
        .parent = Some(TypeDescriptorRef::External(extra_id));
    assert!(
        matches!(StrongTypeDescriptorSemanticPlanSetV1::from_module(&module),
        Err(StrongTypeDescriptorSemanticPlanBuildError::DependencyDescriptorInV1(index))
            if index == extra_id.into_raw().into_u32())
    );
}

#[test]
fn every_ordinary_descriptor_requires_a_committed_layout_selection() {
    for provider in [ConeIdentity::CORE, ordinary_provider()] {
        let mut module = consumer_module(&ConeCoordinate::reserved_single_file());
        let external = descriptor(provider, "Exported");
        let id = module.meta.external_type_descriptors.alloc(external);
        module
            .meta
            .type_descriptors
            .iter_mut()
            .next()
            .unwrap()
            .1
            .parent = Some(TypeDescriptorRef::External(id));
        let empty = StrongProductionDependencySelectionV2::empty(module.cone, TARGET, &mut meter())
            .unwrap();
        assert!(matches!(
            StrongTypeDescriptorSemanticPlanSetV2::from_module(&module, &empty, &mut meter()),
            Err(StrongTypeDescriptorSemanticPlanBuildError::ExternalMaterialization(
                LayoutExternalMaterializationError::MissingDescriptor { provider: found, exact }
            )) if found == provider && exact == external.target()
        ));
    }
}

#[test]
fn a_foreign_runtime_string_role_is_rejected_even_without_descriptor_edges() {
    let mut module = consumer_module(&ConeCoordinate::reserved_single_file());
    let foreign = module
        .meta
        .external_type_descriptors
        .alloc(descriptor(ordinary_provider(), "String"));
    module.meta.well_known_type_descriptors.string = TypeDescriptorRef::External(foreign);
    assert!(matches!(
        StrongExternalLirBridgeSurfaceV1::from_module(&module),
        Err(StrongExternalLirBridgeBuildError::InvalidRuntimeStringDescriptor)
    ));
    assert!(matches!(
        StrongTypeDescriptorSemanticPlanSetV1::from_module(&module),
        Err(StrongTypeDescriptorSemanticPlanBuildError::ExternalBridge(
            StrongExternalLirBridgeBuildError::InvalidRuntimeStringDescriptor
        ))
    ));
    let empty =
        StrongProductionDependencySelectionV2::empty(module.cone, TARGET, &mut meter()).unwrap();
    assert!(matches!(
        StrongTypeDescriptorSemanticPlanSetV2::from_module(&module, &empty, &mut meter()),
        Err(StrongTypeDescriptorSemanticPlanBuildError::ExternalBridge(
            StrongExternalLirBridgeBuildError::InvalidRuntimeStringDescriptor
        ))
    ));
}

fn descriptor(provider: ConeIdentity, name: &str) -> ExternalTypeDescriptor {
    ExternalTypeDescriptor::new(provider, exact(&nominal(provider, name)).id()).unwrap()
}

fn ordinary_provider() -> ConeIdentity {
    ConeCoordinate::new("test", "descriptors", "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}
