use super::lir_fixture::{exact, nominal};
use super::*;

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
        let empty = StrongProductionDependencySelectionV2::empty(module.cone, TARGET).unwrap();
        assert!(matches!(
            StrongTypeDescriptorSemanticPlanSetV2::from_module(&module, &empty),
            Err(StrongTypeDescriptorSemanticPlanBuildError::ExternalMaterialization(
                LayoutExternalMaterializationError::MissingDescriptor { provider: found, exact }
            )) if found == provider && exact == external.target()
        ));
    }
}

#[test]
fn runtime_string_uses_ordinary_selection_even_without_descriptor_edges() {
    let mut module = consumer_module(&ConeCoordinate::reserved_single_file());
    let string = descriptor(ordinary_provider(), "String");
    let foreign = module.meta.external_type_descriptors.alloc(string);
    module.meta.well_known_type_descriptors.string = TypeDescriptorRef::External(foreign);
    assert!(
        StrongExternalLirBridgeSurfaceV1::from_module(&module)
            .unwrap()
            .bridges()
            .is_empty()
    );
    let empty = StrongProductionDependencySelectionV2::empty(module.cone, TARGET).unwrap();
    assert!(matches!(
        StrongTypeDescriptorSemanticPlanSetV2::from_module(&module, &empty),
        Err(StrongTypeDescriptorSemanticPlanBuildError::ExternalMaterialization(
            LayoutExternalMaterializationError::MissingDescriptor { provider, exact }
        )) if provider == string.provider() && exact == string.target()
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
