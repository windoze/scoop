//! Structural verifier for MIR fact products.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use scoopc_ids::StableCanonicalKey as _;
use scoopc_types::{WIRE_SCHEMA_VERSION, WireSchemaVersion};

use crate::MirFacts;
use crate::common::FactIdentity;
use crate::effects::CallSiteTarget;

/// Result type returned by MIR fact verification.
pub type Result<T> = std::result::Result<T, VerifyError>;

/// Structural errors detected before MIR facts are handed to later stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    UnsupportedSchemaVersion {
        found: WireSchemaVersion,
        expected: WireSchemaVersion,
    },
    DuplicateFactIdentity(String),
    DuplicateArtifactKey(String),
    MissingCanonicalSnapshot(String),
    EmptyCallSiteCandidateSet(String),
    EmptyCallSiteTargetJoin(String),
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion { found, expected } => write!(
                f,
                "unsupported MIR fact schema version {}.{}, expected {}.{}",
                found.major, found.minor, expected.major, expected.minor
            ),
            Self::DuplicateFactIdentity(key) => write!(f, "duplicate MIR fact identity `{key}`"),
            Self::DuplicateArtifactKey(key) => write!(f, "duplicate MIR artifact key `{key}`"),
            Self::MissingCanonicalSnapshot(key) => {
                write!(
                    f,
                    "canonical MIR snapshot `{key}` is not present in snapshots"
                )
            }
            Self::EmptyCallSiteCandidateSet(key) => {
                write!(f, "call-site target `{key}` has an empty candidate set")
            }
            Self::EmptyCallSiteTargetJoin(key) => {
                write!(f, "call-site target `{key}` has an empty join source list")
            }
        }
    }
}

impl Error for VerifyError {}

/// Verify facts that are already grouped by the MIR stage.
pub fn verify_mir_facts(facts: &MirFacts) -> Result<()> {
    verify_schema_version(facts)?;
    verify_unique_fact_identities(facts)?;
    verify_unique_owned_artifact_keys(facts)?;
    verify_canonical_snapshot_binding(facts)?;
    verify_call_site_targets(facts)?;
    Ok(())
}

fn verify_schema_version(facts: &MirFacts) -> Result<()> {
    if facts.schema_version != WIRE_SCHEMA_VERSION {
        return Err(VerifyError::UnsupportedSchemaVersion {
            found: facts.schema_version,
            expected: WIRE_SCHEMA_VERSION,
        });
    }

    Ok(())
}

fn verify_unique_fact_identities(facts: &MirFacts) -> Result<()> {
    let mut seen = HashSet::new();

    for identity in fact_identities(facts) {
        let key = identity.canonical_text().to_string();
        if !seen.insert(key.clone()) {
            return Err(VerifyError::DuplicateFactIdentity(key));
        }
    }

    Ok(())
}

fn fact_identities(facts: &MirFacts) -> Vec<&FactIdentity> {
    let mut identities = Vec::new();

    identities.extend(
        facts
            .roots
            .callable_bodies
            .iter()
            .map(|fact| &fact.identity),
    );
    identities.extend(facts.roots.initializers.iter().map(|fact| &fact.identity));
    identities.extend(facts.roots.extern_globals.iter().map(|fact| &fact.identity));
    identities.extend(facts.roots.metadata_roots.iter().map(|fact| &fact.identity));
    identities.extend(facts.families.instances.iter().map(|fact| &fact.identity));
    identities.extend(
        facts
            .families
            .callable_families
            .iter()
            .map(|fact| &fact.identity),
    );
    identities.extend(
        facts
            .effects
            .callable_instances
            .iter()
            .map(|fact| &fact.identity),
    );
    identities.extend(
        facts
            .effects
            .site_inventory
            .iter()
            .map(|fact| &fact.identity),
    );
    identities.extend(
        facts
            .effects
            .effect_events
            .iter()
            .map(|fact| &fact.identity),
    );
    identities.extend(
        facts
            .effects
            .block_regions
            .iter()
            .map(|fact| &fact.identity),
    );
    identities.extend(
        facts
            .effects
            .call_site_targets
            .iter()
            .map(|fact| &fact.identity),
    );
    identities.extend(
        facts
            .effects
            .call_site_surface_effects
            .iter()
            .map(|fact| &fact.identity),
    );
    identities.extend(
        facts
            .provenance
            .callable_values
            .iter()
            .map(|fact| &fact.identity),
    );
    identities.extend(facts.provenance.results.iter().map(|fact| &fact.identity));
    identities.extend(
        facts
            .boundary
            .source_contracts
            .iter()
            .map(|fact| &fact.identity),
    );
    identities.extend(
        facts
            .backend
            .source_signatures
            .iter()
            .map(|fact| &fact.identity),
    );
    identities.extend(facts.backend.enum_layouts.iter().map(|fact| &fact.identity));
    identities.extend(facts.backend.class_inits.iter().map(|fact| &fact.identity));
    identities.extend(facts.backend.vtables.iter().map(|fact| &fact.identity));
    identities.extend(facts.backend.interfaces.iter().map(|fact| &fact.identity));
    identities.extend(facts.backend.itables.iter().map(|fact| &fact.identity));
    identities.extend(facts.backend.extern_funs.iter().map(|fact| &fact.identity));
    identities.extend(
        facts
            .backend
            .native_callable_funs
            .iter()
            .map(|fact| &fact.identity),
    );
    identities.extend(facts.backend.global_inits.iter().map(|fact| &fact.identity));
    identities.extend(
        facts
            .metadata
            .nominal_direct_supertypes
            .iter()
            .map(|fact| &fact.identity),
    );

    identities
}

fn verify_unique_owned_artifact_keys(facts: &MirFacts) -> Result<()> {
    let mut seen = HashSet::new();

    for key in owned_artifact_keys(facts) {
        if !seen.insert(key.clone()) {
            return Err(VerifyError::DuplicateArtifactKey(key));
        }
    }

    Ok(())
}

fn owned_artifact_keys(facts: &MirFacts) -> Vec<String> {
    let mut keys = Vec::new();

    keys.extend(
        facts
            .snapshots
            .snapshots
            .iter()
            .map(|snapshot| snapshot.key.canonical_text()),
    );
    keys.extend(
        facts
            .families
            .instances
            .iter()
            .map(|instance| instance.artifact.canonical_text()),
    );
    keys.extend(
        facts
            .pass_artifacts
            .revisions
            .iter()
            .map(|revision| revision.key.canonical_text()),
    );

    keys
}

fn verify_canonical_snapshot_binding(facts: &MirFacts) -> Result<()> {
    let Some(canonical) = &facts.snapshots.canonical else {
        return Ok(());
    };
    let canonical_text = canonical.canonical_text();
    let exists = facts
        .snapshots
        .snapshots
        .iter()
        .any(|snapshot| snapshot.key.canonical_text() == canonical_text);
    if !exists {
        return Err(VerifyError::MissingCanonicalSnapshot(canonical_text));
    }

    Ok(())
}

fn verify_call_site_targets(facts: &MirFacts) -> Result<()> {
    for target in &facts.effects.call_site_targets {
        match &target.target {
            CallSiteTarget::CandidateSet { keys } if keys.is_empty() => {
                return Err(VerifyError::EmptyCallSiteCandidateSet(
                    target.identity.canonical_text().to_string(),
                ));
            }
            CallSiteTarget::Join { sources, .. } if sources.is_empty() => {
                return Err(VerifyError::EmptyCallSiteTargetJoin(
                    target.identity.canonical_text().to_string(),
                ));
            }
            _ => {}
        }
    }

    Ok(())
}
