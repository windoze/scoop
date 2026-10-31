//! The saved inputs needed for member selection and symbol resolution.

use super::*;
use crate::{
    CanonicalDefinedLinkSymbolOwnerSetV1, CanonicalOdrMemberDirectoryV1, CodeFingerprint,
    DecodedCrossConeLayoutLinkOnlySections, FingerprintAvailability, LirLinkSupportSectionV1,
    SlibMemberId,
};
use scoop_identity::ObjectDefinitionPlanId;

pub enum ProgramLinkMember {
    Scoop {
        member: SlibMemberId,
        definitions: Vec<ObjectDefinitionPlanId>,
    },
    Generated {
        member: SlibMemberId,
    },
}

impl ProgramLinkMember {
    pub fn member(&self) -> SlibMemberId {
        match self {
            Self::Scoop { member, .. } | Self::Generated { member } => *member,
        }
    }
}

pub struct ProgramLinkRequirement {
    member: SlibMemberId,
    symbol: std::sync::Arc<[u8]>,
}

impl ProgramLinkRequirement {
    pub fn member(&self) -> SlibMemberId {
        self.member
    }
    pub fn symbol(&self) -> &[u8] {
        &self.symbol
    }
}

pub struct ProgramLinkData {
    defined: CanonicalDefinedLinkSymbolOwnerSetV1,
    odr: CanonicalOdrMemberDirectoryV1,
    members: Vec<ProgramLinkMember>,
    requirements: Vec<ProgramLinkRequirement>,
    support: LirLinkSupportSectionV1,
    native: lir::CanonicalNativeExternalRequirementSurfaceV1,
    code: CodeFingerprint,
}

impl ProgramLinkData {
    pub(super) fn read(
        link: &DecodedCrossConeLayoutLinkOnlySections,
        manifest: &BootstrapManifest,
        identities: &mut ValidatedIdentityGraph,
        foundation: &lir::ConeLirFoundation,
        selection: lir::ValidatedLirTargetSelection,
    ) -> Result<Self, String> {
        let wire = link.link_identity_closure_wire();
        let defined = wire.read_defined_symbols(
            foundation,
            identities,
            selection.target().contract().native_symbol_normalization(),
        )?;
        let odr = wire
            .read_odr_members(identities)
            .map_err(|error| error.to_string())?;
        let members = wire
            .read_members(identities)
            .map_err(|error| error.to_string())?;
        let mut requirements = Vec::new();
        for group in wire.symbol_uses(&defined) {
            let (members, symbol) = group?;
            let symbol: std::sync::Arc<[u8]> = symbol.into();
            requirements.extend(members.iter().map(|member| ProgramLinkRequirement {
                member: *member,
                symbol: symbol.clone(),
            }));
        }
        let support = link
            .link_support_wire()
            .read_link(identities)
            .map_err(|error| error.to_string())?;
        let native = lir::CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
            selection.target(),
            foundation,
        )
        .map_err(|error| error.to_string())?;
        let FingerprintAvailability::Available(code) = manifest.semantic_fingerprints().code()
        else {
            return Err("program Link input has no Code fingerprint".to_owned());
        };
        Ok(Self {
            defined,
            odr,
            members,
            requirements,
            support,
            native,
            code,
        })
    }

    pub fn defined_symbols(&self) -> &CanonicalDefinedLinkSymbolOwnerSetV1 {
        &self.defined
    }
    pub fn odr_members(&self) -> &CanonicalOdrMemberDirectoryV1 {
        &self.odr
    }
    pub fn members(&self) -> &[ProgramLinkMember] {
        &self.members
    }
    pub fn requirements(&self) -> &[ProgramLinkRequirement] {
        &self.requirements
    }
    pub fn link_support(&self) -> &LirLinkSupportSectionV1 {
        &self.support
    }
    pub fn native_requirements(&self) -> &lir::CanonicalNativeExternalRequirementSurfaceV1 {
        &self.native
    }
    pub fn code_fingerprint(&self) -> CodeFingerprint {
        self.code
    }
}
