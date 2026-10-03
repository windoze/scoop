use std::collections::BTreeMap;
use std::path::Path;

use scoop_identity::ConeIdentity;
use scoop_slib::{self as slib, SlibDiagnostic};

use super::ExplicitDependencyValidationError as Error;
use super::graph::ValidatedDependencyNode;

mod semantic;

pub(super) fn closure_error(
    source: slib::CrossConeArtifactClosureValidationError,
    nodes: &BTreeMap<ConeIdentity, ValidatedDependencyNode<'_>>,
    order: &[ConeIdentity],
) -> Box<Error> {
    match location(&source, order)
        .and_then(|(identity, path)| nodes.get(&identity).map(|node| (node.input.clone(), path)))
    {
        Some((input, semantic_path)) => Box::new(Error::ArtifactClosure {
            input,
            semantic_path,
            source: Box::new(source),
        }),
        None => Box::new(Error::Closure(Box::new(source))),
    }
}

fn location(
    error: &slib::CrossConeArtifactClosureValidationError,
    order: &[ConeIdentity],
) -> Option<(ConeIdentity, String)> {
    use slib::{
        CrossConeArtifactClosureValidationError as Closure,
        CrossConeLayoutArtifactValidationError as Layout,
    };
    match error {
        Closure::Layout(error) => match error.as_ref() {
            Layout::Envelope { slot, source } => Some((
                slot_identity(*slot, order)?,
                source.diagnostic().semantic_path(),
            )),
            Layout::Graph { slot, source } => Some((
                slot_identity(*slot, order)?,
                source.diagnostic().semantic_path(),
            )),
            Layout::LinkSections { slot, .. } => {
                Some((slot_identity(*slot, order)?, "link-sections".to_owned()))
            }
            Layout::Semantic { source } => semantic::location(source),
            Layout::Physical { source } => Some((source.provider, "objects".to_owned())),
            Layout::Resource { .. } | Layout::MissingCurrentArtifact => None,
        },
        Closure::Commit(error) => match error.as_ref() {
            slib::CrossConeSemanticCommitError::IdentityPreparation { identity, source } => {
                Some((*identity, source.diagnostic().semantic_path()))
            }
            slib::CrossConeSemanticCommitError::StateCountMismatch { .. }
            | slib::CrossConeSemanticCommitError::Allocation { .. }
            | slib::CrossConeSemanticCommitError::SemanticImport(_) => None,
        },
        Closure::MissingCompletedCurrentArtifact => None,
    }
}

fn slot_identity(
    slot: slib::CrossConeClosureArtifactSlotV1,
    order: &[ConeIdentity],
) -> Option<ConeIdentity> {
    match slot {
        slib::CrossConeClosureArtifactSlotV1::Dependency(index) => order.get(index).copied(),
        slib::CrossConeClosureArtifactSlotV1::Current => None,
    }
}

impl Error {
    pub(crate) fn artifact_location(&self) -> Option<(&Path, String)> {
        let (input, path) = match self {
            Self::ArtifactClosure {
                input,
                semantic_path,
                ..
            } => (input, semantic_path.clone()),
            Self::Summary { input, source } => (
                input,
                match source.as_ref() {
                    slib::ArtifactManifestSummaryError::Envelope(error) => {
                        error.diagnostic().semantic_path()
                    }
                    slib::ArtifactManifestSummaryError::Graph(error) => {
                        error.diagnostic().semantic_path()
                    }
                    slib::ArtifactManifestSummaryError::LengthOverflow => "container".to_owned(),
                },
            ),
            Self::CurrentConeArtifact { input, .. }
            | Self::UnsupportedArtifactShape { input, .. } => (input, "manifest".to_owned()),
            Self::DuplicateIdentity { second, .. } => (second, "manifest".to_owned()),
            _ => return None,
        };
        Some((input.path(), path))
    }
}

#[cfg(test)]
mod tests;
