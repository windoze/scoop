//! Resolve saved symbol ownership without inspecting object contents.

use super::*;
use scoop_identity::{PersistentIdResolver, ValidatedIdentityGraph};

impl DecodedCanonicalDefinedLinkSymbolOwnerSetV1 {
    pub(crate) fn read(
        &self,
        producer: ConeIdentity,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<CanonicalDefinedLinkSymbolOwnerSetV1, String> {
        let owners = self
            .owners
            .iter()
            .map(|entry| {
                Ok(DefinedLinkSymbolOwnerV1 {
                    member: SlibMemberId::from_array(entry.member.0),
                    symbol: entry.symbol.clone(),
                    owner: entry.owner.read(identities)?,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(CanonicalDefinedLinkSymbolOwnerSetV1 { producer, owners })
    }
}

impl DecodedLinkDefinitionOwnerV1 {
    fn read(
        self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<LinkDefinitionOwnerV1, String> {
        let reference = |error: scoop_identity::IdentityReferenceError| error.to_string();
        Ok(match self {
            Self::StrongDefinition(owner) => {
                LinkDefinitionOwnerV1::StrongDefinition(StrongDefinitionOwnerV1 {
                    entity: owner
                        .entity
                        .resolve(identities)
                        .map_err(|error| error.to_string())?,
                    role: owner.role,
                })
            }
            Self::OdrDefinition(id) => LinkDefinitionOwnerV1::OdrDefinition(
                PersistentIdResolver::resolve(identities, id).map_err(reference)?,
            ),
            Self::GeneratedBridge(id) => LinkDefinitionOwnerV1::GeneratedBridge(
                PersistentIdResolver::resolve(identities, id).map_err(reference)?,
            ),
            Self::ConeImage(id) => LinkDefinitionOwnerV1::ConeImage(
                PersistentIdResolver::resolve(identities, id).map_err(reference)?,
            ),
            Self::VerifierBoundary { atom, boundary } => LinkDefinitionOwnerV1::VerifierBoundary {
                atom: PersistentIdResolver::resolve(identities, atom).map_err(reference)?,
                boundary,
            },
        })
    }
}
