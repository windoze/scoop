use scoop_hir as hir;
use scoop_identity::{ConeCoordinate, ConeIdentity, SemanticOriginFingerprint};

use super::TrustedCoreFixture;

impl TrustedCoreFixture {
    pub(crate) fn world(&self, current: ConeIdentity) -> hir::ImportedSemanticWorld<'_> {
        hir::ImportedSemanticWorld::from_validated_closure(
            current,
            vec![self.provider()],
            Vec::new(),
        )
        .unwrap()
    }

    pub(crate) fn provider(&self) -> hir::DirectImportedProviderInput<'_> {
        hir::DirectImportedProviderInput::from_validated(
            hir::ImportedProviderCertificate::from_validated(
                ConeCoordinate::reserved_core(),
                ConeIdentity::CORE,
                SemanticOriginFingerprint::new([1; 32], [2; 32], [3; 32]),
            ),
            &self.foundation,
            &self.general_interface,
            &self.aliases,
        )
    }

    pub(crate) fn type_binding(&self, name: &str) -> hir::DirectImportedTargetBinding {
        let world = self.world(ConeIdentity::SINGLE_FILE);
        world
            .direct_package(&scoop_identity::PackagePath::root())
            .unwrap()
            .binding_group(scoop_identity::BindingNamespace::Type, name)
            .unwrap_or_else(|| panic!("the core fixture exports type {name}"))
            .targets()
            .next()
            .unwrap()
            .clone()
    }
}

pub(super) fn project_interface(
    output: &hir::Output,
    foundation: &mut hir::CanonicalHirFoundation,
) -> hir::CrossConeHirInterfaceSectionV1 {
    let world = hir::ImportedSemanticWorld::from_validated_closure(
        ConeIdentity::CORE,
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let mut authority = hir::CrossConeHirProductionAuthority::new(
        foundation,
        &output.export.public_export_bindings,
        &world,
    );
    let interface = hir::CrossConeHirInterfaceSectionV1::from_export_hir(
        output.export.module(),
        &[],
        &mut authority,
    )
    .unwrap();
    foundation
        .complete_cross_cone_source_points(output.export.module(), interface.definition_sources())
        .unwrap();
    interface
}

#[test]
fn compiler_operations_come_from_shared_callable_declarations() {
    let core = super::trusted_core();
    let world = core.world(ConeIdentity::SINGLE_FILE);
    let selection = world.dependency_selection_plan().unwrap();
    for kind in hir::intrinsic_function_kinds() {
        let Some(effect) = kind.integer_gc_effect() else {
            continue;
        };
        let (effect, wrong) = match effect {
            hir::GcEffect::NoGc => (
                scoop_identity::GcEffect::NoGc,
                scoop_identity::GcEffect::Managed,
            ),
            hir::GcEffect::Managed => (
                scoop_identity::GcEffect::Managed,
                scoop_identity::GcEffect::NoGc,
            ),
        };
        assert!(selection.has_intrinsic_callable(kind, effect), "{kind:?}");
        assert!(!selection.has_intrinsic_callable(kind, wrong), "{kind:?}");
    }
}
