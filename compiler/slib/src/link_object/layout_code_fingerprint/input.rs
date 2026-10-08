//! One canonical Code encoding shared by the producer and immutable byte reader.

use super::*;
use crate::{SingleConeProductionCodeProjectionV1, VerifiedCodeLinkObjectMemberSetV2};
use scoop_wire::{Encoder, WireEncode, domain_separated_cbor_hash};

const CODE_FINGERPRINT_DOMAIN: &str = "scoop-code-v1";

pub(crate) struct LayoutCodeFingerprintInputV1<'a, S> {
    pub objects: &'a VerifiedCodeLinkObjectMemberSetV2,
    pub production: &'a SingleConeProductionCodeProjectionV1,
    pub strong: &'a S,
    pub link_extension_contributions: &'a CanonicalKnownLinkExtensionCodeContributionSetV1,
    pub native_requirements: &'a CanonicalNativeExternalRequirementSurfaceV1,
    pub native_contracts: &'a CanonicalNativeExternalContractCodeSetV1,
    pub defined_symbols: &'a CanonicalDefinedLinkSymbolOwnerSetV1,
    pub undefined_symbols: &'a CanonicalUndefinedSymbolRequirementSetV1,
}

impl<S: WireEncode> LayoutCodeFingerprintInputV1<'_, S> {
    pub(crate) fn fingerprint(&self) -> Result<CodeFingerprint, LayoutCodeFingerprintError> {
        domain_separated_cbor_hash(CODE_FINGERPRINT_DOMAIN, self)
            .map(|digest| CodeFingerprint::from_array(*digest.as_array()))
            .map_err(LayoutCodeFingerprintError::Hash)
    }
}

impl<S: WireEncode> WireEncode for LayoutCodeFingerprintInputV1<'_, S> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(10)?;
        encoder.field(1)?;
        self.objects.projection().encode(encoder)?;
        encoder.field(2)?;
        self.link_extension_contributions.encode(encoder)?;
        encoder.field(3)?;
        self.objects
            .final_objects()
            .entry()
            .patch_sites()
            .builtins()
            .c_bridge_production()
            .production()
            .encode(encoder)?;
        encoder.field(4)?;
        let requirements = self.native_requirements.library_requirements();
        encoder.array(requirements.len() as u64)?;
        for requirement in requirements {
            requirement.encode(encoder)?;
        }
        encoder.field(5)?;
        self.defined_symbols.encode(encoder)?;
        encoder.field(6)?;
        self.undefined_symbols.encode(encoder)?;
        encoder.field(7)?;
        self.native_contracts.encode(encoder)?;
        encoder.field(8)?;
        self.strong.encode(encoder)?;
        encoder.field(9)?;
        self.production.encode(encoder)?;
        encoder.field(10)?;
        encoder.unsigned(u64::from(self.native_requirements.cxx()))
    }
}
