//! The shared wire snapshot does not require concrete exact identities.

use super::*;

impl SourceAccessDomainV1 {
    pub fn from_export_hir(
        export: &ExportHir,
        source: &AccessDomain,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let path = WirePath::root();
        meter.check_semantic_depth(2, &path)?;
        meter.charge_work(1, &path)?;
        if source.is_empty() {
            return Ok(Self::empty());
        }
        let mut constraints = Vec::new();
        meter.try_reserve_collection_slots(&mut constraints, source.constraints().len(), &path)?;
        for constraint in source.constraints() {
            meter.charge_work(1, &path)?;
            constraints.push(match constraint {
                AccessConstraint::Cone(id) => SourceAccessConstraintV1::Cone(*id),
                AccessConstraint::File(source) => {
                    let bytes = source.logical_path().as_str().len() as u64;
                    meter.check_semantic_leaf(bytes, &path)?;
                    meter.charge_owned_bytes(bytes, &path)?;
                    meter.charge_work(bytes, &path)?;
                    SourceAccessConstraintV1::File(source.clone())
                }
                AccessConstraint::LexicalOwner(owner) => {
                    SourceAccessConstraintV1::LexicalOwner(domains::nominal(export, *owner)?)
                }
                AccessConstraint::SubclassesOf(class) => SourceAccessConstraintV1::SubclassesOf(
                    domains::nominal(export, VisibilityOwner::Class(*class))?,
                ),
            });
        }
        Self::from_constraints(constraints, meter, &path).map_err(Error::Resource)
    }
}

impl ExportDefaultAccessWitnessV1 {
    /// Captures the original provider witness; inherited publication checks are
    /// performed separately against the publishing declaration.
    pub fn from_export_hir(
        export: &ExportHir,
        witness: &ExportDefaultAccessWitness,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let owner = super::super::entities::DefaultEntityProjector::new(export, None, meter)
            .parameter_owner(witness.owner)
            .map_err(Error::Owner)?;
        let direct =
            SourceAccessDomainV1::from_export_hir(export, &witness.call_domain.direct.0, meter)?;
        let slot = witness
            .call_domain
            .slot
            .as_ref()
            .map(|slot| SourceAccessDomainV1::from_export_hir(export, &slot.0, meter))
            .transpose()?;
        let target = SourceAccessDomainV1::from_export_hir(export, &witness.target_domain, meter)?;
        Self::try_new(owner, direct, slot, target).map_err(Error::SharedBuild)
    }
}
