//! Structural verifier for HIR fact products.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use scoopc_ids::SiteId;

use crate::HirFacts;
use crate::common::FactIdentity;

/// Result type returned by HIR fact verification.
pub type Result<T> = std::result::Result<T, VerifyError>;

/// Structural errors detected before facts are handed to later stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    DuplicateFactIdentity(String),
    DuplicateSourceSite { owner: String, site: SiteId },
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateFactIdentity(key) => write!(f, "duplicate HIR fact identity `{key}`"),
            Self::DuplicateSourceSite { owner, site } => write!(
                f,
                "duplicate HIR source-site contract `{}` in owner `{owner}`",
                site.as_u32()
            ),
        }
    }
}

impl Error for VerifyError {}

/// Verify facts that are already grouped by the HIR barrier.
pub fn verify_hir_facts(facts: &HirFacts) -> Result<()> {
    verify_unique_fact_identities(facts)?;
    verify_unique_source_sites(facts)?;
    Ok(())
}

fn verify_unique_fact_identities(facts: &HirFacts) -> Result<()> {
    let mut seen = HashSet::new();

    for identity in fact_identities(facts) {
        let key = identity.canonical_text().to_string();
        if !seen.insert(key.clone()) {
            return Err(VerifyError::DuplicateFactIdentity(key));
        }
    }

    Ok(())
}

fn fact_identities(facts: &HirFacts) -> Vec<&FactIdentity> {
    let mut identities = Vec::new();

    identities.extend(
        facts
            .declarations
            .nominals
            .iter()
            .map(|fact| &fact.identity),
    );
    identities.extend(
        facts
            .declarations
            .callables
            .iter()
            .map(|fact| &fact.identity),
    );
    identities.extend(facts.declarations.fields.iter().map(|fact| &fact.identity));
    identities.extend(
        facts
            .declarations
            .enum_variants
            .iter()
            .map(|fact| &fact.identity),
    );
    identities.extend(facts.globals.roots.iter().map(|fact| &fact.identity));
    identities.extend(
        facts
            .globals
            .object_initializers
            .iter()
            .map(|fact| &fact.identity),
    );
    identities.extend(
        facts
            .globals
            .class_initializers
            .iter()
            .map(|fact| &fact.identity),
    );
    identities.extend(
        facts
            .native
            .extern_functions
            .iter()
            .map(|fact| &fact.identity),
    );
    identities.extend(
        facts
            .native
            .native_callables
            .iter()
            .map(|fact| &fact.identity),
    );
    identities.extend(
        facts
            .native
            .extern_globals
            .iter()
            .map(|fact| &fact.identity),
    );

    identities
}

fn verify_unique_source_sites(facts: &HirFacts) -> Result<()> {
    let mut seen = HashSet::new();

    for key in source_site_keys(facts) {
        let owner = key.owner.clone();
        let site = key.site;
        if !seen.insert(key.clone()) {
            return Err(VerifyError::DuplicateSourceSite {
                owner,
                site: SiteId::from_raw(site),
            });
        }
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SourceSiteKey {
    owner: String,
    site: u32,
    contract: &'static str,
    detail: String,
}

fn source_site_keys(facts: &HirFacts) -> Vec<SourceSiteKey> {
    let mut keys = Vec::new();

    keys.extend(
        facts
            .source_sites
            .call_sites
            .iter()
            .map(|fact| source_site_key(&fact.identity, "call", String::new())),
    );
    keys.extend(
        facts
            .source_sites
            .argument_bindings
            .iter()
            .map(|fact| source_site_key(&fact.identity, "argument", String::new())),
    );
    keys.extend(
        facts
            .source_sites
            .assignments
            .iter()
            .map(|fact| source_site_key(&fact.identity, "assignment", String::new())),
    );
    keys.extend(
        facts
            .source_sites
            .with_updates
            .iter()
            .map(|fact| source_site_key(&fact.identity, "with_update", String::new())),
    );
    keys.extend(
        facts
            .source_sites
            .perform_sites
            .iter()
            .map(|fact| source_site_key(&fact.identity, "perform", String::new())),
    );
    keys.extend(
        facts
            .source_sites
            .handle_sites
            .iter()
            .map(|fact| source_site_key(&fact.identity, "handle", String::new())),
    );
    keys.extend(
        facts
            .source_sites
            .continuation_resumes
            .iter()
            .map(|fact| source_site_key(&fact.identity, "resume", String::new())),
    );
    keys.extend(
        facts
            .source_sites
            .pattern_bindings
            .iter()
            .map(|fact| source_site_key(&fact.identity, "pattern", fact.binding_name.clone())),
    );

    keys
}

fn source_site_key(
    identity: &crate::source_sites::SourceSiteIdentity,
    contract: &'static str,
    detail: String,
) -> SourceSiteKey {
    SourceSiteKey {
        owner: identity.owner.as_str().to_string(),
        site: identity.site.as_u32(),
        contract,
        detail,
    }
}
