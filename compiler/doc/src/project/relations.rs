use super::*;

impl Projection<'_> {
    pub(super) fn relations(
        &self,
        declaration: &hir::DocumentationDeclaration,
    ) -> Result<Vec<ApiRelation>> {
        declaration
            .relations
            .iter()
            .filter(|relation| {
                if relation.hidden {
                    return false;
                }
                let hir::DocumentationSymbol::Current(target) = relation.target else {
                    return true;
                };
                let mut current = Some(target);
                while let Some(target) = current {
                    if let Target::Declaration(source) = target
                        && self
                            .controls
                            .get(&source)
                            .is_some_and(|control| control.hidden)
                    {
                        return false;
                    }
                    current = self
                        .declarations
                        .get(&target)
                        .and_then(|entry| entry.parent);
                }
                true
            })
            .map(|relation| {
                Ok(ApiRelation {
                    kind: match relation.kind {
                        hir::DocumentationRelationKind::Supertype => RelationKind::Supertype,
                        hir::DocumentationRelationKind::Inherited => RelationKind::Inherited,
                        hir::DocumentationRelationKind::Overrides => RelationKind::Overrides,
                    },
                    target: self.symbol(relation.target)?,
                    signature: relation.signature.clone(),
                    origin: relation.origin.clone(),
                    public: match relation.visibility {
                        hir::DeclaredVisibility::Public => true,
                        hir::DeclaredVisibility::Protected => match declaration.target {
                            Target::Declaration(Source::Nominal(hir::NominalOwner::Class(id))) => {
                                self.externally_inheritable(
                                    self.export.nominal_identities[id].declaration_id(),
                                    &mut Default::default(),
                                )
                            }
                            _ => false,
                        },
                        _ => false,
                    },
                })
            })
            .collect()
    }
}
