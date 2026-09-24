//! Reuse the ordinary access graph for complete shared source domains.

use super::*;
use crate::{SourceAccessConstraintV1 as Source, SourceAccessDomainV1};

impl<'a> CheckedNominalInheritanceGraphV1<'a> {
    pub(in crate::cross_cone_type_semantics) fn replay_source_domain<'g>(
        &'g self,
        source: &SourceAccessDomainV1,
        meter: &mut BudgetMeter,
    ) -> Result<CheckedPersistentAccessDomainV1<'g, 'a>, AccessDomainSemanticError> {
        let path = WirePath::root();
        let mut constraints = Vec::new();
        meter
            .try_reserve_collection_slots(&mut constraints, source.constraints().len(), &path)
            .map_err(AccessDomainSemanticError::Resource)?;
        for constraint in source.constraints() {
            meter
                .charge_work(1, &path)
                .map_err(AccessDomainSemanticError::Resource)?;
            constraints.push(match constraint {
                Source::Cone(cone) => Constraint::Cone(*cone),
                Source::File(source) => {
                    meter
                        .check_semantic_leaf(source.logical_path().as_str().len() as u64, &path)
                        .map_err(AccessDomainSemanticError::Resource)?;
                    meter
                        .charge_owned_bytes(source.logical_path().as_str().len() as u64, &path)
                        .map_err(AccessDomainSemanticError::Resource)?;
                    Constraint::File(source.clone())
                }
                Source::LexicalOwner(owner) => Constraint::LexicalOwner(*owner),
                Source::SubclassesOf(owner) => Constraint::SubclassesOf(self.source_exact(*owner)?),
            });
        }
        let count = constraints.len() as u64;
        let mut encoded_bytes = 0u64;
        for constraint in &constraints {
            encoded_bytes = encoded_bytes.saturating_add(
                scoop_wire::encoded_length(constraint).map_err(|error| {
                    AccessDomainSemanticError::Encoding(PersistentAccessDomainError::Encoding(
                        error,
                    ))
                })?,
            );
        }
        meter
            .charge_owned_bytes(encoded_bytes.saturating_mul(2), &path)
            .map_err(AccessDomainSemanticError::Resource)?;
        meter
            .charge_work(
                encoded_bytes.saturating_mul(2 + u64::from(count.max(1).ilog2())),
                &path,
            )
            .map_err(AccessDomainSemanticError::Resource)?;
        meter
            .charge_collection_slots(count.saturating_mul(3), &path)
            .map_err(AccessDomainSemanticError::Resource)?;
        meter
            .charge_work(
                count.saturating_mul(1 + u64::from(count.max(1).ilog2())),
                &path,
            )
            .map_err(AccessDomainSemanticError::Resource)?;
        let domain = if source.is_empty() {
            PersistentAccessDomainV1::empty()
        } else {
            PersistentAccessDomainV1::try_from_constraints(constraints)
                .map_err(AccessDomainSemanticError::Encoding)?
        };
        let domain = normalization::normalize(self, &domain, meter)?;
        Ok(CheckedPersistentAccessDomainV1 {
            graph: self,
            domain,
        })
    }
}
