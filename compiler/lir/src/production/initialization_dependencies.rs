//! Typed definition references for the unchanged initialization dependency wire.
//!
//! This catalog proves that an id names a complete Strong unit definition. The
//! artifact reader must build it from the selected dependency closure and join
//! every foreign edge to its committed initialization-use relation before
//! publishing a `strong-production/2` view.

use scoop_identity::{ConeIdentity, PersistentInitializationUnitId};

mod definition;
mod resolve;

pub use definition::*;
pub use resolve::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongInitializationDependencyKindV2<'a> {
    LocalUnit(&'a StrongInitializationUnitDefinitionRefV2),
    DependencyExternalUnit {
        provider: ConeIdentity,
        unit_ref: &'a StrongInitializationUnitDefinitionRefV2,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DependencyBody {
    Local(StrongInitializationUnitDefinitionRefV2),
    External(StrongInitializationUnitDefinitionRefV2),
}

/// The provider is obtained from the checked unit definition, never from a
/// caller-supplied label paired with an unrelated unit id.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongInitializationDependencyRefV2(DependencyBody);

impl StrongInitializationDependencyRefV2 {
    pub const fn kind(&self) -> StrongInitializationDependencyKindV2<'_> {
        match &self.0 {
            DependencyBody::Local(unit_ref) => {
                StrongInitializationDependencyKindV2::LocalUnit(unit_ref)
            }
            DependencyBody::External(unit_ref) => {
                StrongInitializationDependencyKindV2::DependencyExternalUnit {
                    provider: unit_ref.provider(),
                    unit_ref,
                }
            }
        }
    }

    pub const fn definition(&self) -> &StrongInitializationUnitDefinitionRefV2 {
        match &self.0 {
            DependencyBody::Local(unit) | DependencyBody::External(unit) => unit,
        }
    }

    pub const fn unit(&self) -> PersistentInitializationUnitId {
        self.definition().unit()
    }
}

impl scoop_wire::WireEncode for StrongInitializationDependencyRefV2 {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.unit().encode(encoder)
    }
}

/// The two closed initialization dependency representations accepted by Strong
/// registration plans. V2 references must retain their role in the consumer.
pub trait StrongInitializationDependencyReference: dependency_sealed::Sealed + Clone {
    fn unit_id(&self) -> PersistentInitializationUnitId;
    fn has_valid_provider_role(&self, producer: ConeIdentity) -> bool;
}

mod dependency_sealed {
    pub trait Sealed {}
    impl Sealed for super::PersistentInitializationUnitId {}
    impl Sealed for super::StrongInitializationDependencyRefV2 {}
}

impl StrongInitializationDependencyReference for PersistentInitializationUnitId {
    fn unit_id(&self) -> PersistentInitializationUnitId {
        *self
    }
    fn has_valid_provider_role(&self, _: ConeIdentity) -> bool {
        true
    }
}

impl StrongInitializationDependencyReference for StrongInitializationDependencyRefV2 {
    fn unit_id(&self) -> PersistentInitializationUnitId {
        self.unit()
    }
    fn has_valid_provider_role(&self, producer: ConeIdentity) -> bool {
        match self.kind() {
            StrongInitializationDependencyKindV2::LocalUnit(definition) => {
                definition.provider() == producer
            }
            StrongInitializationDependencyKindV2::DependencyExternalUnit { provider, .. } => {
                provider != producer
            }
        }
    }
}
