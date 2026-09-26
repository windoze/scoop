use super::*;
use scoop_identity::PersistentExactTypeId;
use std::collections::BTreeSet;

pub(in crate::production::type_semantics::nominals) fn fact_requirements(
    export: &ExportHir,
    nominals: &[ConcreteNominal<'_>],
) -> Result<BTreeSet<PersistentExactTypeId>, Error> {
    let mut exacts = BTreeSet::new();
    for nominal in nominals {
        visit_required_types(export, nominal.local, |ty| {
            let exact = export
                .type_identities
                .get(ty)
                .and_then(HirTypeIdentity::exact)
                .ok_or(Error::MissingExactIdentity {
                    context: "representation dependency",
                })?;
            exacts.insert(exact.id());
            Ok(())
        })?;
    }
    Ok(exacts)
}

/// Required storage, inheritance and constructor types share one source walk.
/// Visiting avoids allocating an unchecked temporary dependency collection.
pub(in crate::production::type_semantics::nominals) fn visit_required_types(
    export: &ExportHir,
    nominal: NominalLocalId,
    mut visit: impl FnMut(TypeId) -> Result<(), Error>,
) -> Result<(), Error> {
    match nominal {
        NominalLocalId::Struct(id) => {
            let declaration = &export.structs[id];
            for field in declaration.semantic_fields() {
                visit(field.ty)?;
            }
            for interface in &declaration.interfaces {
                visit(*interface)?;
            }
            for id in &declaration.constructors {
                let constructor = &export.struct_constructors[*id];
                if exported(constructor.access.declared) {
                    for parameter in &constructor.parameters {
                        visit(parameter.ty)?;
                    }
                }
            }
        }
        NominalLocalId::Enum(id) => {
            let declaration = &export.enums[id];
            for variant in &declaration.variants {
                for field in &variant.fields {
                    visit(field.ty)?;
                }
            }
            for interface in &declaration.interfaces {
                visit(*interface)?;
            }
        }
        NominalLocalId::Class(id) => {
            let declaration = &export.classes[id];
            class(export, declaration, &mut visit)?;
            for id in &declaration.constructors {
                let constructor = &export.class_constructors[*id];
                if exported(constructor.access.declared) {
                    for parameter in &constructor.parameters {
                        visit(parameter.ty)?;
                    }
                }
            }
        }
        NominalLocalId::Interface(id) => {
            for parent in &export.interfaces[id].parents {
                visit(export.interface_applications[*parent].canonical_type)?;
            }
        }
        NominalLocalId::Object(id) => {
            class(
                export,
                &export.classes[export.objects[id].backing_class],
                &mut visit,
            )?;
        }
    }
    Ok(())
}

fn class(
    export: &ExportHir,
    declaration: &ClassDecl,
    visit: &mut impl FnMut(TypeId) -> Result<(), Error>,
) -> Result<(), Error> {
    for field in &declaration.fields {
        visit(export.class_fields[*field].ty)?;
    }
    if let Some(base) = declaration.base_class {
        visit(base)?;
    }
    for interface in &declaration.interfaces {
        visit(*interface)?;
    }
    Ok(())
}

fn exported(visibility: DeclaredVisibility) -> bool {
    matches!(
        visibility,
        DeclaredVisibility::Public | DeclaredVisibility::Protected
    )
}
