use scoop_identity::{PersistentExactTypeId, SourceDeclarationKey, SourceDeclarationKind};

use super::{
    AccessDomainSemanticError, CheckedDeclarationAccessSourceV1, CheckedPersistentAccessDomainV1,
    DeclaredVisibilityV1,
};
use crate::{CheckedNominalInheritanceGraphV1, DirectClassBaseV1, SourceNominalId};

/// Access proof for a source declaration, still separate from the typed
/// declaration/slot selection and from any MIR external-target authority.
#[derive(Debug)]
pub struct CheckedProtectedDeclarationDomainV1<'g, 'a, 's> {
    declaration: &'s SourceDeclarationKey,
    owner: PersistentExactTypeId,
    domain: CheckedPersistentAccessDomainV1<'g, 'a>,
}
impl<'a> CheckedNominalInheritanceGraphV1<'a> {
    pub fn protected_declaration_domain<'g, 's>(
        &'g self,
        source: &'s CheckedDeclarationAccessSourceV1<'s>,
    ) -> Result<CheckedProtectedDeclarationDomainV1<'g, 'a, 's>, AccessDomainSemanticError> {
        if source.source().declared_visibility() != DeclaredVisibilityV1::Protected {
            return Err(AccessDomainSemanticError::NotProtectedMember);
        }
        let owner = source
            .source()
            .lexical_owners()
            .last()
            .ok_or(AccessDomainSemanticError::NotProtectedMember)?;
        let owner = self.source_exact(*owner)?;
        self.require_class(owner)?;
        let domains = self.replay_declaration_access(*source)?;
        Ok(CheckedProtectedDeclarationDomainV1 {
            declaration: source.declaration(),
            owner,
            domain: domains.lookup,
        })
    }
}
impl CheckedProtectedDeclarationDomainV1<'_, '_, '_> {
    pub const fn declaration(&self) -> &SourceDeclarationKey {
        self.declaration
    }
    pub const fn owner(&self) -> PersistentExactTypeId {
        self.owner
    }
    pub fn domain(&self) -> &crate::PersistentAccessDomainV1 {
        self.domain.domain()
    }

    fn authorize(
        &self,
        scope: SourceNominalId,
        receiver: Option<PersistentExactTypeId>,
    ) -> Result<ProtectedLexicalAuthorizationV1, AccessDomainSemanticError> {
        if !self.domain.allows_scope(scope)? {
            return Err(AccessDomainSemanticError::OutsideDomain);
        }
        for access_class in self.domain.graph.scope_classes(scope)? {
            if self.domain.graph.is_subclass(access_class, self.owner)?
                && match receiver {
                    Some(receiver) => self
                        .domain
                        .graph
                        .receiver_is_access_subtype(receiver, access_class)?,
                    None => true,
                }
            {
                return Ok(ProtectedLexicalAuthorizationV1 {
                    scope,
                    access_class: self.domain.graph.access_class_exact(access_class)?,
                    access_subject: access_class,
                    declaring_class: self.owner,
                });
            }
        }
        Err(AccessDomainSemanticError::ProtectedReceiver)
    }

    pub fn explicit_receiver(
        &self,
        scope: SourceNominalId,
        receiver: PersistentExactTypeId,
    ) -> Result<ProtectedExplicitReceiverDomainAccessV1, AccessDomainSemanticError> {
        self.require_member_value()?;
        Ok(ProtectedExplicitReceiverDomainAccessV1 {
            authorization: self.authorize(scope, Some(receiver))?,
            receiver,
        })
    }

    pub fn implicit_this(
        &self,
        scope: SourceNominalId,
    ) -> Result<ProtectedImplicitThisDomainAccessV1, AccessDomainSemanticError> {
        self.require_member_value()?;
        let receiver = self.domain.graph.source_exact(scope)?;
        self.domain
            .graph
            .access_class_exact(receiver)
            .map_err(|_| AccessDomainSemanticError::NoImplicitThis)?;
        let authorization = self.authorize(scope, Some(receiver))?;
        if authorization.access_subject != receiver {
            return Err(AccessDomainSemanticError::NoImplicitThis);
        }
        Ok(ProtectedImplicitThisDomainAccessV1 { authorization })
    }

    pub fn constructor_delegation(
        &self,
        scope: SourceNominalId,
    ) -> Result<ProtectedConstructorDelegationDomainAccessV1, AccessDomainSemanticError> {
        if self.declaration.declaration_kind() != SourceDeclarationKind::Constructor {
            return Err(AccessDomainSemanticError::InvalidPurpose);
        }
        let current = self.domain.graph.source_exact(scope)?;
        self.domain.graph.access_class_exact(current)?;
        let node = self
            .domain
            .graph
            .get(current)
            .ok_or(AccessDomainSemanticError::InvalidDelegation)?;
        if current != self.owner
            && node.edges().direct_base() != (DirectClassBaseV1::ClassBase { exact: self.owner })
        {
            return Err(AccessDomainSemanticError::InvalidDelegation);
        }
        let authorization = self.authorize(scope, Some(current))?;
        Ok(ProtectedConstructorDelegationDomainAccessV1 { authorization })
    }

    pub fn qualified_super(
        &self,
        scope: SourceNominalId,
        qualifier: PersistentExactTypeId,
    ) -> Result<ProtectedSuperDomainAccessV1, AccessDomainSemanticError> {
        self.require_member_value()?;
        let current = self.domain.graph.source_exact(scope)?;
        self.domain.graph.access_class_exact(current)?;
        let node = self
            .domain
            .graph
            .get(current)
            .ok_or(AccessDomainSemanticError::InvalidSuper)?;
        if node.edges().direct_base() != (DirectClassBaseV1::ClassBase { exact: qualifier })
            || !self.domain.graph.is_subclass(qualifier, self.owner)?
        {
            return Err(AccessDomainSemanticError::InvalidSuper);
        }
        Ok(ProtectedSuperDomainAccessV1 {
            authorization: self.authorize(scope, Some(current))?,
            qualifier,
        })
    }

    pub fn nested_type(
        &self,
        scope: SourceNominalId,
    ) -> Result<ProtectedNestedTypeDomainAccessV1, AccessDomainSemanticError> {
        if SourceNominalId::from_source_declaration(self.declaration).is_err() {
            return Err(AccessDomainSemanticError::InvalidPurpose);
        }
        Ok(ProtectedNestedTypeDomainAccessV1 {
            authorization: self.authorize(scope, None)?,
        })
    }

    fn require_member_value(&self) -> Result<(), AccessDomainSemanticError> {
        if matches!(
            self.declaration.declaration_kind(),
            SourceDeclarationKind::Function | SourceDeclarationKind::Property
        ) {
            Ok(())
        } else {
            Err(AccessDomainSemanticError::InvalidPurpose)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedLexicalAuthorizationV1 {
    scope: SourceNominalId,
    access_class: PersistentExactTypeId,
    access_subject: PersistentExactTypeId,
    declaring_class: PersistentExactTypeId,
}
impl ProtectedLexicalAuthorizationV1 {
    pub const fn scope(&self) -> SourceNominalId {
        self.scope
    }
    pub const fn access_class(&self) -> PersistentExactTypeId {
        self.access_class
    }
    pub const fn access_subject(&self) -> PersistentExactTypeId {
        self.access_subject
    }
    pub const fn declaring_class(&self) -> PersistentExactTypeId {
        self.declaring_class
    }
}

macro_rules! authorization_access {
    ($($name:ident),+ $(,)?) => {$(
        impl $name {
            pub const fn authorization(&self) -> ProtectedLexicalAuthorizationV1 { self.authorization }
        }
    )+};
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedExplicitReceiverDomainAccessV1 {
    authorization: ProtectedLexicalAuthorizationV1,
    receiver: PersistentExactTypeId,
}
impl ProtectedExplicitReceiverDomainAccessV1 {
    pub const fn receiver(&self) -> PersistentExactTypeId {
        self.receiver
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedImplicitThisDomainAccessV1 {
    authorization: ProtectedLexicalAuthorizationV1,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedConstructorDelegationDomainAccessV1 {
    authorization: ProtectedLexicalAuthorizationV1,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedSuperDomainAccessV1 {
    authorization: ProtectedLexicalAuthorizationV1,
    qualifier: PersistentExactTypeId,
}
impl ProtectedSuperDomainAccessV1 {
    pub const fn qualifier(&self) -> PersistentExactTypeId {
        self.qualifier
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedNestedTypeDomainAccessV1 {
    authorization: ProtectedLexicalAuthorizationV1,
}
authorization_access!(
    ProtectedExplicitReceiverDomainAccessV1,
    ProtectedImplicitThisDomainAccessV1,
    ProtectedConstructorDelegationDomainAccessV1,
    ProtectedSuperDomainAccessV1,
    ProtectedNestedTypeDomainAccessV1
);
