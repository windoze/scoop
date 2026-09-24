//! Borrowed semantic dependencies from the shared nominal and callable tables.

use super::*;
use crate::{
    CallableDeclarationRecordV1, DeclaredVisibilityV1, EnumSourceFieldV1, NominalSourceFieldV1,
    NominalSourceShapeV1, PublicDeclarationOwnerV1,
};
use scoop_identity::{CallableTemplateOrigin, SignatureTypeKey};

/// A dependency position in the original declaration, not a transport record
/// or a machine-use permit. Repeated types retain their distinct source sites.
#[derive(Clone, Copy, Debug)]
pub enum NominalMaterializationRequirementV1<'a> {
    Field {
        owner: PersistentTypeId,
        field: &'a NominalSourceFieldV1,
    },
    EnumVariantField {
        owner: PersistentTypeId,
        field: &'a EnumSourceFieldV1,
    },
    Inheritance {
        owner: PersistentTypeId,
        parent: &'a SignatureTypeKey,
    },
    Constructor {
        owner: PersistentTypeId,
        callable: &'a CallableDeclarationRecordV1,
    },
    Slot {
        owner: PersistentTypeId,
        callable: &'a CallableDeclarationRecordV1,
    },
}

impl NominalMaterializationRequirementV1<'_> {
    pub const fn owner(self) -> PersistentTypeId {
        match self {
            Self::Field { owner, .. }
            | Self::EnumVariantField { owner, .. }
            | Self::Inheritance { owner, .. }
            | Self::Constructor { owner, .. }
            | Self::Slot { owner, .. } => owner,
        }
    }
}

impl CanonicalNominalInterfacesV1 {
    /// Visits all parameter-free nominal machine requirements, including
    /// private storage. The callback and traversal share one cumulative meter.
    pub fn visit_materialization_requirements<'a, E>(
        &'a self,
        callables: &'a CanonicalCallableInterfacesV1,
        meter: &mut BudgetMeter,
        mut visit: impl FnMut(
            NominalMaterializationRequirementV1<'a>,
            &mut BudgetMeter,
        ) -> Result<(), E>,
    ) -> Result<(), E>
    where
        E: From<WireError> + From<NominalMaterializationClosureError>,
    {
        use NominalMaterializationRequirementV1 as Requirement;
        let path = WirePath::root();
        meter.check_table_entries(self.declaration_count() as u64, &path)?;
        meter.check_table_entries(callables.declaration_count() as u64, &path)?;
        let mut dispatched = BTreeSet::new();
        for nominal in self.all_records() {
            meter.charge_work(1, &path)?;
            let SourceNominalId::Concrete(owner) = nominal.declaration() else {
                continue;
            };
            let selections = nominal
                .declaration_details()
                .dispatch_selections()
                .records();
            meter.check_table_entries(selections.len() as u64, &path)?;
            for selection in selections {
                meter.charge_work(1 + u64::from(dispatched.len().max(1).ilog2()), &path)?;
                if let Some(target) = selection.callable_target()
                    && !dispatched.contains(&target)
                {
                    meter.check_table_entries(dispatched.len() as u64 + 1, &path)?;
                    meter.charge_collection_slots(1, &path)?;
                    dispatched.insert(target);
                }
            }
            let fields = nominal.source_shape().declared_fields();
            meter.check_table_entries(fields.len() as u64, &path)?;
            for field in fields {
                meter.charge_work(1, &path)?;
                visit(Requirement::Field { owner, field }, meter)?;
            }
            if let NominalSourceShapeV1::Enum(shape) = nominal.source_shape() {
                meter.check_table_entries(shape.variants().len() as u64, &path)?;
                for variant in shape.variants() {
                    meter.charge_work(1, &path)?;
                    meter.check_table_entries(variant.fields().len() as u64, &path)?;
                    for field in variant.fields() {
                        meter.charge_work(1, &path)?;
                        visit(Requirement::EnumVariantField { owner, field }, meter)?;
                    }
                }
            }
            meter.check_table_entries(nominal.exact_supertypes().values().len() as u64, &path)?;
            for parent in nominal.exact_supertypes().values() {
                meter.charge_work(1, &path)?;
                visit(Requirement::Inheritance { owner, parent }, meter)?;
            }
        }
        for callable in callables.all_declarations() {
            meter.charge_work(1, &path)?;
            let PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(owner)) =
                callable.owner()
            else {
                continue;
            };
            let constructor = matches!(
                callable.declaration(),
                CallableTemplateOrigin::Constructor(_)
                    | CallableTemplateOrigin::VariantConstructor(_)
            ) && matches!(
                callable.declared_visibility(),
                DeclaredVisibilityV1::Public | DeclaredVisibilityV1::Protected
            );
            meter.charge_work(1 + u64::from(dispatched.len().max(1).ilog2()), &path)?;
            if !constructor
                && callable.slot_relations().is_empty()
                && !dispatched.contains(&callable.declaration())
            {
                continue;
            }
            meter.charge_work(
                1 + u64::from(self.declaration_count().max(1).ilog2()),
                &path,
            )?;
            if self.declaration(SourceNominalId::Concrete(owner)).is_none() {
                return Err(NominalMaterializationClosureError::MissingNominal(owner).into());
            }
            let requirement = if constructor {
                Requirement::Constructor { owner, callable }
            } else {
                Requirement::Slot { owner, callable }
            };
            visit(requirement, meter)?;
        }
        Ok(())
    }
}
