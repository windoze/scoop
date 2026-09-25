use super::{
    AccessDomainSemanticError, CheckedPersistentAccessDomainV1, Constraint, normalization,
};
use crate::{CheckedNominalInheritanceGraphV1, InheritanceQueryError, SourceNominalId};

impl<'g, 'a> CheckedPersistentAccessDomainV1<'g, 'a> {
    /// Whether every call point in `required` is admitted by this target.
    pub fn covers(&self, required: &Self) -> Result<bool, AccessDomainSemanticError> {
        if !std::ptr::eq(self.graph, required.graph) {
            return Err(AccessDomainSemanticError::DifferentGraph);
        }
        if required.domain.is_empty() {
            return Ok(true);
        }
        if self.domain.is_empty() {
            return Ok(false);
        }
        for wider in self.domain.constraints() {
            let mut implied = false;
            for narrower in required.domain.constraints() {
                if implies(self.graph, narrower, wider)? {
                    implied = true;
                    break;
                }
            }
            if !implied {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn intersect(&self, other: &Self) -> Result<Self, AccessDomainSemanticError> {
        if !std::ptr::eq(self.graph, other.graph) {
            return Err(AccessDomainSemanticError::DifferentGraph);
        }

        let domain = self
            .domain
            .intersect(&other.domain)
            .map_err(AccessDomainSemanticError::Encoding)?;
        let domain = normalization::normalize(self.graph, &domain)?;
        Ok(Self {
            graph: self.graph,
            domain,
        })
    }

    pub(super) fn allows_scope(
        &self,
        scope: SourceNominalId,
    ) -> Result<bool, AccessDomainSemanticError> {
        if self.domain.is_empty() {
            return Ok(false);
        }
        let source = self
            .graph
            .source(scope)
            .ok_or(InheritanceQueryError::UnknownSource(scope))?;
        for constraint in self.domain.constraints() {
            let allowed = match constraint {
                Constraint::Cone(cone) => source.key.origin() == *cone,
                Constraint::File(file) => {
                    source.access.definition_origin().origin().source() == file
                }
                Constraint::LexicalOwner(owner) => self.graph.lexically_contains(*owner, scope)?,
                Constraint::SubclassesOf(base) => {
                    let mut allowed = false;
                    for class in self.graph.scope_classes(scope)? {
                        if self.graph.is_subclass(class, *base)? {
                            allowed = true;
                            break;
                        }
                    }
                    allowed
                }
            };
            if !allowed {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn implies(
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    narrow: &Constraint,
    wide: &Constraint,
) -> Result<bool, AccessDomainSemanticError> {
    if narrow == wide {
        return Ok(true);
    }
    Ok(match (narrow, wide) {
        (Constraint::File(file), Constraint::Cone(cone)) => file.cone() == *cone,
        (Constraint::LexicalOwner(owner), Constraint::Cone(cone)) => {
            graph
                .source(*owner)
                .ok_or(InheritanceQueryError::UnknownSource(*owner))?
                .key
                .origin()
                == *cone
        }
        (Constraint::LexicalOwner(owner), Constraint::File(file)) => {
            graph
                .source(*owner)
                .ok_or(InheritanceQueryError::UnknownSource(*owner))?
                .access
                .definition_origin()
                .origin()
                .source()
                == file
        }
        (Constraint::LexicalOwner(inner), Constraint::LexicalOwner(outer)) => {
            graph.lexically_contains(*outer, *inner)?
        }
        (Constraint::SubclassesOf(derived), Constraint::SubclassesOf(base)) => {
            graph.is_subclass(*derived, *base)?
        }
        (Constraint::LexicalOwner(owner), Constraint::SubclassesOf(base)) => {
            let mut implied = false;
            for class in graph.scope_classes(*owner)? {
                if graph.is_subclass(class, *base)? {
                    implied = true;
                    break;
                }
            }
            implied
        }
        _ => false,
    })
}
