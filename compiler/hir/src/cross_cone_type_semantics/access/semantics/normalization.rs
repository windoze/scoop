use scoop_wire::{BudgetMeter, WirePath};

use super::{AccessDomainSemanticError, Constraint, PersistentAccessDomainV1};
use crate::{CheckedNominalInheritanceGraphV1, InheritanceQueryError};

pub(super) fn normalize(
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    domain: &PersistentAccessDomainV1,
    meter: &mut BudgetMeter,
) -> Result<PersistentAccessDomainV1, AccessDomainSemanticError> {
    let path = WirePath::root();
    for constraint in domain.constraints() {
        meter
            .charge_work(1, &path)
            .map_err(AccessDomainSemanticError::Resource)?;
        match constraint {
            Constraint::LexicalOwner(owner) => {
                graph
                    .source(*owner)
                    .ok_or(InheritanceQueryError::UnknownSource(*owner))?;
            }
            Constraint::SubclassesOf(class) => graph.require_class(*class)?,
            Constraint::Cone(_) | Constraint::File(_) => {}
        }
    }
    for (index, left) in domain.constraints().iter().enumerate() {
        for right in &domain.constraints()[index + 1..] {
            meter
                .charge_work(1, &path)
                .map_err(AccessDomainSemanticError::Resource)?;
            if disjoint(graph, left, right)? {
                return Ok(PersistentAccessDomainV1::empty());
            }
        }
    }
    meter
        .charge_collection_slots(domain.constraints().len() as u64, &path)
        .map_err(AccessDomainSemanticError::Resource)?;
    Ok(domain.clone())
}

fn disjoint(
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    left: &Constraint,
    right: &Constraint,
) -> Result<bool, AccessDomainSemanticError> {
    Ok(match (left, right) {
        (Constraint::Cone(left), Constraint::Cone(right)) => left != right,
        (Constraint::File(left), Constraint::File(right)) => left != right,
        (Constraint::Cone(cone), Constraint::File(file))
        | (Constraint::File(file), Constraint::Cone(cone)) => *cone != file.cone(),
        (Constraint::LexicalOwner(owner), Constraint::Cone(cone))
        | (Constraint::Cone(cone), Constraint::LexicalOwner(owner)) => {
            graph
                .source(*owner)
                .ok_or(InheritanceQueryError::UnknownSource(*owner))?
                .key
                .origin()
                != *cone
        }
        (Constraint::LexicalOwner(owner), Constraint::File(file))
        | (Constraint::File(file), Constraint::LexicalOwner(owner)) => {
            graph
                .source(*owner)
                .ok_or(InheritanceQueryError::UnknownSource(*owner))?
                .access
                .definition_origin()
                .origin()
                .source()
                != file
        }
        (Constraint::LexicalOwner(left), Constraint::LexicalOwner(right)) => {
            !graph.lexically_contains(*left, *right)? && !graph.lexically_contains(*right, *left)?
        }
        // A lexical chain can contain two unrelated subclass owners. Neither
        // single inheritance nor absent private nested records proves this
        // intersection empty.
        (Constraint::SubclassesOf(_), _) | (_, Constraint::SubclassesOf(_)) => false,
    })
}
