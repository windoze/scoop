//! Read physical member assignments without rebuilding their producer plans.

use super::*;
use crate::ProgramLinkMember;
use scoop_identity::{IdentityReferenceError, PersistentIdResolver, ValidatedIdentityGraph};

impl DecodedLinkIdentityClosureSectionV1 {
    pub(crate) fn read_members(
        &self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<Vec<ProgramLinkMember>, IdentityReferenceError> {
        self.materializations
            .iter()
            .map(|record| {
                Ok(match record {
                    DecodedLinkObjectMaterializationV1::ScoopLir { member, units } => {
                        ProgramLinkMember::Scoop {
                            member: SlibMemberId::from_array(*member.bytes()),
                            definitions: units
                                .iter()
                                .map(|id| PersistentIdResolver::resolve(identities, *id))
                                .collect::<Result<_, _>>()?,
                        }
                    }
                    DecodedLinkObjectMaterializationV1::GeneratedCBridge { member, .. } => {
                        ProgramLinkMember::Generated {
                            member: SlibMemberId::from_array(*member.bytes()),
                        }
                    }
                })
            })
            .collect()
    }

    pub(crate) fn symbol_uses(&self) -> impl Iterator<Item = (SlibMemberId, &[u8])> {
        self.symbol_uses
            .iter()
            .map(|site| (site.member, site.symbol.as_slice()))
    }
}
