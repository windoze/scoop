use crate::{
    CanonicalCallableInterfacesV1, CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1,
    NestedSourceMemberRefV1, NominalSourceShapeV1, PublicDeclarationOwnerV1,
};
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{BudgetMeter, WireError, WirePath};
use std::collections::BTreeMap;

impl CanonicalNominalInterfacesV1 {
    pub fn declared_source_callables(
        &self,
        meter: &mut BudgetMeter,
    ) -> Result<
        BTreeMap<CallableTemplateOrigin, PublicDeclarationOwnerV1>,
        CallableDeclarationInventoryError,
    > {
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
                if let Some(previous) = required.insert(
                    declaration,
                    PublicDeclarationOwnerV1::Nominal(nominal.declaration()),
                ) {
                    return Err(CallableDeclarationInventoryError::DuplicateRelation {
                        declaration,
                        first: previous,
                        second: PublicDeclarationOwnerV1::Nominal(nominal.declaration()),
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
    pub(crate) fn required_declarations(
        nominals: &CanonicalNominalInterfacesV1,
        properties: &CanonicalPropertyInterfacesV1,
        meter: &mut BudgetMeter,
    ) -> Result<
        BTreeMap<CallableTemplateOrigin, PublicDeclarationOwnerV1>,
        CallableDeclarationInventoryError,
    > {
        let mut required = nominals.declared_source_callables(meter)?;
        let path = WirePath::root().field(3);
        for property in properties.all_declarations() {
            for accessor in
                std::iter::once(property.accessors().getter()).chain(property.accessors().setter())
            {
                meter.check_table_entries(required.len() as u64 + 1, &path)?;
                meter.charge_collection_slots(1, &path)?;
                meter.charge_work(u64::from(required.len().max(1).ilog2()) + 1, &path)?;
                let declaration = CallableTemplateOrigin::Accessor(accessor);
                if let Some(first) = required.insert(declaration, property.owner()) {
                    return Err(CallableDeclarationInventoryError::DuplicateRelation {
                        declaration,
                        first,
                        second: property.owner(),
                    });
                }
            }
        }
        Ok(required)
    }

    /// The shared nominal declaration relationships require exact source coverage.
    pub fn validate_declaration_inventory(
        &self,
        nominals: &CanonicalNominalInterfacesV1,
        properties: &CanonicalPropertyInterfacesV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), CallableDeclarationInventoryError> {
        self.validate_inventory(nominals, properties, true, meter)
    }

    /// Checks member ownership and accessor relationships. Top-level support
    /// reachability is proved separately from public roots and actual bodies.
    pub fn validate_member_declaration_inventory(
        &self,
        nominals: &CanonicalNominalInterfacesV1,
        properties: &CanonicalPropertyInterfacesV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), CallableDeclarationInventoryError> {
        self.validate_inventory(nominals, properties, false, meter)
    }

    fn validate_inventory(
        &self,
        nominals: &CanonicalNominalInterfacesV1,
        properties: &CanonicalPropertyInterfacesV1,
        exact_members: bool,
        meter: &mut BudgetMeter,
    ) -> Result<(), CallableDeclarationInventoryError> {
        let required = Self::required_declarations(nominals, properties, meter)?;
        let path = WirePath::root().field(3);
        for (declaration, owner) in &required {
            meter.charge_work(
                u64::from(self.declaration_count().max(1).ilog2()) + 1,
                &path,
            )?;
            let record = self
                .declaration(*declaration)
                .ok_or(CallableDeclarationInventoryError::Missing(*declaration))?;
            if record.owner() != *owner {
                return Err(CallableDeclarationInventoryError::Owner {
                    declaration: *declaration,
                    expected: *owner,
                    actual: record.owner(),
                });
            }
        }
        for record in self.support_records() {
            meter.charge_work(u64::from(required.len().max(1).ilog2()) + 1, &path)?;
            if !required.contains_key(&record.declaration())
                && (exact_members
                    || !matches!(
                        record.owner(),
                        PublicDeclarationOwnerV1::TopLevel | PublicDeclarationOwnerV1::Extension
                    )
                    || !matches!(
                        record.declaration(),
                        CallableTemplateOrigin::Function(_)
                            | CallableTemplateOrigin::GenericFunction(_)
                    ))
            {
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
        expected: PublicDeclarationOwnerV1,
        actual: PublicDeclarationOwnerV1,
    },
    DuplicateRelation {
        declaration: CallableTemplateOrigin,
        first: PublicDeclarationOwnerV1,
        second: PublicDeclarationOwnerV1,
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
