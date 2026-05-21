//! Structural verifier for MIR fact products.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use scoopc_ids::StableCanonicalKey as _;

use crate::MirFacts;
use crate::common::FactIdentity;

/// Result type returned by MIR fact verification.
pub type Result<T> = std::result::Result<T, VerifyError>;

/// Structural errors detected before MIR facts are handed to later stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    DuplicateFactIdentity(String),
    DuplicateArtifactKey(String),
    MissingCanonicalSnapshot(String),
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateFactIdentity(key) => write!(f, "duplicate MIR fact identity `{key}`"),
            Self::DuplicateArtifactKey(key) => write!(f, "duplicate MIR artifact key `{key}`"),
            Self::MissingCanonicalSnapshot(key) => {
                write!(
                    f,
                    "canonical MIR snapshot `{key}` is not present in snapshots"
                )
            }
        }
    }
}

impl Error for VerifyError {}

/// Verify facts that are already grouped by the MIR stage.
pub fn verify_mir_facts(facts: &MirFacts) -> Result<()> {
    verify_unique_fact_identities(facts)?;
    verify_unique_owned_artifact_keys(facts)?;
    verify_canonical_snapshot_binding(facts)?;
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
