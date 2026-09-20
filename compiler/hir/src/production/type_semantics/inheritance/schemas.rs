use std::collections::BTreeSet;

use scoop_identity::PersistentExactTypeId;

use super::{ConcreteNominal, Error, NominalLocalId, exact};
use crate::*;

pub(super) fn project(
    export: &ExportHir,
    nominal: &ConcreteNominal<'_>,
) -> Result<CanonicalInheritanceSlotSchemasV1, Error> {
    let mut schemas = Vec::new();
    if matches!(
        nominal.local,
        NominalLocalId::Class(_) | NominalLocalId::Object(_)
    ) {
        schemas.push(
            InheritanceSlotSchemaV1::try_new(InheritanceSlotSchemaRoleV1::ClassVtable, Vec::new())
                .map_err(|error| invalid(nominal, error))?,
        );
    }
    if matches!(nominal.local, NominalLocalId::Interface(_)) {
        schemas.push(
            InheritanceSlotSchemaV1::try_new(
                InheritanceSlotSchemaRoleV1::Interface {
                    interface_exact: nominal.exact,
                },
                Vec::new(),
            )
            .map_err(|error| invalid(nominal, error))?,
        );
    }
    for interface_exact in implemented_interfaces(export, nominal)? {
        schemas.push(
            InheritanceSlotSchemaV1::try_new(
                InheritanceSlotSchemaRoleV1::Interface { interface_exact },
                Vec::new(),
            )
            .map_err(|error| invalid(nominal, error))?,
        );
    }
    CanonicalInheritanceSlotSchemasV1::try_new(schemas).map_err(|error| invalid(nominal, error))
}

fn invalid(nominal: &ConcreteNominal<'_>, error: impl std::fmt::Display) -> Error {
    Error::InvalidInheritance {
        exact: nominal.exact,
        reason: error.to_string(),
    }
}

fn implemented_interfaces(
    export: &ExportHir,
    nominal: &ConcreteNominal<'_>,
) -> Result<Vec<PersistentExactTypeId>, Error> {
    let mut interfaces = BTreeSet::new();
    let mut active_classes = BTreeSet::new();
    match nominal.local {
        NominalLocalId::Struct(id) => {
            collect_interface_types(export, &export.structs[id].interfaces, &mut interfaces)?;
        }
        NominalLocalId::Enum(id) => {
            collect_interface_types(export, &export.enums[id].interfaces, &mut interfaces)?;
        }
        NominalLocalId::Class(id) => {
            collect_class_interfaces(export, id, &mut active_classes, &mut interfaces)?;
        }
        NominalLocalId::Interface(id) => {
            for parent in &export.interfaces[id].parents {
                collect_interface_type(
                    export,
                    export.interface_applications[*parent].canonical_type,
                    &mut interfaces,
                )?;
            }
        }
        NominalLocalId::Object(id) => collect_class_interfaces(
            export,
            export.objects[id].backing_class,
            &mut active_classes,
            &mut interfaces,
        )?,
    }
    Ok(interfaces.into_iter().collect())
}

fn collect_class_interfaces(
    export: &ExportHir,
    class: ClassId,
    active: &mut BTreeSet<ClassId>,
    interfaces: &mut BTreeSet<PersistentExactTypeId>,
) -> Result<(), Error> {
    if !active.insert(class) {
        return Ok(());
    }
    let declaration = &export.classes[class];
    collect_interface_types(export, &declaration.interfaces, interfaces)?;
    if let Some(base) = declaration.base_class {
        let Type::Class(application) = export.types[base] else {
            return Err(Error::InvalidInheritance {
                exact: exact(export, base)?,
                reason: "class base does not resolve to a class application".into(),
            });
        };
        if !export.class_applications[application].arguments.is_empty() {
            return Err(Error::GenericOdrRequired(exact(export, base)?));
        }
        collect_class_interfaces(
            export,
            export.class_applications[application].template,
            active,
            interfaces,
        )?;
    }
    active.remove(&class);
    Ok(())
}

fn collect_interface_types(
    export: &ExportHir,
    types: &[TypeId],
    interfaces: &mut BTreeSet<PersistentExactTypeId>,
) -> Result<(), Error> {
    for ty in types {
        collect_interface_type(export, *ty, interfaces)?;
    }
    Ok(())
}

fn collect_interface_type(
    export: &ExportHir,
    ty: TypeId,
    interfaces: &mut BTreeSet<PersistentExactTypeId>,
) -> Result<(), Error> {
    let Type::Interface(application) = export.types[ty] else {
        return Err(Error::InvalidInheritance {
            exact: exact(export, ty)?,
            reason: "interface edge does not resolve to an interface application".into(),
        });
    };
    let exact = exact(export, ty)?;
    if !interfaces.insert(exact) {
        return Ok(());
    }
    let declaration = &export.interfaces[export.interface_applications[application].template];
    for parent in &declaration.parents {
        collect_interface_type(
            export,
            export.interface_applications[*parent].canonical_type,
            interfaces,
        )?;
    }
    Ok(())
}
