use super::source_errors::{invalid, resource};
use super::*;
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::WirePath;
use std::collections::BTreeSet;

impl CanonicalInheritanceSourceParameterProtocolsV1 {
    /// Projects a closed Export HIR's source protocols for an independent
    /// inheritance inventory. Inventory ownership and artifact binding remain
    /// obligations of the complete source-authority transaction.
    pub fn from_export_hir(
        output: &ExportHirOutput,
        inventory: &CanonicalSourceInheritanceInventoriesV1,
    ) -> Result<Self, Error> {
        project(output.module(), inventory)
    }
}

pub(in crate::production::type_semantics) fn project(
    export: &ExportHir,
    inventory: &CanonicalSourceInheritanceInventoriesV1,
) -> Result<CanonicalInheritanceSourceParameterProtocolsV1, Error> {
    let mut required = BTreeSet::new();
    for owner in inventory.records() {
        for constructor in owner.constructors().values() {
            insert(
                &mut required,
                CallableTemplateOrigin::Constructor(*constructor),
            )?;
        }
        for member in owner.protected_members().values() {
            if let ProtectedDeclarationRefV1::Callable(callable) = member {
                if matches!(
                    callable.declaration(),
                    CallableTemplateOrigin::Function(_)
                        | CallableTemplateOrigin::GenericFunction(_)
                ) {
                    insert(&mut required, callable.declaration())?;
                }
            }
        }
    }
    let sources = super::super::nominal_parameters::project(export, required)?;
    let path = WirePath::root();
    let mut records = Vec::new();
    scoop_wire::allocation::try_reserve(&mut records, sources.len(), &path).map_err(resource)?;
    for source in sources {
        records.push(
            InheritanceSourceParameterProtocolV1::try_from(source)
                .map_err(Error::SourceInventory)?,
        );
    }
    CanonicalInheritanceSourceParameterProtocolsV1::try_new(records).map_err(Error::SourceInventory)
}

fn insert(
    required: &mut BTreeSet<CallableTemplateOrigin>,
    declaration: CallableTemplateOrigin,
) -> Result<(), Error> {
    if !required.insert(declaration) {
        return Err(invalid(
            "parameter protocol belongs to multiple inheritance owners",
        ));
    }
    Ok(())
}
