use std::fmt;

use super::NominalIntrinsicRepresentationV1;
use crate::{
    CanonicalBinderListV1, IntrinsicTypeKind, IntrinsicTypeParameters, TypeParameterBoundsV1,
};

impl NominalIntrinsicRepresentationV1 {
    /// Replays the source binder contract without selecting an owner by name or provider.
    pub fn validate_binders(
        self,
        binders: &CanonicalBinderListV1,
    ) -> Result<(), NominalIntrinsicBinderError> {
        let expected_bounds = match self.family().parameters() {
            IntrinsicTypeParameters::None => None,
            IntrinsicTypeParameters::OneInvariantUnconstrained => {
                Some(TypeParameterBoundsV1::Unconstrained)
            }
            IntrinsicTypeParameters::OneInvariantValue => Some(TypeParameterBoundsV1::Value),
            IntrinsicTypeParameters::OneInvariantRef => Some(TypeParameterBoundsV1::Ref),
        };
        let expected = u32::from(expected_bounds.is_some());
        let actual = binders.len_u32();
        if actual != expected {
            return Err(NominalIntrinsicBinderError::Arity {
                family: self.family(),
                expected,
                actual,
            });
        }
        if let Some(bounds) = expected_bounds
            && binders.binders().first().map(|binder| binder.bounds()) != Some(&bounds)
        {
            return Err(NominalIntrinsicBinderError::Bounds {
                family: self.family(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NominalIntrinsicBinderError {
    Arity {
        family: IntrinsicTypeKind,
        expected: u32,
        actual: u32,
    },
    Bounds {
        family: IntrinsicTypeKind,
    },
}

impl fmt::Display for NominalIntrinsicBinderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arity {
                family,
                expected,
                actual,
            } => write!(
                f,
                "intrinsic nominal {family:?} has {actual} binders, expected {expected}"
            ),
            Self::Bounds { family } => {
                write!(
                    f,
                    "intrinsic nominal {family:?} has incorrect type parameter bounds"
                )
            }
        }
    }
}
impl std::error::Error for NominalIntrinsicBinderError {}
