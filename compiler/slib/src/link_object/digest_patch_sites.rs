//! Digest locations returned by codegen and indexed for finalization.

use super::VerifiedBuiltinObjectStrongRelocationSetV1;
use crate::SlibMemberId;
use scoop_identity::DigestPatchIntentId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DigestPatchSiteV1 {
    intent: DigestPatchIntentId,
    member: SlibMemberId,
    checked_offset: u64,
}
impl DigestPatchSiteV1 {
    pub const fn new(
        intent: DigestPatchIntentId,
        member: SlibMemberId,
        checked_offset: u64,
    ) -> Self {
        Self {
            intent,
            member,
            checked_offset,
        }
    }
    pub const fn intent(self) -> DigestPatchIntentId {
        self.intent
    }
    pub const fn member(self) -> SlibMemberId {
        self.member
    }
    pub const fn checked_offset(self) -> u64 {
        self.checked_offset
    }
    pub const fn width_bytes(self) -> u8 {
        32
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoopLirDigestPatchSitesV1 {
    builtins: VerifiedBuiltinObjectStrongRelocationSetV1,
    sites: Vec<DigestPatchSiteV1>,
}
impl ScoopLirDigestPatchSitesV1 {
    pub fn new(
        builtins: VerifiedBuiltinObjectStrongRelocationSetV1,
        mut sites: Vec<DigestPatchSiteV1>,
    ) -> Self {
        sites.sort_unstable_by_key(|site| site.intent());
        Self { builtins, sites }
    }
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.builtins.producer()
    }
    pub const fn builtins(&self) -> &VerifiedBuiltinObjectStrongRelocationSetV1 {
        &self.builtins
    }
    pub fn require_site(
        &self,
        intent: DigestPatchIntentId,
    ) -> Result<DigestPatchSiteV1, MissingDigestPatchSiteV1> {
        self.sites
            .binary_search_by_key(&intent, |site| site.intent())
            .map(|index| self.sites[index])
            .map_err(|_| MissingDigestPatchSiteV1(intent))
    }
    pub fn sites(&self) -> &[DigestPatchSiteV1] {
        &self.sites
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissingDigestPatchSiteV1(pub DigestPatchIntentId);
impl std::fmt::Display for MissingDigestPatchSiteV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "missing generated digest patch site: {:?}", self.0)
    }
}
impl std::error::Error for MissingDigestPatchSiteV1 {}
