use std::fmt;

use scoop_identity::SignatureTypeKey;

use crate::{SignatureBinderScopeError, SignatureBinderScopeV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DefaultTemplateProviderShapeV1 {
    nominal_owner_binder_arity: u32,
    callable_own_binder_arity: u32,
    binder_arity: u32,
}

impl DefaultTemplateProviderShapeV1 {
    pub fn try_new(
        nominal_owner_binder_arity: u32,
        callable_own_binder_arity: u32,
    ) -> Result<Self, DefaultTemplateProviderShapeBuildError> {
        let binder_arity = nominal_owner_binder_arity
            .checked_add(callable_own_binder_arity)
            .ok_or(
                DefaultTemplateProviderShapeBuildError::BinderArityOverflow {
                    nominal_owner: nominal_owner_binder_arity,
                    callable_own: callable_own_binder_arity,
                },
            )?;
        Ok(Self {
            nominal_owner_binder_arity,
            callable_own_binder_arity,
            binder_arity,
        })
    }

    pub const fn nominal_owner_binder_arity(self) -> u32 {
        self.nominal_owner_binder_arity
    }

    pub const fn callable_own_binder_arity(self) -> u32 {
        self.callable_own_binder_arity
    }

    pub const fn binder_arity(self) -> u32 {
        self.binder_arity
    }

    pub fn signature_scope(self) -> SignatureBinderScopeV1 {
        SignatureBinderScopeV1::for_declaration(
            self.callable_own_binder_arity,
            (self.nominal_owner_binder_arity != 0).then_some(self.nominal_owner_binder_arity),
        )
    }

    /// Returns the canonical identity argument in flattened host/own order.
    pub(crate) fn identity_binder_at(self, position: u32) -> Option<SignatureTypeKey> {
        if position >= self.binder_arity {
            return None;
        }
        Some(if position < self.nominal_owner_binder_arity {
            SignatureTypeKey::Binder {
                depth: u32::from(self.callable_own_binder_arity != 0),
                index: position,
            }
        } else {
            SignatureTypeKey::Binder {
                depth: 0,
                index: position - self.nominal_owner_binder_arity,
            }
        })
    }

    pub(crate) fn flattened_binder_position(
        self,
        depth: u32,
        index: u32,
    ) -> Result<u32, SignatureBinderScopeError> {
        self.signature_scope()
            .validate(&scoop_identity::SignatureTypeKey::Binder { depth, index })?;
        if self.nominal_owner_binder_arity != 0 && self.callable_own_binder_arity != 0 && depth == 0
        {
            Ok(self.nominal_owner_binder_arity + index)
        } else {
            Ok(index)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultTemplateProviderShapeBuildError {
    BinderArityOverflow {
        nominal_owner: u32,
        callable_own: u32,
    },
}

impl fmt::Display for DefaultTemplateProviderShapeBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BinderArityOverflow {
                nominal_owner,
                callable_own,
            } => write!(
                formatter,
                "default-template provider binder arities {nominal_owner} + {callable_own} exceed u32"
            ),
        }
    }
}

impl std::error::Error for DefaultTemplateProviderShapeBuildError {}
