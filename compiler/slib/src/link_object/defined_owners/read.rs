//! Resolve saved symbol ownership without inspecting object contents.

use super::*;
use scoop_identity::{PersistentIdResolver, ValidatedIdentityGraph};

impl DecodedCanonicalDefinedLinkSymbolOwnerSetV1 {
    pub(crate) fn read(
        &self,
        foundation: &scoop_lir::ConeLirFoundation,
        identities: &mut ValidatedIdentityGraph,
        normalization: scoop_lir::NativeSymbolNormalization,
    ) -> Result<CanonicalDefinedLinkSymbolOwnerSetV1, String> {
        let owners = self
            .owners
            .iter()
            .map(|entry| {
                let owner = entry.owner.read(identities)?;
                let symbol =
                    scoop_identity::MangledSymbol::from_key(&owner.symbol_key(foundation)?);
                Ok(DefinedLinkSymbolOwnerV1 {
                    member: SlibMemberId::from_array(entry.member.0),
                    symbol: normalization
                        .compiler_generated_object_symbol(symbol.as_str())
                        .into_bytes(),
                    owner,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(CanonicalDefinedLinkSymbolOwnerSetV1 {
            producer: foundation.producer(),
            owners,
        })
    }
}

impl LinkDefinitionOwnerV1 {
    fn symbol_key(
        self,
        foundation: &scoop_lir::ConeLirFoundation,
    ) -> Result<scoop_identity::PersistentSymbolKey, String> {
        use scoop_identity::PersistentSymbolKey as Key;
        Ok(match self {
            Self::StrongDefinition(owner) => owner
                .entity
                .primary_symbol_key(owner.role)
                .ok_or_else(|| format!("invalid definition subject {owner:?}"))?,
            Self::OdrDefinition(member) => foundation
                .odr_definition_symbol(member)
                .ok_or_else(|| format!("missing definition subject for ODR member {member}"))?,
            Self::GeneratedBridge(atom) => Key::GeneratedBridge(atom),
            Self::ConeImage(cone) => Key::ImageDescriptor(cone),
            Self::VerifierBoundary {
                atom,
                boundary: VerifiedBoundaryRoleV1::Start,
            } => Key::DefinitionBoundaryStart(atom),
            Self::VerifierBoundary {
                atom,
                boundary: VerifiedBoundaryRoleV1::End,
            } => Key::DefinitionBoundaryEnd(atom),
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_owners_recover_the_verified_names_for_each_object_format() {
        use crate::link_object::strong_relocation_closure::tests::verified_member_without_relocations;
        use crate::link_object::symbol_verification::tests::fixture_named;
        use scoop_lir::NativeSymbolNormalization;

        let fixture = fixture_named("ownerNameProjection");
        let closure = crate::verify_current_cone_strong_relocation_closure_v1(vec![
            verified_member_without_relocations(&fixture),
        ])
        .unwrap();
        let owners =
            CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&closure).unwrap();
        let foundation = scoop_lir::ConeLirFoundation::try_new(
            ConeIdentity::CORE,
            scoop_lir::CanonicalLirFoundation::empty(),
        )
        .unwrap();
        for entry in owners.owners() {
            let key = entry.owner().symbol_key(&foundation).unwrap();
            let logical = scoop_identity::MangledSymbol::from_key(&key);
            for normalization in [
                NativeSymbolNormalization::MachOExternalUnderscore,
                NativeSymbolNormalization::ElfIdentity,
                NativeSymbolNormalization::CoffIdentity,
            ] {
                let expected =
                    if normalization == NativeSymbolNormalization::MachOExternalUnderscore {
                        entry.symbol()
                    } else {
                        &entry.symbol()[1..]
                    };
                assert_eq!(
                    normalization
                        .compiler_generated_object_symbol(logical.as_str())
                        .as_bytes(),
                    expected
                );
            }
            let encoded = scoop_wire::encode(entry).unwrap();
            assert_eq!(encoded[0], 0xa2);
            assert!(
                !encoded
                    .windows(entry.symbol().len())
                    .any(|bytes| bytes == entry.symbol())
            );
        }
    }
}
