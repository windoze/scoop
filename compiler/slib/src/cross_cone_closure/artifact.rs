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
    validate_self_describing_cross_cone_strong_link_artifact_with_authorities,
};

mod definition;
mod errors;
pub use errors::*;

use definition::validate_terminal_definitions;

/// Borrowed final bytes for one dependency closure and, optionally, its
/// completed current artifact.
pub struct CrossConeArtifactClosureInput<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<&'input [u8]>,
    current_artifact: Option<&'input [u8]>,
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

/// The exact artifact set after every member has independently passed both
/// views and every cross-Cone requirement has resolved to a provider Strong
/// definition.
pub struct ValidatedCrossConeArtifactClosure<'input> {
    semantic: ValidatedCrossConeSemanticClosure<'input>,
    links: Vec<ValidatedCrossConeStrongLinkArtifact<'input>>,
    publications: Vec<PublishableCrossConeArtifact>,
    positions: BTreeMap<ConeIdentity, usize>,
}

/// Closure proof produced only when the current artifact bytes participated
/// in validation. The retained position makes all three current views total.
pub struct ValidatedCompletedCrossConeArtifactClosure<'input> {
    closure: ValidatedCrossConeArtifactClosure<'input>,
    current_position: usize,
}

impl<'input> ValidatedCompletedCrossConeArtifactClosure<'input> {
    pub const fn semantic(&self) -> &ValidatedCrossConeSemanticClosure<'input> {
        &self.closure.semantic
    }

    pub fn current_compile(
        &self,
    ) -> &ValidatedCompileArtifact<'input, crate::CrossConeSemanticsStrongProfile> {
        self.closure.semantic.artifact_at(self.current_position)
    }

    pub fn current_link(&self) -> &ValidatedCrossConeStrongLinkArtifact<'input> {
        &self.closure.links[self.current_position]
    }

    pub fn current_publication(&self) -> &PublishableCrossConeArtifact {
        &self.closure.publications[self.current_position]
    }

    pub fn into_current_publication(mut self) -> PublishableCrossConeArtifact {
        self.closure.publications.swap_remove(self.current_position)
    }
}

impl ValidatedCrossConeArtifactClosure<'_> {
    pub const fn semantic(&self) -> &ValidatedCrossConeSemanticClosure<'_> {
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

    pub fn link(
        &self,
        identity: ConeIdentity,
    ) -> Option<&ValidatedCrossConeStrongLinkArtifact<'_>> {
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

/// Reopens all bytes through the M23-5 Compile and Link readers, commits the
/// semantic closure once, and resolves Link imports against terminal owners.
pub fn validate_cross_cone_artifact_closure<'input>(
    input: CrossConeArtifactClosureInput<'input>,

    c_bridge_profile: &CBridgeToolchainProfileV1,
    session: &mut SemanticIdentitySession,
) -> Result<ValidatedCrossConeArtifactClosure<'input>, CrossConeArtifactClosureValidationError> {
    let CrossConeArtifactClosureInput {
        current,
        target,
        direct,
        dependency_first,
        current_artifact,
    } = input;

    let mut decoded = Vec::with_capacity(dependency_first.len());
    for (index, bytes) in dependency_first.iter().copied().enumerate() {
        decoded.push(decode_compile_front(
            bytes,
            target,
            CrossConeClosureArtifactSlotV1::Dependency(index),
        )?);
    }
    let current_front = current_artifact
        .map(|bytes| decode_compile_front(bytes, target, CrossConeClosureArtifactSlotV1::Current))
        .transpose()?;
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

    let dependency_count = dependency_first.len();
    let mut bytes = dependency_first;
    if let Some(current_bytes) = current_artifact {
        bytes.push(current_bytes);
    }
    let identities = semantic
        .all_artifacts_for_validation()
        .map(|artifact| artifact.identity())
        .collect::<Vec<_>>();
    debug_assert_eq!(bytes.len(), identities.len());

    let mut links = Vec::with_capacity(bytes.len());
    let mut publications = Vec::with_capacity(bytes.len());
    let mut positions = BTreeMap::new();
    for (position, (identity, bytes)) in identities.into_iter().zip(bytes).enumerate() {
        let (publication, link) = validate_link(
            &semantic,
            identity,
            bytes,
            target,
            &links,
            &positions,
            c_bridge_profile,
            slot_for(position, dependency_count),
        )?;
        positions.insert(identity, position);
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
) -> Result<
    ValidatedCompletedCrossConeArtifactClosure<'input>,
    CrossConeArtifactClosureValidationError,
> {
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

fn decode_compile_front<'input>(
    bytes: &'input [u8],

    target: ValidatedLirTargetSelection,
    slot: CrossConeClosureArtifactSlotV1,
) -> Result<crate::DecodedCrossConeHirFrontSections<'input>, CrossConeArtifactClosureValidationError>
{
    DecodedSlibEnvelope::open(bytes, target)
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
        )?
        .decode_cross_cone_hir_front_sections()
        .map_err(
            |source| CrossConeArtifactClosureValidationError::CompileSections {
                slot,
                source: Box::new(source),
            },
        )
}

#[allow(clippy::too_many_arguments)]
fn validate_link<'input>(
    semantic: &ValidatedCrossConeSemanticClosure<'input>,
    identity: ConeIdentity,
    bytes: &'input [u8],

    target: ValidatedLirTargetSelection,
    validated_links: &[ValidatedCrossConeStrongLinkArtifact<'input>],
    validated_positions: &BTreeMap<ConeIdentity, usize>,
    c_bridge_profile: &CBridgeToolchainProfileV1,
    slot: CrossConeClosureArtifactSlotV1,
) -> Result<
    (
        PublishableCrossConeArtifact,
        ValidatedCrossConeStrongLinkArtifact<'input>,
    ),
    CrossConeArtifactClosureValidationError,
> {
    let compile = semantic
        .all_artifacts_for_validation()
        .find(|artifact| artifact.identity() == identity)
        .expect("Link validation identity belongs to the committed Compile closure");
    let graph = DecodedSlibEnvelope::open(bytes, target).map_err(|source| {
        CrossConeArtifactClosureValidationError::LinkEnvelope {
            slot,
            source: Box::new(source),
        }
    })?;
    let graph = graph.validate_graph().map_err(|source| {
        CrossConeArtifactClosureValidationError::LinkGraph {
            slot,
            source: Box::new(source),
        }
    })?;
    let mut authorities = Vec::with_capacity(compile.direct_dependencies().len());
    for dependency in compile.direct_dependencies() {
        let dependency = dependency.identity();
        let Some(position) = validated_positions.get(&dependency).copied() else {
            return Err(
                CrossConeArtifactClosureValidationError::MissingLinkDependencyAuthority {
                    slot,
                    dependency,
                },
            );
        };
        let Some(link) = validated_links.get(position) else {
            return Err(
                CrossConeArtifactClosureValidationError::MissingLinkDependencyAuthority {
                    slot,
                    dependency,
                },
            );
        };
        authorities.push(link.identity_graph());
    }
    let dependency_owners = validated_links
        .iter()
        .map(|link| link.defined_symbols().clone())
        .collect::<Vec<_>>();
    let link = validate_self_describing_cross_cone_strong_link_artifact_with_authorities(
        graph,
        authorities,
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
