//! Joins actual body uses to independent source access and complete call domains.
use scoop_wire::WirePath;

use crate::{
    CheckedNominalInheritanceGraphV1, CheckedNominalInheritanceInterfacesV1,
    CheckedPersistentAccessDomainV1, CheckedProtectedDefaultOwnerProfileV1,
    DefaultBodyReferenceOccurrenceV1, ExportDefinitionSourceSemanticAuthority,
    ProtectedDefaultOwnerSourceV1, ProtectedDefaultReferenceReceiverV1,
    ProtectedDefaultRootSlotSemanticAuthority, ProtectedDefaultSourceProfileSemanticAuthority,
    ProtectedDefaultTemplateV1, ProtectedDefaultWitnessSourceProfileV1,
};

mod adapter;
mod checked;
mod errors;
pub use checked::*;
pub use errors::*;

/// Actual typed source use. The template is input to source replay, not an
/// authority for its claimed reference domains or source classification.
#[derive(Clone, Copy, Debug)]
pub struct ProtectedDefaultReferenceSourceUseV1<'t, 's, 'o> {
    pub template: &'t ProtectedDefaultTemplateV1,
    pub owner: CheckedProtectedDefaultOwnerProfileV1<'s>,
    pub occurrence: DefaultBodyReferenceOccurrenceV1<'o>,
    pub receiver: ProtectedDefaultReferenceReceiverV1<'o>,
}

/// Replays target identity, definition/evaluation origin, source route, lexical
/// access and the real receiver from independent checked source facts. Expected
/// target access must never be derived from the candidate reference witness.
pub trait ProtectedDefaultReferenceAccessSemanticAuthority<E>:
    ProtectedDefaultSourceProfileSemanticAuthority<E>
    + ProtectedDefaultRootSlotSemanticAuthority<E>
    + ExportDefinitionSourceSemanticAuthority<E>
{
    fn replay_param_free_default_reference<'g, 'a>(
        &mut self,
        source_use: ProtectedDefaultReferenceSourceUseV1<'_, '_, '_>,
        graph: &'g CheckedNominalInheritanceGraphV1<'a>,

        path: &WirePath,
    ) -> Result<CheckedPersistentAccessDomainV1<'g, 'a>, E>;

    /// Performs the same complete source checks without inventing concrete
    /// domains for an owner whose required generic coverage is not available.
    fn validate_generic_default_reference(
        &mut self,
        source_use: ProtectedDefaultReferenceSourceUseV1<'_, '_, '_>,

        path: &WirePath,
    ) -> Result<(), E>;
}

impl ProtectedDefaultTemplateV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn validate_reference_access_semantics<
        't,
        's,
        A: ProtectedDefaultReferenceAccessSemanticAuthority<E>,
        E,
    >(
        &'t self,
        source: ProtectedDefaultOwnerSourceV1<'s>,
        graph: &CheckedNominalInheritanceGraphV1<'_>,
        inheritance: CheckedNominalInheritanceInterfacesV1<'_>,
        authority: &mut A,

        path: &WirePath,
    ) -> Result<
        CheckedProtectedDefaultReferenceReplayV1<'t, 's>,
        ProtectedDefaultReferenceSemanticError<E>,
    > {
        let owner = source
            .validate_default_profile(self.key(), authority)
            .map_err(ProtectedDefaultReferenceSemanticError::OwnerProfile)?;
        let mut adapter = adapter::Adapter {
            template: self,
            owner,
            graph,
            inheritance,
            authority,
        };
        let body = self
            .references()
            .validate_body_closure(
                self.key(),
                self.body(),
                self.locals(),
                self.definition_origin(),
                self.receiver(),
                &mut adapter,
                path,
            )
            .map_err(ProtectedDefaultReferenceSemanticError::Body)?;
        let replay = checked::Replay {
            template: self,
            owner,
            body,
        };
        Ok(match owner.profile() {
            ProtectedDefaultWitnessSourceProfileV1::ParamFree => {
                CheckedProtectedDefaultReferenceReplayV1::ParamFree(
                    CheckedParamFreeProtectedDefaultReferenceReplayV1(replay),
                )
            }
            ProtectedDefaultWitnessSourceProfileV1::GenericSourceMetadata => {
                CheckedProtectedDefaultReferenceReplayV1::GenericSourceMetadata(
                    CheckedGenericProtectedDefaultReferenceReplayV1(replay),
                )
            }
        })
    }
}

#[cfg(test)]
mod tests;
