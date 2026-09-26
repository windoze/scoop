//! Closure-wide Compile, Link, and terminal-definition validation.

use std::collections::BTreeMap;

use scoop_identity::{ConeIdentity, SemanticIdentitySession};
use scoop_lir::{CBridgeToolchainProfileV1, ValidatedLirTargetSelection};

use super::{
    DecodedCrossConeClosure, ValidatedCrossConeSemanticClosure,
    validate_and_commit_cross_cone_semantic_closure,
};
use crate::{
    CanonicalDefinedLinkSymbolOwnerSetV1, DecodedSlibEnvelope, PublishableCrossConeArtifact,
    ValidatedCompileArtifact, ValidatedCrossConeStrongLinkArtifact,
};

mod definition;
mod errors;
pub use errors::*;

use crate::link_decode::{
    DecodedCrossConeLinkOnlySections, decode_cross_cone_link_only,
    validate_cross_cone_link_from_compile,
};
use definition::validate_terminal_definitions;

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
    publications: Vec<PublishableCrossConeArtifact>,
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
    ) -> &ValidatedCompileArtifact<crate::CrossConeSemanticsStrongProfile> {
        self.closure.semantic.artifact_at(self.current_position)
    }

    pub fn current_link(&self) -> &ValidatedCrossConeStrongLinkArtifact {
        &self.closure.links[self.current_position]
    }

    pub fn current_publication(&self) -> &PublishableCrossConeArtifact {
        &self.closure.publications[self.current_position]
    }

    pub fn into_current_parts(
        mut self,
    ) -> (
        ValidatedCompileArtifact<crate::CrossConeSemanticsStrongProfile>,
        ValidatedCrossConeStrongLinkArtifact,
        PublishableCrossConeArtifact,
    ) {
        let compile = self
            .closure
            .semantic
            .into_artifact_at(self.current_position);
        let link = self.closure.links.swap_remove(self.current_position);
        let publication = self.closure.publications.swap_remove(self.current_position);
        (compile, link, publication)
    }

    pub fn into_current_publication(mut self) -> PublishableCrossConeArtifact {
        self.closure.publications.swap_remove(self.current_position)
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

    pub fn publication(&self, identity: ConeIdentity) -> Option<&PublishableCrossConeArtifact> {
        self.positions
            .get(&identity)
            .map(|position| &self.publications[*position])
    }

    pub fn link(&self, identity: ConeIdentity) -> Option<&ValidatedCrossConeStrongLinkArtifact> {
        self.positions
            .get(&identity)
            .map(|position| &self.links[*position])
    }

    pub fn current_publication(&self) -> Option<&PublishableCrossConeArtifact> {
        self.publication(self.semantic.current())
    }

    pub fn into_current_publication(mut self) -> Option<PublishableCrossConeArtifact> {
        let current = self.semantic.current();
        self.positions
            .get(&current)
            .copied()
            .map(|position| self.publications.swap_remove(position))
    }
}

/// Reads each envelope and its semantic sections once, then checks Link
/// objects and resolves their imports against actual provider definitions.
pub fn validate_cross_cone_artifact_closure<'input>(
    input: CrossConeArtifactClosureInput<'input>,

    c_bridge_profile: &CBridgeToolchainProfileV1,
    session: &mut SemanticIdentitySession,
) -> Result<ValidatedCrossConeArtifactClosure, CrossConeArtifactClosureValidationError> {
    let CrossConeArtifactClosureInput {
        current,
        target,
        direct,
        dependency_first,
        current_artifact,
    } = input;

    let dependency_count = dependency_first.len();
    let mut decoded = Vec::with_capacity(dependency_count);
    let mut physical =
        Vec::with_capacity(dependency_count + usize::from(current_artifact.is_some()));
    for (index, bytes) in dependency_first.into_iter().enumerate() {
        let (front, link) = decode_sections(
            bytes,
            target,
            CrossConeClosureArtifactSlotV1::Dependency(index),
        )?;
        decoded.push(front);
        physical.push(link);
    }
    let current_front = if let Some(bytes) = current_artifact {
        let (front, link) =
            decode_sections(bytes, target, CrossConeClosureArtifactSlotV1::Current)?;
        physical.push(link);
        Some(front)
    } else {
        None
    };
    let decoded = match current_front {
        Some(current_artifact) => DecodedCrossConeClosure::with_current_artifact(
            current,
            target,
            direct,
            decoded,
            current_artifact,
        ),
        None => DecodedCrossConeClosure::new(current, target, direct, decoded),
    };
    let semantic = validate_and_commit_cross_cone_semantic_closure(decoded, session)
        .map_err(|source| CrossConeArtifactClosureValidationError::Semantic(Box::new(source)))?;

    let mut links = Vec::with_capacity(physical.len());
    let mut publications = Vec::with_capacity(physical.len());
    let mut positions = BTreeMap::new();
    for (position, (compile, sections)) in semantic
        .all_artifacts_for_validation()
        .zip(physical)
        .enumerate()
    {
        let (publication, link) = validate_link(
            compile,
            sections,
            &links,
            c_bridge_profile,
            slot_for(position, dependency_count),
        )?;
        positions.insert(compile.identity(), position);
        publications.push(publication);
        links.push(link);
    }
    validate_terminal_definitions(&links, &positions)?;
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

fn decode_sections<'input>(
    bytes: &'input [u8],
    target: ValidatedLirTargetSelection,
    slot: CrossConeClosureArtifactSlotV1,
) -> Result<
    (
        crate::DecodedCrossConeHirFrontSections<'input>,
        DecodedCrossConeLinkOnlySections<'input>,
    ),
    CrossConeArtifactClosureValidationError,
> {
    let graph = DecodedSlibEnvelope::open(bytes, target)
        .map_err(
            |source| CrossConeArtifactClosureValidationError::CompileEnvelope {
                slot,
                source: Box::new(source),
            },
        )?
        .validate_graph()
        .map_err(
            |source| CrossConeArtifactClosureValidationError::CompileGraph {
                slot,
                source: Box::new(source),
            },
        )?;
    let (front, metadata) = graph
        .decode_cross_cone_shared_metadata()
        .map_err(
            |source| CrossConeArtifactClosureValidationError::CompileSections {
                slot,
                source: Box::new(source),
            },
        )?;
    let physical =
        decode_cross_cone_link_only(front.graph.clone(), &metadata).map_err(|source| {
            CrossConeArtifactClosureValidationError::Link {
                slot,
                source: Box::new(crate::StrongLinkArtifactValidationError::Decode(Box::new(
                    source,
                ))),
            }
        })?;
    Ok((front, physical))
}

fn validate_link(
    compile: &ValidatedCompileArtifact<crate::CrossConeSemanticsStrongProfile>,
    sections: DecodedCrossConeLinkOnlySections<'_>,
    validated_links: &[ValidatedCrossConeStrongLinkArtifact],
    c_bridge_profile: &CBridgeToolchainProfileV1,
    slot: CrossConeClosureArtifactSlotV1,
) -> Result<
    (
        PublishableCrossConeArtifact,
        ValidatedCrossConeStrongLinkArtifact,
    ),
    CrossConeArtifactClosureValidationError,
> {
    let dependency_owners = validated_links
        .iter()
        .map(|link| link.defined_symbols().clone())
        .collect::<Vec<_>>();
    let link = validate_cross_cone_link_from_compile(
        sections,
        compile,
        &dependency_owners,
        c_bridge_profile,
    )
    .map_err(|source| CrossConeArtifactClosureValidationError::Link {
        slot,
        source: Box::new(source),
    })?;
    let publication =
        PublishableCrossConeArtifact::from_validated_views(compile, &link).map_err(|source| {
            CrossConeArtifactClosureValidationError::ViewMismatch {
                slot,
                source: Box::new(source),
            }
        })?;
    Ok((publication, link))
}

const fn slot_for(position: usize, dependency_count: usize) -> CrossConeClosureArtifactSlotV1 {
    if position == dependency_count {
        CrossConeClosureArtifactSlotV1::Current
    } else {
        CrossConeClosureArtifactSlotV1::Dependency(position)
    }
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
        assert!(closure.current_publication().is_none());
        assert_eq!(closure.semantic().current(), ConeIdentity::CORE);
    }
}
