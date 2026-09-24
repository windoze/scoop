use super::*;
use scoop_identity::PersistentConstructorId;
use scoop_wire::{BudgetMeter, WirePath};

mod members;
pub(in crate::production::type_semantics) use members::project as protected_members;

/// Projects declaration-side obligations before constructing any inheritance
/// candidate. No public/candidate table supplies the required constructor set.
pub(in crate::production::type_semantics) fn project(
    export: &ExportHir,
    nominals: &[ConcreteNominal<'_>],
    meter: &mut BudgetMeter,
) -> Result<CanonicalSourceInheritanceInventoriesV1, Error> {
    let path = WirePath::root();
    let mut records = Vec::new();
    meter
        .check_table_entries(nominals.len() as u64, &path)
        .map_err(resource)?;
    meter
        .try_reserve_collection_slots(&mut records, nominals.len(), &path)
        .map_err(resource)?;
    let protected = members::project(export, meter)?;
    for nominal in nominals {
        let constructors = constructors(export, nominal, meter)?;
        let required = protected
            .get(&SourceNominalId::Concrete(nominal.owner))
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let mut member_refs = Vec::new();
        meter
            .try_reserve_collection_slots(&mut member_refs, required.len(), &path)
            .map_err(resource)?;
        member_refs.extend_from_slice(required);
        let members = CanonicalProtectedDeclarationRefsV1::try_new(member_refs)
            .map_err(|error| invalid(nominal, error))?;
        records.push(
            SourceInheritanceInventoryV1::try_new(
                nominal.exact,
                constructors,
                members,
                schemas::project(export, nominal, meter)?,
                meter,
            )
            .map_err(Error::SourceInventory)?,
        );
    }
    CanonicalSourceInheritanceInventoriesV1::try_new(records, meter).map_err(Error::SourceInventory)
}

fn constructors(
    export: &ExportHir,
    nominal: &ConcreteNominal<'_>,
    meter: &mut BudgetMeter,
) -> Result<CanonicalPersistentIdsV1<PersistentConstructorId>, Error> {
    let owners = nominal.source.declaration().owners().owners();
    meter
        .charge_work(owners.len() as u64, &WirePath::root())
        .map_err(resource)?;
    if owners
        .iter()
        .any(|owner| matches!(owner, scoop_identity::DefinitionOwnerAtom::GenericType(_)))
    {
        return Ok(CanonicalPersistentIdsV1::empty());
    }
    let mut constructors = Vec::new();
    match nominal.local {
        NominalLocalId::Struct(id) => {
            for constructor in &export.structs[id].constructors {
                if visible(export.struct_constructors[*constructor].access.declared) {
                    constructors.push(export.constructor_identities[*constructor].id());
                }
            }
        }
        NominalLocalId::Class(id) => {
            for constructor in &export.classes[id].constructors {
                let declaration = &export.class_constructors[*constructor];
                match declaration.identity_kind {
                    ClassConstructorIdentityKind::Source => {
                        if visible(declaration.access.declared) {
                            let source = export.constructor_identities[*constructor]
                                .source_record()
                                .ok_or(Error::MissingConstructor(nominal.exact))?;
                            constructors.push(source.id());
                        }
                    }
                    ClassConstructorIdentityKind::ZeroArgumentAdapter { .. } => continue,
                }
            }
        }
        NominalLocalId::Enum(_) | NominalLocalId::Interface(_) | NominalLocalId::Object(_) => {
            return Ok(CanonicalPersistentIdsV1::empty());
        }
    }
    CanonicalPersistentIdsV1::try_new(constructors).map_err(|error| invalid(nominal, error))
}

fn visible(visibility: DeclaredVisibility) -> bool {
    matches!(
        visibility,
        DeclaredVisibility::Public | DeclaredVisibility::Protected
    )
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
