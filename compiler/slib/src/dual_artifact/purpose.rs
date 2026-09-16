use std::marker::PhantomData;
use std::sync::Arc;

use crate::{
    ArtifactSnapshot, CompileViewCertificateV1, DualValidatedArtifactHandle,
    DualValidatedArtifactReopenError, LinkViewCertificateV1, PublishableSingleConeArtifact,
    SingleConeStrongProfile, ValidatedCompileArtifact, ValidatedSingleConeStrongLinkArtifact,
};

/// Type marker for an artifact authority projected from the Compile
/// certificate of a dual-validated immutable snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompileArtifactPurpose;

/// Type marker for an artifact authority projected from the Link certificate
/// of a dual-validated immutable snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinkArtifactPurpose;

/// Purpose-preserving authority for one dual-validated artifact.
///
/// Projection requires the dual handle itself. There is no conversion between
/// Compile and Link handles even though both may share the same immutable
/// backing snapshot.
#[derive(Debug)]
pub struct PurposeArtifactHandle<P> {
    artifact: Arc<DualValidatedArtifactHandle>,
    purpose: PhantomData<fn() -> P>,
}

impl<P> Clone for PurposeArtifactHandle<P> {
    fn clone(&self) -> Self {
        Self {
            artifact: Arc::clone(&self.artifact),
            purpose: PhantomData,
        }
    }
}

impl PurposeArtifactHandle<CompileArtifactPurpose> {
    pub fn from_dual(artifact: Arc<DualValidatedArtifactHandle>) -> Self {
        Self {
            artifact,
            purpose: PhantomData,
        }
    }

    pub fn certificate(&self) -> &CompileViewCertificateV1 {
        self.artifact.compile_certificate()
    }

    pub fn with_view<R>(
        &self,
        use_view: impl for<'view> FnOnce(&ValidatedCompileArtifact<'view, SingleConeStrongProfile>) -> R,
    ) -> Result<R, DualValidatedArtifactReopenError> {
        self.artifact.with_compile_view(use_view)
    }
}

impl PurposeArtifactHandle<LinkArtifactPurpose> {
    pub fn from_dual(artifact: Arc<DualValidatedArtifactHandle>) -> Self {
        Self {
            artifact,
            purpose: PhantomData,
        }
    }

    pub fn certificate(&self) -> &LinkViewCertificateV1 {
        self.artifact.link_certificate()
    }

    pub fn with_view<R>(
        &self,
        use_view: impl for<'view> FnOnce(&ValidatedSingleConeStrongLinkArtifact<'view>) -> R,
    ) -> Result<R, DualValidatedArtifactReopenError> {
        self.artifact.with_link_view(use_view)
    }
}

impl<P> PurposeArtifactHandle<P> {
    pub fn publication(&self) -> &PublishableSingleConeArtifact {
        self.artifact.publication()
    }

    pub fn snapshot(&self) -> &Arc<ArtifactSnapshot> {
        self.artifact.snapshot()
    }
}

#[cfg(test)]
mod tests {
    use scoop_lir::ValidatedLirTargetSelection;
    use scoop_wire::DecodeLimits;

    use super::*;
    use crate::{
        CanonicalDefinedLinkSymbolOwnerSetV1, SlibClosureDecodeLimitsV1, SlibClosureDecodeMeterV1,
    };

    #[test]
    fn purpose_handles_retain_independent_typed_authority() {
        let snapshot = Arc::new(ArtifactSnapshot::from_bytes(
            crate::link_decode::complete_strong_artifact_for_test(false),
        ));
        let mut meter = SlibClosureDecodeMeterV1::new(SlibClosureDecodeLimitsV1::M23_DEFAULT);
        let dual = Arc::new(
            DualValidatedArtifactHandle::validate(
                snapshot,
                DecodeLimits::default(),
                ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
                &CanonicalDefinedLinkSymbolOwnerSetV1::empty_core_bootstrap(),
                &crate::link_decode::c_bridge_profile_for_test(),
                &mut meter,
            )
            .unwrap(),
        );
        let compile = PurposeArtifactHandle::<CompileArtifactPurpose>::from_dual(Arc::clone(&dual));
        let link = PurposeArtifactHandle::<LinkArtifactPurpose>::from_dual(dual);

        assert_eq!(
            compile.certificate().artifact_fingerprint(),
            link.certificate().artifact_fingerprint()
        );
        assert_eq!(
            compile.with_view(|view| view.identity()).unwrap(),
            link.with_view(|view| view.identity()).unwrap()
        );
        assert_eq!(compile.snapshot().digest(), link.snapshot().digest());
    }
}
