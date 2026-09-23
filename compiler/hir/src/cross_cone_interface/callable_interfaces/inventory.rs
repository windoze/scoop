use crate::{
    CanonicalCallableInterfacesV1, CanonicalNominalInterfacesV1, NestedSourceMemberRefV1,
    NominalSourceShapeV1, PublicDeclarationOwnerV1, SourceNominalId,
};
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{BudgetMeter, WireError, WirePath};
use std::collections::BTreeMap;

impl CanonicalNominalInterfacesV1 {
    pub fn declared_source_callables(
        &self,
        meter: &mut BudgetMeter,
    ) -> Result<BTreeMap<CallableTemplateOrigin, SourceNominalId>, CallableDeclarationInventoryError>
    {
        let mut required = BTreeMap::new();
        let path = WirePath::root().field(3);
        for nominal in self.all_records() {
            meter.charge_work(
                1 + nominal.declaration_details().members().values().len() as u64,
                &path,
            )?;
            let mut add = |declaration| -> Result<(), CallableDeclarationInventoryError> {
                meter.charge_collection_slots(1, &path)?;
                meter.charge_work(u64::from(required.len().max(1).ilog2()) + 1, &path)?;
                if let Some(previous) = required.insert(declaration, nominal.declaration()) {
                    return Err(CallableDeclarationInventoryError::DuplicateRelation {
                        declaration,
                        first: previous,
                        second: nominal.declaration(),
                    });
                }
                Ok(())
            };
            for constructor in nominal.declaration_details().constructors().values() {
                add(CallableTemplateOrigin::Constructor(*constructor))?;
            }
            for member in nominal.declaration_details().members().values() {
                match member {
                    NestedSourceMemberRefV1::Function(id) => {
                        add(CallableTemplateOrigin::Function(*id))?
                    }
                    NestedSourceMemberRefV1::GenericFunction(id) => {
                        add(CallableTemplateOrigin::GenericFunction(*id))?
                    }
                    NestedSourceMemberRefV1::Property(_) => continue,
                }
            }
            if let NominalSourceShapeV1::Enum(shape) = nominal.source_shape() {
                for variant in shape.variants() {
                    add(CallableTemplateOrigin::VariantConstructor(
                        variant.variant(),
                    ))?;
                }
            }
        }
        Ok(required)
    }
}

impl CanonicalCallableInterfacesV1 {
    /// The shared nominal declaration relationships require exact source coverage.
    pub fn validate_declaration_inventory(
        &self,
        nominals: &CanonicalNominalInterfacesV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), CallableDeclarationInventoryError> {
        let required = nominals.declared_source_callables(meter)?;
        let path = WirePath::root().field(3);
        for (declaration, owner) in &required {
            meter.charge_work(
                u64::from(self.declaration_count().max(1).ilog2()) + 1,
                &path,
            )?;
            let record = self
                .declaration(*declaration)
                .ok_or(CallableDeclarationInventoryError::Missing(*declaration))?;
            if record.owner() != PublicDeclarationOwnerV1::Nominal(*owner) {
                return Err(CallableDeclarationInventoryError::Owner {
                    declaration: *declaration,
                    expected: *owner,
                    actual: record.owner(),
                });
            }
        }
        for record in self.support_records() {
            meter.charge_work(u64::from(required.len().max(1).ilog2()) + 1, &path)?;
            if !required.contains_key(&record.declaration()) {
                return Err(CallableDeclarationInventoryError::UnexpectedSupport(
                    record.declaration(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum CallableDeclarationInventoryError {
    Missing(CallableTemplateOrigin),
    UnexpectedSupport(CallableTemplateOrigin),
    Owner {
        declaration: CallableTemplateOrigin,
        expected: SourceNominalId,
        actual: PublicDeclarationOwnerV1,
    },
    DuplicateRelation {
        declaration: CallableTemplateOrigin,
        first: SourceNominalId,
        second: SourceNominalId,
    },
    Resource(WireError),
}
impl From<WireError> for CallableDeclarationInventoryError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for CallableDeclarationInventoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid shared callable declaration inventory: {self:?}")
    }
}
impl std::error::Error for CallableDeclarationInventoryError {}
