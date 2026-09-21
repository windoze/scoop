//! Artifact binding for default target declaration access sources.
use super::binding_keys;
use crate::*;
use scoop_identity::{ConeIdentity, DefinitionOriginSubject as Subject, SourceDeclarationKey};
use scoop_wire::{BudgetMeter, WireError, WirePath};
use std::collections::{BTreeMap, BTreeSet};

mod errors;
mod inventory;
mod keys;
mod replay;
pub use errors::*;
type Error = DefaultSourceAccessBindingError;

/// Complete local declaration access provenance, without target-domain,
/// receiver, profile, or executable default authority.
#[derive(Debug)]
pub struct BoundDefaultSourceAccessDeclarationsV1<'s, 'a, 'f> {
    pub(super) foundation: &'a BoundTypeFoundationSourcesV1<'f>,
    source: &'s CanonicalDefaultSourceAccessDeclarationsV1,
    keys: BTreeMap<Subject, &'f SourceDeclarationKey>,
}
impl<'f> BoundTypeFoundationSourcesV1<'f> {
    pub fn bind_default_access_declarations<'s, 'a>(
        &'a self,
        source: &'s CanonicalDefaultSourceAccessDeclarationsV1,
        required: &BTreeSet<Subject>,
        meter: &mut BudgetMeter,
    ) -> Result<BoundDefaultSourceAccessDeclarationsV1<'s, 'a, 'f>, Error> {
        let path = WirePath::root();
        meter.check_semantic_depth(1, &path)?;
        meter.charge_nodes(1, &path)?;
        let keys = inventory::bind(self, source, required, meter)?;
        let bound = BoundDefaultSourceAccessDeclarationsV1 {
            foundation: self,
            source,
            keys,
        };
        replay::validate(&bound, meter)?;
        Ok(bound)
    }
}
impl<'s, 'a, 'f> BoundDefaultSourceAccessDeclarationsV1<'s, 'a, 'f> {
    pub fn provider(&self) -> ConeIdentity {
        self.foundation.source().entries().provider
    }
    pub const fn table(&self) -> &'s CanonicalDefaultSourceAccessDeclarationsV1 {
        self.source
    }
    pub fn declaration(
        &self,
        subject: Subject,
        meter: &mut BudgetMeter,
    ) -> Result<&'s DefaultSourceAccessDeclarationV1, Error> {
        query(self.source.records().len(), meter, &WirePath::root())?;
        self.source
            .get(subject)
            .ok_or(Error::MissingRecord(subject))
    }
    /// Accessors use the source key of their verified logical property owner.
    pub fn source_key(
        &self,
        subject: Subject,
        meter: &mut BudgetMeter,
    ) -> Result<&'f SourceDeclarationKey, Error> {
        query(self.keys.len(), meter, &WirePath::root())?;
        self.keys
            .get(&subject)
            .copied()
            .ok_or(Error::MissingKey(subject))
    }
}
fn query(count: usize, meter: &mut BudgetMeter, path: &WirePath) -> Result<(), Error> {
    meter.charge_work((u64::from(count.max(1).ilog2()) + 1) * 65, path)?;
    Ok(())
}
fn nominal_subject(owner: SourceNominalId) -> Subject {
    match owner {
        SourceNominalId::Concrete(id) => Subject::Type(id),
        SourceNominalId::GenericTemplate(id) => Subject::GenericType(id),
    }
}
