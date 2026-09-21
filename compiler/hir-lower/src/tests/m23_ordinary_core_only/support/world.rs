use scoop_hir as hir;
use scoop_identity::{ConeCoordinate, ConeIdentity, SemanticOriginFingerprint};

use super::TrustedCoreFixture;

impl TrustedCoreFixture {
    pub(crate) fn world(&self, current: ConeIdentity) -> hir::ImportedSemanticWorld<'_> {
        hir::ImportedSemanticWorld::from_validated_closure(
            current,
            vec![hir::DirectImportedProviderInput::from_validated(
                hir::ImportedProviderCertificate::from_validated(
                    ConeCoordinate::reserved_core(),
                    ConeIdentity::CORE,
                    SemanticOriginFingerprint::new([1; 32], [2; 32], [3; 32]),
                ),
                &self.foundation,
                &self.general_interface,
                &self.aliases,
            )],
            Vec::new(),
        )
        .unwrap()
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
