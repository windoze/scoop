//! Intrinsic type checks follow the value's exact source nominal reference.

use std::fmt;

use scoop_hir::{IntrinsicTypeKind, NominalSourceShapeV1, PublicNominalKindV1, SourceNominalId};
use scoop_identity::{PersistentGenericTypeId, PersistentTypeId, SourceDeclarationKey};
use scoop_wire::{WireError, WirePath};

use super::{CanonicalCrossConeHirSurfaceAuthority, CrossConeHirNominalAuthorityError};

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(super) fn validate_intrinsic_type(
        &mut self,
        declaration: SourceNominalId,
        expected: IntrinsicTypeKind,
    ) -> Result<(), CrossConeHirIntrinsicTypeError> {
        use CrossConeHirIntrinsicTypeError as Error;
        let path = WirePath::root();
        self.meter.charge_nodes(1, &path).map_err(Error::Resource)?;
        self.meter
            .charge_work(self.dependencies.len() as u64 + 1, &path)
            .map_err(Error::Resource)?;
        let key = match declaration {
            SourceNominalId::Concrete(id) => self
                .identities
                .canonical_key::<PersistentTypeId, SourceDeclarationKey>(id),
            SourceNominalId::GenericTemplate(id) => self
                .identities
                .canonical_key::<PersistentGenericTypeId, SourceDeclarationKey>(id),
        }
        .map_err(|error| Error::Nominal(CrossConeHirNominalAuthorityError::Identity(error)))?;
        let interface = self
            .provider_interface(key.origin())
            .map_err(Error::Nominal)?;
        let lookup_work = u64::from(
            interface
                .nominal_interfaces()
                .records()
                .len()
                .max(1)
                .ilog2(),
        ) + 1;
        self.meter
            .charge_work(lookup_work, &path)
            .map_err(Error::Resource)?;
        let record =
            Self::checked_nominal_record(interface, declaration, &key).map_err(Error::Nominal)?;
        let NominalSourceShapeV1::Intrinsic(representation) = record.source_shape() else {
            return Err(Error::NotIntrinsic {
                declaration,
                kind: record.kind(),
            });
        };
        let actual = representation.family();
        if actual != expected {
            return Err(Error::Family {
                declaration,
                expected,
                actual,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CrossConeHirIntrinsicTypeError {
    Resource(WireError),
    Nominal(CrossConeHirNominalAuthorityError),
    NotIntrinsic {
        declaration: SourceNominalId,
        kind: PublicNominalKindV1,
    },
    Family {
        declaration: SourceNominalId,
        expected: IntrinsicTypeKind,
        actual: IntrinsicTypeKind,
    },
}

impl fmt::Display for CrossConeHirIntrinsicTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Nominal(error) => error.fmt(f),
            Self::NotIntrinsic { declaration, kind } => write!(
                f,
                "nominal {declaration:?} is an ordinary {kind:?}, expected an intrinsic declaration"
            ),
            Self::Family {
                declaration,
                expected,
                actual,
            } => write!(
                f,
                "nominal {declaration:?} has intrinsic family {actual:?}, expected {expected:?}"
            ),
        }
    }
}

impl std::error::Error for CrossConeHirIntrinsicTypeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Nominal(error) => Some(error),
            Self::NotIntrinsic { .. } | Self::Family { .. } => None,
        }
    }
}
