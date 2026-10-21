//! Saved definition ownership and ABI data used by downstream compilation.

use scoop_identity::{ConeIdentity, ValidatedIdentityGraph};

use crate::{
    BootstrapManifest, CanonicalDefinedLinkSymbolOwnerSetV1, CanonicalOdrMemberDirectoryV1,
    CrossConeArtifactSummary, DecodedCrossConeLayoutLinkOnlySections,
};

pub struct ArtifactDefinitionData {
    defined: CanonicalDefinedLinkSymbolOwnerSetV1,
    odr: CanonicalOdrMemberDirectoryV1,
    summary: CrossConeArtifactSummary,
}

impl ArtifactDefinitionData {
    pub(crate) fn read(
        link: &DecodedCrossConeLayoutLinkOnlySections,
        manifest: &BootstrapManifest,
        identities: &mut ValidatedIdentityGraph,
        target: scoop_lir::ValidatedLirTargetSelection,
    ) -> Result<Self, String> {
        let defined = link.link_identity_closure_wire().read_defined_symbols(
            manifest.cone().identity(),
            identities,
            target.target().contract().native_symbol_normalization(),
        )?;
        let odr = link
            .production_manifest_wire()
            .read_odr_members(identities)
            .map_err(|error| error.to_string())?;
        let directory =
            crate::TestDirectory::from_manifest(manifest).map_err(|error| error.to_string())?;
        Ok(Self {
            defined,
            odr,
            summary: CrossConeArtifactSummary::from_layout(manifest, target, directory),
        })
    }

    pub fn provider(&self) -> ConeIdentity {
        self.defined.producer()
    }

    pub fn defined_symbols(&self) -> &CanonicalDefinedLinkSymbolOwnerSetV1 {
        &self.defined
    }

    pub fn odr_members(&self) -> &CanonicalOdrMemberDirectoryV1 {
        &self.odr
    }

    pub fn summary(&self) -> &CrossConeArtifactSummary {
        &self.summary
    }
}
