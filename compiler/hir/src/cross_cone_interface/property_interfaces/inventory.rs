use crate::{
    CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1, NestedSourceMemberRefV1,
    PropertyDeclarationId, PublicDeclarationOwnerV1, SourceNominalId,
};
use scoop_identity::PropertyOwner;
use scoop_wire::WireError;
use std::collections::BTreeMap;

impl CanonicalNominalInterfacesV1 {
    pub fn declared_source_properties(
        &self,
    ) -> Result<BTreeMap<PropertyDeclarationId, SourceNominalId>, PropertyDeclarationInventoryError>
    {
        let mut required = BTreeMap::new();

        for nominal in self.all_records() {
            for member in nominal.declaration_details().members().values() {
                let NestedSourceMemberRefV1::Property(id) = member else {
                    continue;
                };
                let declaration = PropertyOwner::Property(*id);

                if let Some(first) = required.insert(declaration, nominal.declaration()) {
                    return Err(PropertyDeclarationInventoryError::DuplicateRelation {
                        declaration,
                        first,
                        second: nominal.declaration(),
                    });
                }
            }
        }
        Ok(required)
    }
}

impl CanonicalPropertyInterfacesV1 {
    pub fn validate_declaration_inventory(
        &self,
        nominals: &CanonicalNominalInterfacesV1,
    ) -> Result<(), PropertyDeclarationInventoryError> {
        self.validate_inventory(nominals, true)
    }

    /// Checks member ownership and accessor relationships. Top-level support
    /// reachability is proved separately from public roots and actual bodies.
    pub fn validate_member_declaration_inventory(
        &self,
        nominals: &CanonicalNominalInterfacesV1,
    ) -> Result<(), PropertyDeclarationInventoryError> {
        self.validate_inventory(nominals, false)
    }

    fn validate_inventory(
        &self,
        nominals: &CanonicalNominalInterfacesV1,
        exact_members: bool,
    ) -> Result<(), PropertyDeclarationInventoryError> {
        let required = nominals.declared_source_properties()?;

        for (declaration, owner) in &required {
            let record = self
                .declaration(*declaration)
                .ok_or(PropertyDeclarationInventoryError::Missing(*declaration))?;
            if record.owner() != PublicDeclarationOwnerV1::Nominal(*owner) {
                return Err(PropertyDeclarationInventoryError::Owner {
                    declaration: *declaration,
                    expected: *owner,
                    actual: record.owner(),
                });
            }
        }
        for record in self.support_records() {
            if !required.contains_key(&record.declaration())
                && (exact_members
                    || !matches!(
                        record.owner(),
                        PublicDeclarationOwnerV1::TopLevel | PublicDeclarationOwnerV1::Extension
                    ))
            {
                return Err(PropertyDeclarationInventoryError::UnexpectedSupport(
                    record.declaration(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PropertyDeclarationInventoryError {
    Missing(PropertyDeclarationId),
    UnexpectedSupport(PropertyDeclarationId),
    Owner {
        declaration: PropertyDeclarationId,
        expected: SourceNominalId,
        actual: PublicDeclarationOwnerV1,
    },
    DuplicateRelation {
        declaration: PropertyDeclarationId,
        first: SourceNominalId,
        second: SourceNominalId,
    },
    Resource(WireError),
}
impl From<WireError> for PropertyDeclarationInventoryError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for PropertyDeclarationInventoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid shared property declaration inventory: {self:?}")
    }
}
impl std::error::Error for PropertyDeclarationInventoryError {}
