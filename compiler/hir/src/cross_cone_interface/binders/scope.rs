use std::fmt;
use std::num::NonZeroU32;

use scoop_identity::SignatureTypeKey;

use super::{CanonicalBinderListV1, TypeParameterBoundsV1};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureBinderScopeV1 {
    frames: SignatureBinderFrames,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SignatureBinderFrames {
    Empty,
    One(NonZeroU32),
    Two {
        inner: NonZeroU32,
        outer: NonZeroU32,
    },
}

impl SignatureBinderScopeV1 {
    pub fn for_declaration(own_arity: u32, nominal_owner_arity: Option<u32>) -> Self {
        let own = NonZeroU32::new(own_arity);
        let owner = nominal_owner_arity.and_then(NonZeroU32::new);
        let frames = match (own, owner) {
            (None, None) => SignatureBinderFrames::Empty,
            (Some(arity), None) | (None, Some(arity)) => SignatureBinderFrames::One(arity),
            (Some(inner), Some(outer)) => SignatureBinderFrames::Two { inner, outer },
        };
        Self { frames }
    }

    pub fn available_depths(&self) -> u32 {
        match &self.frames {
            SignatureBinderFrames::Empty => 0,
            SignatureBinderFrames::One(_) => 1,
            SignatureBinderFrames::Two { .. } => 2,
        }
    }

    pub fn arity_at_depth(&self, depth: u32) -> Option<u32> {
        match (&self.frames, depth) {
            (SignatureBinderFrames::One(arity), 0)
            | (SignatureBinderFrames::Two { inner: arity, .. }, 0)
            | (SignatureBinderFrames::Two { outer: arity, .. }, 1) => Some(arity.get()),
            (SignatureBinderFrames::Empty, _)
            | (SignatureBinderFrames::One(_), _)
            | (SignatureBinderFrames::Two { .. }, _) => None,
        }
    }

    pub fn validate(&self, signature: &SignatureTypeKey) -> Result<(), SignatureBinderScopeError> {
        match signature {
            SignatureTypeKey::Nominal(_) => Ok(()),
            SignatureTypeKey::NominalApplication { arguments, .. }
            | SignatureTypeKey::Tuple(arguments) => self.validate_sequence(arguments.as_slice()),
            SignatureTypeKey::Function {
                parameters, result, ..
            }
            | SignatureTypeKey::NativeFunctionPointer {
                parameters, result, ..
            } => {
                self.validate_sequence(parameters)?;
                self.validate(result)
            }
            SignatureTypeKey::RawPointer(pointee) => self.validate(pointee),
            SignatureTypeKey::Binder { depth, index } => {
                let Some(arity) = self.arity_at_depth(*depth) else {
                    return Err(SignatureBinderScopeError::DepthOutOfRange {
                        depth: *depth,
                        available_depths: self.available_depths(),
                    });
                };
                if *index >= arity {
                    return Err(SignatureBinderScopeError::IndexOutOfRange {
                        depth: *depth,
                        index: *index,
                        arity,
                    });
                }
                Ok(())
            }
        }
    }

    fn validate_sequence(
        &self,
        signatures: &[SignatureTypeKey],
    ) -> Result<(), SignatureBinderScopeError> {
        for signature in signatures {
            self.validate(signature)?;
        }
        Ok(())
    }
}

impl CanonicalBinderListV1 {
    pub fn signature_scope(&self, nominal_owner_arity: Option<u32>) -> SignatureBinderScopeV1 {
        SignatureBinderScopeV1::for_declaration(self.len_u32(), nominal_owner_arity)
    }

    pub fn validate_bound_scopes(
        &self,
        nominal_owner_arity: Option<u32>,
    ) -> Result<(), TypeParameterBinderScopeValidationError> {
        let scope = self.signature_scope(nominal_owner_arity);
        for (binder_index, binder) in self.binders().iter().enumerate() {
            let TypeParameterBoundsV1::Nominal(bounds) = binder.bounds() else {
                continue;
            };
            if let Some(class) = bounds.class() {
                scope
                    .validate(class)
                    .map_err(|error| TypeParameterBinderScopeValidationError {
                        binder_index,
                        bound: TypeParameterBoundLocation::Class,
                        error,
                    })?;
            }
            for (interface_index, interface) in bounds.interfaces().values().iter().enumerate() {
                scope.validate(interface).map_err(|error| {
                    TypeParameterBinderScopeValidationError {
                        binder_index,
                        bound: TypeParameterBoundLocation::Interface { interface_index },
                        error,
                    }
                })?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignatureBinderScopeError {
    DepthOutOfRange { depth: u32, available_depths: u32 },
    IndexOutOfRange { depth: u32, index: u32, arity: u32 },
}

impl fmt::Display for SignatureBinderScopeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DepthOutOfRange {
                depth,
                available_depths,
            } => write!(
                formatter,
                "binder depth {depth} is outside {available_depths} available scope frames"
            ),
            Self::IndexOutOfRange {
                depth,
                index,
                arity,
            } => write!(
                formatter,
                "binder index {index} is outside arity {arity} at depth {depth}"
            ),
        }
    }
}

impl std::error::Error for SignatureBinderScopeError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeParameterBoundLocation {
    Class,
    Interface { interface_index: usize },
}

impl fmt::Display for TypeParameterBoundLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Class => formatter.write_str("class bound"),
            Self::Interface { interface_index } => {
                write!(formatter, "interface bound {interface_index}")
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeParameterBinderScopeValidationError {
    pub binder_index: usize,
    pub bound: TypeParameterBoundLocation,
    pub error: SignatureBinderScopeError,
}

impl fmt::Display for TypeParameterBinderScopeValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "type parameter {} has an invalid {}: {}",
            self.binder_index, self.bound, self.error
        )
    }
}

impl std::error::Error for TypeParameterBinderScopeValidationError {}

#[cfg(test)]
mod tests;
