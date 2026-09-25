use super::*;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

pub(in crate::production::type_semantics) fn project(
    export: &ExportHir,
    nominals: &[ConcreteNominal<'_>],
    inventory: &CanonicalSourceInheritanceInventoriesV1,
) -> Result<CanonicalInheritanceSourceConstructorsV1, Error> {
    let mut required = BTreeMap::new();
    for nominal in nominals {
        let source = inventory
            .get(nominal.exact)
            .ok_or(Error::MissingConstructor(nominal.exact))?;
        for id in source.constructors().values() {
            if required.insert(*id, nominal).is_some() {
                return Err(invalid("constructor belongs to multiple source owners"));
            }
        }
    }

    let ids: BTreeSet<_> = required.keys().copied().collect();
    let records = super::super::nominal_constructors::project(export, ids)?;
    for record in &records {
        let nominal = required
            .get(&record.declaration())
            .ok_or_else(|| invalid("constructor is absent from inheritance inventory"))?;
        if record.payload().owner() != SourceNominalId::Concrete(nominal.owner)
            || !matches!(
                record.declaration_access().declared_visibility(),
                DeclaredVisibilityV1::Public | DeclaredVisibilityV1::Protected
            )
        {
            return Err(Error::MissingConstructor(nominal.exact));
        }
    }
    CanonicalInheritanceSourceConstructorsV1::try_new(records).map_err(Error::SourceInventory)
}

fn invalid(reason: impl ToString) -> Error {
    Error::InvalidSourceDeclaration(reason.to_string())
}
