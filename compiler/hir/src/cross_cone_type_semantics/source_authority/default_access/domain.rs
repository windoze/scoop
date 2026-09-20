use crate::{CanonicalPersistentIdsV1, PersistentAccessDomainV1};
use scoop_identity::PersistentGenericTypeId;

use super::DefaultSourceAccessBuildError;

mod wire;
pub use wire::*;

/// The conjunction of persistent regions and actual, uninstantiated source
/// class regions. Generic ids are not concrete exact-type substitutes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultSourceAccessDomainV1 {
    persistent: PersistentAccessDomainV1,
    generic_subclasses: CanonicalPersistentIdsV1<PersistentGenericTypeId>,
}
impl DefaultSourceAccessDomainV1 {
    pub fn try_new(
        persistent: PersistentAccessDomainV1,
        generic_subclasses: CanonicalPersistentIdsV1<PersistentGenericTypeId>,
    ) -> Result<Self, DefaultSourceAccessBuildError> {
        if persistent.is_empty() && !generic_subclasses.is_empty() {
            return Err(DefaultSourceAccessBuildError::GenericConstraintsOnEmpty);
        }
        Ok(Self {
            persistent,
            generic_subclasses,
        })
    }
    pub const fn empty() -> Self {
        Self {
            persistent: PersistentAccessDomainV1::empty(),
            generic_subclasses: CanonicalPersistentIdsV1::empty(),
        }
    }
    pub const fn universal() -> Self {
        Self {
            persistent: PersistentAccessDomainV1::universal(),
            generic_subclasses: CanonicalPersistentIdsV1::empty(),
        }
    }
    pub const fn persistent(&self) -> &PersistentAccessDomainV1 {
        &self.persistent
    }
    pub const fn generic_subclasses(&self) -> &CanonicalPersistentIdsV1<PersistentGenericTypeId> {
        &self.generic_subclasses
    }
    pub fn is_empty(&self) -> bool {
        self.persistent.is_empty()
    }
    pub fn is_universal(&self) -> bool {
        self.persistent.is_universal() && self.generic_subclasses.is_empty()
    }
}
