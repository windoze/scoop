use super::*;

/// Checked definition-side classification, required even for a body with no
/// external references. This proves neither concrete access nor expansion.
#[derive(Clone, Copy, Debug)]
pub struct CheckedProtectedDefaultOwnerProfileV1<'a> {
    key: ProtectedDefaultTemplateKeyV1,
    source: ProtectedDefaultOwnerSourceV1<'a>,
    profile: ProtectedDefaultWitnessSourceProfileV1,
}
impl<'a> CheckedProtectedDefaultOwnerProfileV1<'a> {
    pub const fn key(&self) -> ProtectedDefaultTemplateKeyV1 {
        self.key
    }
    pub const fn source(&self) -> ProtectedDefaultOwnerSourceV1<'a> {
        self.source
    }
    pub const fn profile(&self) -> ProtectedDefaultWitnessSourceProfileV1 {
        self.profile
    }

    pub fn validate_witness<'w>(
        self,
        witness: &'w ProtectedDefaultAccessWitnessV1,
    ) -> Result<CheckedProtectedDefaultWitnessSourceV1<'w, 'a>, ProtectedDefaultWitnessProfileError>
    {
        use ProtectedDefaultWitnessProfileError as Error;
        if witness.owner() != self.key.owner() {
            return Err(Error::Owner);
        }
        match (&witness.0, self.profile) {
            (Witness::ParamFree(witness), ProtectedDefaultWitnessSourceProfileV1::ParamFree) => {
                Ok(CheckedProtectedDefaultWitnessSourceV1::ParamFree(
                    CheckedParamFreeProtectedDefaultWitnessSourceV1 {
                        witness,
                        source: self.source,
                    },
                ))
            }
            (
                Witness::GenericSourceMetadata { .. },
                ProtectedDefaultWitnessSourceProfileV1::GenericSourceMetadata,
            ) => Ok(
                CheckedProtectedDefaultWitnessSourceV1::GenericSourceMetadata(
                    CheckedGenericProtectedDefaultWitnessSourceV1 {
                        witness,
                        source: self.source,
                    },
                ),
            ),
            _ => Err(Error::SourceProfile),
        }
    }
}

impl<'a> ProtectedDefaultOwnerSourceV1<'a> {
    pub fn validate_default_profile<A: ProtectedDefaultSourceProfileSemanticAuthority<E>, E>(
        self,
        key: ProtectedDefaultTemplateKeyV1,
        authority: &A,
        meter: &mut BudgetMeter,
    ) -> Result<CheckedProtectedDefaultOwnerProfileV1<'a>, ProtectedDefaultWitnessSourceError<E>>
    {
        use ProtectedDefaultWitnessSourceError as Error;
        if key.owner() != self.declaration() {
            return Err(Error::Owner);
        }
        let access = self.declaration_access();
        let owners = access.source().lexical_owners();
        meter
            .charge_work(owners.len() as u64, &WirePath::root())
            .map_err(Error::Resource)?;
        let generic = owners
            .iter()
            .any(|owner| matches!(owner, SourceNominalId::GenericTemplate(_)));
        let profile = authority
            .default_access_profile(key)
            .map_err(Error::Foundation)?;
        let valid = match profile {
            ProtectedDefaultWitnessSourceProfileV1::ParamFree => {
                matches!(self.payload().owner(), SourceNominalId::Concrete(_))
            }
            ProtectedDefaultWitnessSourceProfileV1::GenericSourceMetadata => generic,
        };
        if !valid {
            return Err(Error::SourceProfile);
        }
        Ok(CheckedProtectedDefaultOwnerProfileV1 {
            key,
            source: self,
            profile,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedDefaultWitnessProfileError {
    Owner,
    SourceProfile,
}
impl std::fmt::Display for ProtectedDefaultWitnessProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Owner => "default reference witness has a different checked source owner",
            Self::SourceProfile => {
                "default reference witness disagrees with the checked source profile"
            }
        })
    }
}
impl std::error::Error for ProtectedDefaultWitnessProfileError {}
