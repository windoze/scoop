//! Independent profile classification after complete source domain replay.
use super::*;
mod errors;
pub use errors::DefaultSourceProfileBindingError;
type Profile = ProtectedDefaultWitnessSourceProfileV1;
type BindingError = DefaultSourceProfileBindingError;

/// Verified source classification, without receiver, operation or execution proof.
#[derive(Debug)]
pub struct BoundNominalDefaultSourceProfilesV1<'b, 'd, 'p, 's, 'a, 'f> {
    domains: BoundNominalDefaultTargetDomainsV1<'b, 'd, 'p, 's, 'a, 'f>,
    profiles: &'b CanonicalDefaultSourceProfilesV1,
}
impl<'b, 'd, 'p, 's, 'a, 'f> BoundNominalDefaultSourceProfilesV1<'b, 'd, 'p, 's, 'a, 'f> {
    pub const fn domains(&self) -> &BoundNominalDefaultTargetDomainsV1<'b, 'd, 'p, 's, 'a, 'f> {
        &self.domains
    }
    pub const fn profiles(&self) -> &'b CanonicalDefaultSourceProfilesV1 {
        self.profiles
    }
}
impl<'b, 'd, 'p, 's, 'a, 'f> BoundNominalDefaultTargetDomainsV1<'b, 'd, 'p, 's, 'a, 'f> {
    pub fn bind_source_profiles(
        self,
        profiles: &'b CanonicalDefaultSourceProfilesV1,
    ) -> Result<BoundNominalDefaultSourceProfilesV1<'b, 'd, 'p, 's, 'a, 'f>, BindingError> {
        profiles
            .validate_template_coverage(self.declarations().origins().templates())
            .map_err(BindingError::coverage)?;

        for (declaration, record) in self
            .declarations()
            .declarations()
            .iter()
            .zip(profiles.records())
        {
            let mut generic = declaration.owner_binders().nominal_owner_binder_arity() != 0
                || !declaration
                    .publishing_call_domain()
                    .generic_subclasses()
                    .is_empty();
            for occurrence in declaration.references().occurrences() {
                let witness = occurrence.source().witness();
                generic |= !witness.direct_call_domain().generic_subclasses().is_empty()
                    || !witness.target_domain().generic_subclasses().is_empty()
                    || matches!(witness.slot_call_domain(), OptionalDefaultSourceSlotDomainV1::Present(domain) if !domain.generic_subclasses().is_empty());
            }
            let expected = if generic {
                Profile::GenericSourceMetadata
            } else {
                Profile::ParamFree
            };
            if record.profile() != expected {
                return Err(BindingError::Mismatch {
                    key: declaration.key(),
                    expected,
                    actual: record.profile(),
                });
            }
        }
        Ok(BoundNominalDefaultSourceProfilesV1 {
            domains: self,
            profiles,
        })
    }
}
impl ProtectedDefaultSourceProfileSemanticAuthority<BindingError>
    for BoundNominalDefaultSourceProfilesV1<'_, '_, '_, '_, '_, '_>
{
    fn default_access_profile(
        &self,
        key: ProtectedDefaultTemplateKeyV1,
    ) -> Result<Profile, BindingError> {
        self.profiles
            .get(key)
            .map(DefaultSourceProfileV1::profile)
            .ok_or(BindingError::MissingProfile(key))
    }
}
