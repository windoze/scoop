use std::fmt;

use scoop_wire::{WireError, WirePath};

use super::{
    CheckedDeclarationAccessSourceV1, DeclarationAccessSourceV1, DeclaredVisibilityV1,
    PersistentAccessConstraintV1 as Constraint, PersistentAccessDomainError,
    PersistentAccessDomainV1,
};
use crate::{CheckedNominalInheritanceGraphV1, InheritanceQueryError, SourceNominalId};

mod implication;
mod nominal;
mod normalization;
mod protected;
#[cfg(test)]
mod tests;

pub use nominal::*;
pub use protected::*;

/// A domain whose identities and semantic normal form were replayed against
/// one checked inheritance/lexical graph. It grants no declaration selection.
#[derive(Debug)]
pub struct CheckedPersistentAccessDomainV1<'g, 'a> {
    graph: &'g CheckedNominalInheritanceGraphV1<'a>,
    domain: PersistentAccessDomainV1,
}
impl CheckedPersistentAccessDomainV1<'_, '_> {
    pub const fn domain(&self) -> &PersistentAccessDomainV1 {
        &self.domain
    }
}

#[derive(Debug)]
pub struct ReplayedDeclarationAccessDomainsV1<'g, 'a> {
    declared: CheckedPersistentAccessDomainV1<'g, 'a>,
    lookup: CheckedPersistentAccessDomainV1<'g, 'a>,
}
impl<'g, 'a> ReplayedDeclarationAccessDomainsV1<'g, 'a> {
    pub const fn declared(&self) -> &CheckedPersistentAccessDomainV1<'g, 'a> {
        &self.declared
    }
    pub const fn lookup(&self) -> &CheckedPersistentAccessDomainV1<'g, 'a> {
        &self.lookup
    }
}

impl<'a> CheckedNominalInheritanceGraphV1<'a> {
    pub fn validate_access_domain<'g>(
        &'g self,
        domain: &PersistentAccessDomainV1,
    ) -> Result<CheckedPersistentAccessDomainV1<'g, 'a>, AccessDomainSemanticError> {
        let normalized = normalization::normalize(self, domain)?;
        if &normalized != domain {
            return Err(AccessDomainSemanticError::NonCanonicalEmpty);
        }
        Ok(CheckedPersistentAccessDomainV1 {
            graph: self,
            domain: normalized,
        })
    }

    pub fn replay_nominal_access<'g>(
        &'g self,
        owner: SourceNominalId,
    ) -> Result<ReplayedDeclarationAccessDomainsV1<'g, 'a>, AccessDomainSemanticError> {
        let source = self
            .source(owner)
            .ok_or(InheritanceQueryError::UnknownSource(owner))?;
        self.replay_access(source.access)
    }

    pub fn replay_declaration_access<'g>(
        &'g self,
        source: CheckedDeclarationAccessSourceV1<'_>,
    ) -> Result<ReplayedDeclarationAccessDomainsV1<'g, 'a>, AccessDomainSemanticError> {
        self.replay_access(source.source())
    }

    pub(in crate::cross_cone_type_semantics) fn replay_access<'g>(
        &'g self,
        source: &DeclarationAccessSourceV1,
    ) -> Result<ReplayedDeclarationAccessDomainsV1<'g, 'a>, AccessDomainSemanticError> {
        let declared = self.declared_domain(source)?;
        let mut lookup = declared.clone();
        for owner in source.lexical_owners() {
            let ancestor = self
                .source(*owner)
                .ok_or(InheritanceQueryError::UnknownSource(*owner))?;
            if ancestor.access.definition_origin().origin().source()
                != source.definition_origin().origin().source()
            {
                return Err(AccessDomainSemanticError::OwnerSource(*owner));
            }
            let domain = self.declared_domain(ancestor.access)?;

            lookup = lookup
                .intersect(&domain)
                .map_err(AccessDomainSemanticError::Encoding)?;
        }
        let declared = normalization::normalize(self, &declared)?;
        let lookup = normalization::normalize(self, &lookup)?;
        Ok(ReplayedDeclarationAccessDomainsV1 {
            declared: CheckedPersistentAccessDomainV1 {
                graph: self,
                domain: declared,
            },
            lookup: CheckedPersistentAccessDomainV1 {
                graph: self,
                domain: lookup,
            },
        })
    }

    fn declared_domain(
        &self,
        source: &DeclarationAccessSourceV1,
    ) -> Result<PersistentAccessDomainV1, AccessDomainSemanticError> {
        let constraints = match (source.declared_visibility(), source.lexical_owners().last()) {
            (DeclaredVisibilityV1::Public, _) => vec![],
            (DeclaredVisibilityV1::Internal, _) => vec![Constraint::Cone(
                source.definition_origin().origin().source().cone(),
            )],
            (DeclaredVisibilityV1::Private, None) => vec![
                Constraint::Cone(source.definition_origin().origin().source().cone()),
                Constraint::File(source.definition_origin().origin().source().clone()),
            ],
            (DeclaredVisibilityV1::Private, Some(owner)) => vec![Constraint::LexicalOwner(*owner)],
            (DeclaredVisibilityV1::Protected, Some(owner)) => {
                let exact = self.source_exact(*owner)?;
                self.require_class(exact)?;
                vec![Constraint::SubclassesOf(exact)]
            }
            (DeclaredVisibilityV1::Protected, None) => {
                return Err(AccessDomainSemanticError::NotProtectedMember);
            }
        };

        PersistentAccessDomainV1::try_from_constraints(constraints)
            .map_err(AccessDomainSemanticError::Encoding)
    }

    fn scope_classes(
        &self,
        scope: SourceNominalId,
    ) -> Result<Vec<scoop_identity::PersistentExactTypeId>, AccessDomainSemanticError> {
        let source = self
            .source(scope)
            .ok_or(InheritanceQueryError::UnknownSource(scope))?;
        let mut classes = Vec::new();
        scoop_wire::allocation::try_reserve(
            &mut classes,
            source.access.lexical_owners().len() + 1,
            &WirePath::root(),
        )
        .map_err(AccessDomainSemanticError::Resource)?;
        for owner in std::iter::once(&scope).chain(source.access.lexical_owners().iter().rev()) {
            if self.is_class_access_scope(*owner)? {
                let exact = self.source_exact(*owner)?;
                self.access_class_exact(exact)?;
                classes.push(exact);
            }
        }
        Ok(classes)
    }
}

#[derive(Debug)]
pub enum AccessDomainSemanticError {
    Resource(WireError),
    Inheritance(InheritanceQueryError),
    Encoding(PersistentAccessDomainError),
    NonCanonicalEmpty,
    DifferentGraph,
    OwnerSource(SourceNominalId),
    NotProtectedMember,
    OutsideDomain,
    ProtectedReceiver,
    NoImplicitThis,
    InvalidDelegation,
    InvalidSuper,
    InvalidPurpose,
}
impl From<InheritanceQueryError> for AccessDomainSemanticError {
    fn from(error: InheritanceQueryError) -> Self {
        Self::Inheritance(error)
    }
}
impl fmt::Display for AccessDomainSemanticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Inheritance(error) => error.fmt(f),
            Self::Encoding(error) => error.fmt(f),
            Self::NonCanonicalEmpty => {
                f.write_str("incompatible access constraints must encode Empty")
            }
            Self::DifferentGraph => {
                f.write_str("access domains belong to different inheritance closures")
            }
            Self::OwnerSource(owner) => {
                write!(f, "access owner {owner:?} belongs to another source")
            }
            Self::NotProtectedMember => {
                f.write_str("access source is not a protected class member")
            }
            Self::OutsideDomain => {
                f.write_str("lexical access point is outside the effective target domain")
            }
            Self::ProtectedReceiver => {
                f.write_str("protected receiver is not an access subclass or its subtype")
            }
            Self::NoImplicitThis => {
                f.write_str("lexical access scope has no applicable implicit this receiver")
            }
            Self::InvalidDelegation => f.write_str(
                "protected constructor delegation is not to self or the direct class base",
            ),
            Self::InvalidSuper => {
                f.write_str("protected super access does not follow the direct class base")
            }
            Self::InvalidPurpose => {
                f.write_str("protected declaration kind does not admit this access purpose")
            }
        }
    }
}
impl std::error::Error for AccessDomainSemanticError {}
