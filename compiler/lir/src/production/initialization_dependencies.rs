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
