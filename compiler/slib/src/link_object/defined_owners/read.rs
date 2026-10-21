//! Resolve saved symbol ownership without inspecting object contents.

use super::*;
use scoop_identity::{PersistentIdResolver, ValidatedIdentityGraph};

impl DecodedCanonicalDefinedLinkSymbolOwnerSetV1 {
    pub(crate) fn read(
        &self,
        producer: ConeIdentity,
        identities: &mut ValidatedIdentityGraph,
        normalization: scoop_lir::NativeSymbolNormalization,
    ) -> Result<CanonicalDefinedLinkSymbolOwnerSetV1, String> {
        let owners = self
            .owners
            .iter()
            .map(|entry| {
                let (member, symbol, owner) = match entry {
                    DecodedDefinedLinkSymbolOwnerV1::Named {
                        member,
                        symbol,
                        owner,
                    } => (*member, symbol.clone(), owner.read(identities)?),
                    DecodedDefinedLinkSymbolOwnerV1::Boundary {
                        member,
                        atom,
                        boundary,
                    } => {
                        let atom = PersistentIdResolver::resolve(identities, *atom)
                            .map_err(|error| error.to_string())?;
                        let key = match boundary {
                            VerifiedBoundaryRoleV1::Start => {
                                scoop_identity::PersistentSymbolKey::DefinitionBoundaryStart(atom)
                            }
                            VerifiedBoundaryRoleV1::End => {
                                scoop_identity::PersistentSymbolKey::DefinitionBoundaryEnd(atom)
                            }
                        };
                        let symbol = scoop_identity::MangledSymbol::from_key(&key);
                        (
                            *member,
                            normalization
                                .compiler_generated_object_symbol(symbol.as_str())
                                .into_bytes(),
                            LinkDefinitionOwnerV1::VerifierBoundary {
                                atom,
                                boundary: *boundary,
                            },
                        )
                    }
                };
                Ok(DefinedLinkSymbolOwnerV1 {
                    member: SlibMemberId::from_array(member.0),
                    symbol,
                    owner,
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
