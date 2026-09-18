use scoop_hir::{
    DirectImportedProviderInput, ImportedProviderCertificate, ImportedSemanticWorld,
    ImportedSemanticWorldBuildError, SupportImportedProviderInput,
    TrustedCoreImportedProviderInput,
};
use scoop_identity::{ConeIdentity, SemanticOriginFingerprint};

use super::ValidatedCrossConeSemanticClosure;
use crate::{CrossConeSemanticsStrongProfile, ValidatedCompileArtifact};

impl ValidatedCrossConeSemanticClosure<'_> {
    /// Projects this atomically committed closure into HIR's immutable,
    /// role-separated semantic world. Support artifacts are supplied through
    /// a distinct input type and therefore cannot acquire binding enumeration
    /// while crossing the crate boundary.
    pub fn imported_semantic_world(
        &self,
    ) -> Result<ImportedSemanticWorld<'_>, ImportedSemanticWorldBuildError> {
        let mut trusted_core = None;
        let mut direct = Vec::with_capacity(self.direct.len());
        let mut support = Vec::with_capacity(
            self.dependency_first
                .len()
                .saturating_sub(self.direct.len()),
        );

        for artifact in &self.dependency_first {
            if artifact.identity() == self.current {
                continue;
            }
            let certificate = provider_certificate(artifact);
            let foundation = artifact.hir();
            let production = artifact.production();
            let interface = production.hir_interface();
            let aliases = production.type_alias_expansions();
            if self.direct.binary_search(&artifact.identity()).is_ok() {
                if artifact.identity() == ConeIdentity::CORE {
                    trusted_core = Some(TrustedCoreImportedProviderInput::from_validated(
                        certificate,
                        foundation,
                        interface,
                        aliases,
                    ));
                } else {
                    direct.push(DirectImportedProviderInput::from_validated(
                        certificate,
                        foundation,
                        interface,
                        aliases,
                    ));
                }
            } else {
                support.push(SupportImportedProviderInput::from_validated(
                    certificate,
                    foundation,
                    interface,
                    aliases,
                ));
            }
        }

        ImportedSemanticWorld::from_validated_closure(self.current, trusted_core, direct, support)
    }
}

pub(super) fn provider_certificate(
    artifact: &ValidatedCompileArtifact<'_, CrossConeSemanticsStrongProfile>,
) -> ImportedProviderCertificate {
    let semantic = artifact.semantic_fingerprints();
    ImportedProviderCertificate::from_validated(
        artifact.coordinate().clone(),
        artifact.identity(),
        SemanticOriginFingerprint::new(
            *semantic.hir().as_array(),
            *semantic.mir().as_array(),
            *semantic.lir().as_array(),
        ),
    )
}
