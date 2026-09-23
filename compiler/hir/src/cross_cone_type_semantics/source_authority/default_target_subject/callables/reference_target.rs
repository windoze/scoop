//! Borrows the actual source target without granting descriptor or access authority.
use super::*;

impl View<'_> {
    pub fn source_declaration(self) -> Result<Declaration, Error> {
        match self {
            Self::Callable(callee) => Ok(callee.declaration()),
            Self::FunctionAddress(declaration) => Ok(declaration),
            Self::Bound(bound) => match bound.source() {
                DefaultBoundCallableSourceV1::Class { callable, .. } => Ok(callable.declaration()),
                DefaultBoundCallableSourceV1::Interface { member, .. } => function(*member),
            },
            Self::LocalFunction(origin) => function(origin),
            Self::Lambda(id) | Self::AnonymousFunction(id) | Self::CallableReference(id) => {
                Ok(Declaration::Generated(id))
            }
            Self::DerivedEquality(_) => Err(Error::EqualityTarget),
        }
    }
}

impl DefaultCallableReferenceV1 {
    /// The invoke descriptor is not itself a named source callable. Its actual
    /// target must retain the source role and, for locals, the exact declaration.
    pub fn source_access_target(&self) -> Result<View<'_>, Error> {
        let target = match self.target() {
            DefaultCallableReferenceTargetV1::Named(callee)
            | DefaultCallableReferenceTargetV1::BoundExtension { callee, .. } => {
                View::Callable(callee)
            }
            DefaultCallableReferenceTargetV1::Local {
                declaration,
                callee,
            } => {
                if callee.declaration() != function(*declaration)? {
                    return Err(Error::LocalReference(*declaration));
                }
                View::LocalFunction(*declaration)
            }
            DefaultCallableReferenceTargetV1::BoundMember { callee, .. } => match callee {
                DefaultMethodCalleeV1::Callable(callee) => View::Callable(callee),
                DefaultMethodCalleeV1::Bound(bound) => View::Bound(bound),
                DefaultMethodCalleeV1::DerivedEquality { owner_type } => {
                    View::DerivedEquality(owner_type)
                }
            },
        };
        if !matches!(target, View::DerivedEquality(_)) {
            let declaration = target.source_declaration()?;
            if matches!(declaration, Declaration::Generated(_)) {
                return Err(Error::CallableRole(declaration));
            }
        }
        Ok(target)
    }
}
