//! Closure-wide Compile, Link, and terminal-definition validation.

use std::collections::BTreeMap;

use scoop_identity::{ConeIdentity, SemanticIdentitySession};
use scoop_lir::{CBridgeToolchainProfileV1, ValidatedLirTargetSelection};

use super::ValidatedCrossConeSemanticClosure;
use crate::{
    CanonicalDefinedLinkSymbolOwnerSetV1, CrossConeArtifactSummary, ValidatedCompileArtifact,
    ValidatedCrossConeStrongLinkArtifact,
};

mod errors;
pub use errors::*;

/// Borrowed final bytes for one dependency closure and, optionally, its
/// completed current artifact.
pub struct CrossConeArtifactClosureInput<'input> {
    pub(crate) current: ConeIdentity,
    pub(crate) target: ValidatedLirTargetSelection,
    pub(crate) direct: Vec<ConeIdentity>,
    pub(crate) dependency_first: Vec<&'input [u8]>,
    pub(crate) current_artifact: Option<&'input [u8]>,
}

impl<'input> CrossConeArtifactClosureInput<'input> {
    pub const fn dependencies(
        current: ConeIdentity,
        target: ValidatedLirTargetSelection,
        direct: Vec<ConeIdentity>,
        dependency_first: Vec<&'input [u8]>,
    ) -> Self {
        Self {
            current,
            target,
            direct,
            dependency_first,
            current_artifact: None,
        }
    }

    pub const fn completed(
        current: ConeIdentity,
        target: ValidatedLirTargetSelection,
        direct: Vec<ConeIdentity>,
        dependency_first: Vec<&'input [u8]>,
        current_artifact: &'input [u8],
    ) -> Self {
        Self {
            current,
            target,
            direct,
            dependency_first,
            current_artifact: Some(current_artifact),
        }
    }
}

/// The complete artifact set with shared semantics, checked objects, and
/// cross-Cone references resolved to their provider definitions.
pub struct ValidatedCrossConeArtifactClosure {
    semantic: ValidatedCrossConeSemanticClosure,
    links: Vec<ValidatedCrossConeStrongLinkArtifact>,
    publications: Vec<CrossConeArtifactSummary>,
    positions: BTreeMap<ConeIdentity, usize>,
}

/// A completed artifact closure retaining all three current artifact views.
pub struct ValidatedCompletedCrossConeArtifactClosure {
    closure: ValidatedCrossConeArtifactClosure,
    current_position: usize,
}

impl ValidatedCompletedCrossConeArtifactClosure {
    pub const fn semantic(&self) -> &ValidatedCrossConeSemanticClosure {
        &self.closure.semantic
    }

    pub fn current_compile(
        &self,
    ) -> &ValidatedCompileArtifact<crate::CrossConeLayoutStrongProfile> {
        self.closure.semantic.artifact_at(self.current_position)
    }

    pub fn current_link(&self) -> &ValidatedCrossConeStrongLinkArtifact {
        &self.closure.links[self.current_position]
    }
}

impl ValidatedCrossConeArtifactClosure {
    pub const fn semantic(&self) -> &ValidatedCrossConeSemanticClosure {
        &self.semantic
    }

    pub fn dependency_symbol_owners(
        &self,
    ) -> impl Iterator<Item = &CanonicalDefinedLinkSymbolOwnerSetV1> {
        self.links.iter().map(|link| link.defined_symbols())
    }

    pub fn artifact_count(&self) -> usize {
        self.publications.len()
    }

    pub fn publication(&self, identity: ConeIdentity) -> Option<&CrossConeArtifactSummary> {
        self.positions
            .get(&identity)
            .map(|position| &self.publications[*position])
    }

    pub fn link(&self, identity: ConeIdentity) -> Option<&ValidatedCrossConeStrongLinkArtifact> {
        self.positions
            .get(&identity)
            .map(|position| &self.links[*position])
    }
}

/// Reads each envelope and its semantic sections once, then checks Link
/// objects and resolves their imports against actual provider definitions.
pub fn validate_cross_cone_artifact_closure<'input>(
    input: CrossConeArtifactClosureInput<'input>,

    c_bridge_profile: &CBridgeToolchainProfileV1,
    session: &mut SemanticIdentitySession,
) -> Result<ValidatedCrossConeArtifactClosure, CrossConeArtifactClosureValidationError> {
    let complete = crate::read_cross_cone_layout_artifact_closure(input, c_bridge_profile)
        .map_err(|error| CrossConeArtifactClosureValidationError::Layout(Box::new(error)))?;
    let (current, target, direct, records) = complete.into_parts();
    let mut artifacts = Vec::with_capacity(records.len());
    let mut links = Vec::with_capacity(records.len());
    let mut publications = Vec::with_capacity(records.len());
    let mut positions = BTreeMap::new();
    for (artifact, symbols) in records {
        let artifact = std::rc::Rc::new(artifact);
        positions.insert(artifact.identity(), artifacts.len());
        publications.push(crate::publish::capture_layout_publication(
            &artifact, &symbols, target,
        ));
        links.push(ValidatedCrossConeStrongLinkArtifact::new(
            std::rc::Rc::clone(&artifact),
            symbols,
        ));
        artifacts.push(artifact);
    }
    let semantic = ValidatedCrossConeSemanticClosure::from_layout(
        current, target, direct, &artifacts, session,
    )
    .map_err(|error| CrossConeArtifactClosureValidationError::Commit(Box::new(error)))?;
    Ok(ValidatedCrossConeArtifactClosure {
        semantic,
        links,
        publications,
        positions,
    })
}

/// Validates a closure that necessarily includes the completed current
/// artifact and returns a type with total current-view accessors.
#[allow(clippy::too_many_arguments)]
pub fn validate_completed_cross_cone_artifact_closure<'input>(
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<&'input [u8]>,
    current_artifact: &'input [u8],

    c_bridge_profile: &CBridgeToolchainProfileV1,
    session: &mut SemanticIdentitySession,
) -> Result<ValidatedCompletedCrossConeArtifactClosure, CrossConeArtifactClosureValidationError> {
    let closure = validate_cross_cone_artifact_closure(
        CrossConeArtifactClosureInput::completed(
            current,
            target,
            direct,
            dependency_first,
            current_artifact,
        ),
        c_bridge_profile,
        session,
    )?;
    let current_position = closure
        .positions
        .get(&current)
        .copied()
        .ok_or(CrossConeArtifactClosureValidationError::MissingCompletedCurrentArtifact)?;
    Ok(ValidatedCompletedCrossConeArtifactClosure {
        closure,
        current_position,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_core_dependency_closure_needs_no_artifact_views() {
        let mut session = SemanticIdentitySession::new();
        let closure = validate_cross_cone_artifact_closure(
            CrossConeArtifactClosureInput::dependencies(
                ConeIdentity::CORE,
                ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
                Vec::new(),
                Vec::new(),
            ),
            &crate::link_decode::c_bridge_profile_for_test(),
            &mut session,
        )
        .unwrap();

        assert_eq!(closure.artifact_count(), 0);
        assert_eq!(closure.semantic().current(), ConeIdentity::CORE);
    }
}
