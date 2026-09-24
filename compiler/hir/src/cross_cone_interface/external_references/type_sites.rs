//! Type occurrences retained by the ordinary external-reference metadata.

use scoop_identity::{ConcreteExpressionOrigin, PersistentExactTypeId};
use scoop_wire::{Encoder, WireEncode};

use crate::concrete::ExecutableExpressionPosition;

mod decode;
mod errors;
mod nominals;
mod relations;
mod role;
mod table;
#[cfg(test)]
mod tests;

pub use decode::DecodedHirDependencyTypeSiteV1;
pub use errors::{HirDependencyTypeSiteBuildError, HirDependencyTypeSiteResolutionError};
pub use nominals::{HirTypeSiteExactError, collect_type_site_nominals};
pub use relations::HirDependencyTypeRelationError;
pub use role::HirExpressionTypeRoleV1;
pub use table::{CanonicalHirDependencyTypeSitesV1, DecodedCanonicalHirDependencyTypeSitesV1};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDependencyTypeSiteV1 {
    position: ExecutableExpressionPosition,
    origin: ConcreteExpressionOrigin,
    role: HirExpressionTypeRoleV1,
    exact: PersistentExactTypeId,
}

impl HirDependencyTypeSiteV1 {
    pub const fn new(
        position: ExecutableExpressionPosition,
        origin: ConcreteExpressionOrigin,
        role: HirExpressionTypeRoleV1,
        exact: PersistentExactTypeId,
    ) -> Self {
        Self {
            position,
            origin,
            role,
            exact,
        }
    }

    pub const fn position(&self) -> ExecutableExpressionPosition {
        self.position
    }

    pub const fn origin(&self) -> &ConcreteExpressionOrigin {
        &self.origin
    }

    pub const fn role(&self) -> HirExpressionTypeRoleV1 {
        self.role
    }

    pub const fn exact(&self) -> PersistentExactTypeId {
        self.exact
    }

    pub const fn sort_key(&self) -> (ExecutableExpressionPosition, HirExpressionTypeRoleV1) {
        (self.position, self.role)
    }
}

impl WireEncode for HirDependencyTypeSiteV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.position.root.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.position.expression_index))?;
        encoder.field(3)?;
        self.origin.encode(encoder)?;
        encoder.field(4)?;
        self.role.encode(encoder)?;
        encoder.field(5)?;
        self.exact.encode(encoder)
    }
}
