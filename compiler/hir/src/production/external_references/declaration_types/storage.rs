//! Semantic storage edges are rooted in actual materialization, not type arenas.

use super::*;
use crate::concrete::{ClassRepresentation, StructRepresentation, TypeKind};

pub(super) fn collect<E>(
    local: &crate::LocalConcreteHirOutput,
    output: &mut Collector<'_>,
) -> Result<(), Error<E>> {
    let module = output.module;
    for ty in local
        .materialized_type_closure(output.meter)
        .map_err(Error::MaterializedTypes)?
    {
        match &module.types[ty].kind {
            TypeKind::Struct(id) => {
                if let StructRepresentation::Declared { fields, .. } =
                    &module.structs[*id].representation
                {
                    for field in fields {
                        output.add(field.ty, |exact| Site::FieldStorage {
                            field: field.identity,
                            exact,
                        })?;
                    }
                }
            }
            TypeKind::Class(id) => {
                if let ClassRepresentation::Declared { fields, .. } =
                    &module.classes[*id].representation
                {
                    for field in fields {
                        output.add(field.ty, |exact| Site::FieldStorage {
                            field: field.identity,
                            exact,
                        })?;
                    }
                }
            }
            TypeKind::Enum(id) => {
                for variant in &module.enums[*id].variants {
                    output
                        .meter
                        .charge_work(1, &WirePath::root())
                        .map_err(Error::Resource)?;
                    for field in &variant.fields {
                        output.add(field.ty, |exact| Site::EnumVariantFieldStorage {
                            field: field.identity,
                            exact,
                        })?;
                    }
                }
            }
            TypeKind::Interface(_)
            | TypeKind::Unit
            | TypeKind::Integer(_)
            | TypeKind::Boolean
            | TypeKind::String
            | TypeKind::Any
            | TypeKind::Tuple(_)
            | TypeKind::Ptr(_)
            | TypeKind::Function(_)
            | TypeKind::FunPtr(_) => continue,
        }
    }
    for (_, unit) in module.initialization_units.iter() {
        output.add(module.string, |exact| Site::InitializationCycleMessage {
            unit: unit.identity.id(),
            exact,
        })?;
    }
    Ok(())
}
