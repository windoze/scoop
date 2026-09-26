use super::*;
use scoop_wire::WirePath;

mod members;

/// Collects protected member references and dispatch schemas for each nominal.
pub(in crate::production::type_semantics) fn project(
    export: &ExportHir,
    nominals: &[ConcreteNominal<'_>],
) -> Result<CanonicalSourceInheritanceInventoriesV1, Error> {
    let path = WirePath::root();
    let mut records = Vec::new();

    scoop_wire::allocation::try_reserve(&mut records, nominals.len(), &path).map_err(resource)?;
    let protected = members::project(export)?;
    for nominal in nominals {
        let required = protected
            .get(&SourceNominalId::Concrete(nominal.owner))
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let mut member_refs = Vec::new();
        scoop_wire::allocation::try_reserve(&mut member_refs, required.len(), &path)
            .map_err(resource)?;
        member_refs.extend_from_slice(required);
        let members = CanonicalProtectedDeclarationRefsV1::try_new(member_refs)
            .map_err(|error| invalid(nominal, error))?;
        records.push(SourceInheritanceInventoryV1::new(
            nominal.exact,
            members,
            schemas::project(export, nominal)?,
        ));
    }
    CanonicalSourceInheritanceInventoriesV1::try_new(records).map_err(Error::SourceInventory)
}

fn invalid(nominal: &ConcreteNominal<'_>, error: impl std::fmt::Display) -> Error {
    Error::InvalidInheritance {
        exact: nominal.exact,
        reason: error.to_string(),
    }
}

fn resource(error: scoop_wire::WireError) -> Error {
    Error::SourceInventory(SourceInventoryError::Resource(error))
}
