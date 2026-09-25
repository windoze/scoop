//! Reuse the ordinary access graph for complete shared source domains.

use super::*;
use crate::{SourceAccessConstraintV1 as Source, SourceAccessDomainV1};

impl<'a> CheckedNominalInheritanceGraphV1<'a> {
    pub(in crate::cross_cone_type_semantics) fn replay_source_domain<'g>(
        &'g self,
        source: &SourceAccessDomainV1,
    ) -> Result<CheckedPersistentAccessDomainV1<'g, 'a>, AccessDomainSemanticError> {
        let path = WirePath::root();
        let mut constraints = Vec::new();
        scoop_wire::allocation::try_reserve(&mut constraints, source.constraints().len(), &path)
            .map_err(AccessDomainSemanticError::Resource)?;
        for constraint in source.constraints() {
            constraints.push(match constraint {
                Source::Cone(cone) => Constraint::Cone(*cone),
                Source::File(source) => Constraint::File(source.clone()),
                Source::LexicalOwner(owner) => Constraint::LexicalOwner(*owner),
                Source::SubclassesOf(owner) => Constraint::SubclassesOf(self.source_exact(*owner)?),
            });
        }

        let domain = if source.is_empty() {
            PersistentAccessDomainV1::empty()
        } else {
            PersistentAccessDomainV1::try_from_constraints(constraints)
                .map_err(AccessDomainSemanticError::Encoding)?
        };
        let domain = normalization::normalize(self, &domain)?;
        Ok(CheckedPersistentAccessDomainV1 {
            graph: self,
            domain,
        })
    }
}
