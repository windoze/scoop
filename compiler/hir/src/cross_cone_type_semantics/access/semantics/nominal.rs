use scoop_identity::{PersistentExactTypeId, SourceDeclarationKind};

use super::{AccessDomainSemanticError, CheckedPersistentAccessDomainV1};
use crate::{
    CheckedNominalInheritanceGraphV1, InheritanceQueryError, NominalAccessDomainsV1,
    NominalInheritanceModalityV1, PersistentAccessConstraintV1, PersistentAccessDomainV1,
    PersistentInheritanceDomainV1, PersistentLookupDomainV1, PersistentSlotContractDomainV1,
};

/// Replayed nominal regions, not callable-slot or declaration-selection authority.
#[derive(Debug)]
pub struct CheckedNominalAccessDomainsV1<'g, 'a> {
    owner: PersistentExactTypeId,
    lookup: CheckedPersistentAccessDomainV1<'g, 'a>,
    inheritance: CheckedPersistentAccessDomainV1<'g, 'a>,
}
impl CheckedNominalAccessDomainsV1<'_, '_> {
    pub const fn owner(&self) -> PersistentExactTypeId {
        self.owner
    }
    pub const fn lookup(&self) -> &CheckedPersistentAccessDomainV1<'_, '_> {
        &self.lookup
    }
    pub const fn inheritance(&self) -> &CheckedPersistentAccessDomainV1<'_, '_> {
        &self.inheritance
    }
    pub fn to_record(&self) -> NominalAccessDomainsV1 {
        NominalAccessDomainsV1::new(
            PersistentLookupDomainV1::new(self.lookup.domain().clone()),
            PersistentInheritanceDomainV1::new(self.inheritance.domain().clone()),
            PersistentSlotContractDomainV1::new(PersistentAccessDomainV1::empty()),
        )
    }
}

impl<'a> CheckedNominalInheritanceGraphV1<'a> {
    pub fn replay_nominal_domains<'g>(
        &'g self,
        owner: PersistentExactTypeId,
    ) -> Result<CheckedNominalAccessDomainsV1<'g, 'a>, AccessDomainSemanticError> {
        let node = self
            .get(owner)
            .ok_or(InheritanceQueryError::UnknownExact(owner))?;
        let access = self.replay_nominal_access(node.source())?;
        let source = self
            .source(node.source())
            .ok_or(InheritanceQueryError::UnknownSource(node.source()))?;
        let inheritance = match (source.key.declaration_kind(), node.edges().modality()) {
            (
                SourceDeclarationKind::Class,
                NominalInheritanceModalityV1::Open | NominalInheritanceModalityV1::Abstract,
            ) => {
                let subclasses = PersistentAccessDomainV1::try_from_constraints(vec![
                    PersistentAccessConstraintV1::SubclassesOf(owner),
                ])
                .map_err(AccessDomainSemanticError::Encoding)?;
                let subclasses = self.validate_access_domain(&subclasses)?;
                access.lookup().intersect(&subclasses)?
            }
            (SourceDeclarationKind::Interface, _) => {
                self.validate_access_domain(access.lookup().domain())?
            }
            _ => self.validate_access_domain(&PersistentAccessDomainV1::empty())?,
        };
        Ok(CheckedNominalAccessDomainsV1 {
            owner,
            lookup: access.lookup,
            inheritance,
        })
    }

    pub fn validate_nominal_domains<'g>(
        &'g self,
        owner: PersistentExactTypeId,
        domains: &NominalAccessDomainsV1,
    ) -> Result<CheckedNominalAccessDomainsV1<'g, 'a>, AccessDomainSemanticError> {
        let expected = self.replay_nominal_domains(owner)?;
        if domains != &expected.to_record() {
            return Err(AccessDomainSemanticError::NominalDomains);
        }
        Ok(expected)
    }
}
