//! The shared wire snapshot does not require concrete exact identities.

use super::*;

impl SourceAccessDomainV1 {
    pub fn from_export_hir(export: &ExportHir, source: &AccessDomain) -> Result<Self, Error> {
        let path = WirePath::root();

        if source.is_empty() {
            return Ok(Self::empty());
        }
        let mut constraints = Vec::new();
        scoop_wire::allocation::try_reserve(&mut constraints, source.constraints().len(), &path)?;
        for constraint in source.constraints() {
            constraints.push(match constraint {
                AccessConstraint::Cone(id) => SourceAccessConstraintV1::Cone(*id),
                AccessConstraint::File(source) => SourceAccessConstraintV1::File(source.clone()),
                AccessConstraint::LexicalOwner(owner) => {
                    SourceAccessConstraintV1::LexicalOwner(domains::nominal(export, *owner)?)
                }
                AccessConstraint::SubclassesOf(class) => SourceAccessConstraintV1::SubclassesOf(
                    domains::nominal(export, VisibilityOwner::Class(*class))?,
                ),
            });
        }
        Self::from_constraints(constraints).map_err(Error::Resource)
    }
}

impl ExportDefaultAccessWitnessV1 {
    /// Captures the original provider witness; inherited publication checks are
    /// performed separately against the publishing declaration.
    pub fn from_export_hir(
        export: &ExportHir,
        witness: &ExportDefaultAccessWitness,
    ) -> Result<Self, Error> {
        let owner = super::super::entities::DefaultEntityProjector::new(export, None)
            .parameter_owner(witness.owner)
            .map_err(Error::Owner)?;
        let direct = SourceAccessDomainV1::from_export_hir(export, &witness.call_domain.direct.0)?;
        let slot = witness
            .call_domain
            .slot
            .as_ref()
            .map(|slot| SourceAccessDomainV1::from_export_hir(export, &slot.0))
            .transpose()?;
        let target = SourceAccessDomainV1::from_export_hir(export, &witness.target_domain)?;
        Self::try_new(owner, direct, slot, target).map_err(Error::SharedBuild)
    }
}
