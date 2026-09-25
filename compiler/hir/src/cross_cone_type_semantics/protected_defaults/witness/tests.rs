use super::*;
use crate::cross_cone_type_semantics::protected_interfaces::tests::support::{Fixture, nominal};
use crate::*;
use scoop_identity::{CallableTemplateOrigin, SignatureTypeKey};
use scoop_wire::{decode_canonical, encode};
mod coverage;
mod profiles;
mod wire_tests;

fn lookup(domain: PersistentAccessDomainV1) -> PersistentLookupDomainV1 {
    PersistentLookupDomainV1::new(domain)
}
fn empty_slots() -> CanonicalProtectedDefaultSlotCallDomainsV1 {
    CanonicalProtectedDefaultSlotCallDomainsV1::try_new(vec![]).unwrap()
}
fn param_free(owner: CallableTemplateOrigin) -> ProtectedDefaultAccessWitnessV1 {
    ProtectedDefaultAccessWitnessV1::param_free(
        owner,
        lookup(PersistentAccessDomainV1::universal()),
        empty_slots(),
        lookup(PersistentAccessDomainV1::universal()),
    )
    .unwrap()
}
struct ExpectedProfile {
    key: ProtectedDefaultTemplateKeyV1,
    profile: ProtectedDefaultWitnessSourceProfileV1,
}
impl ProtectedDefaultSourceProfileSemanticAuthority<&'static str> for ExpectedProfile {
    fn default_access_profile(
        &self,
        key: ProtectedDefaultTemplateKeyV1,
    ) -> Result<ProtectedDefaultWitnessSourceProfileV1, &'static str> {
        if self.key == key {
            Ok(self.profile)
        } else {
            Err("unknown default source")
        }
    }
}
