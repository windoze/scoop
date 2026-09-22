//! Storage dependencies retain private source types without adding lookup names.

use super::*;

pub(super) fn visit_fields(
    export: &ExportHir,
    local: LocalNominalId,
    mut visit: impl FnMut(TypeId) -> Result<(), Error>,
) -> Result<(), Error> {
    match local {
        LocalNominalId::Struct(id) => {
            for field in export.structs[id].semantic_fields() {
                visit(field.ty)?;
            }
        }
        LocalNominalId::Enum(id) => {
            for variant in &export.enums[id].variants {
                for field in &variant.fields {
                    visit(field.ty)?;
                }
            }
        }
        LocalNominalId::Class(id) => {
            for field in &export.classes[id].fields {
                visit(export.class_fields[*field].ty)?;
            }
        }
        LocalNominalId::Object(id) => {
            for field in &export.classes[export.objects[id].backing_class].fields {
                visit(export.class_fields[*field].ty)?;
            }
        }
        LocalNominalId::Interface(_) => return Ok(()),
    }
    Ok(())
}

impl Roots<'_> {
    pub(super) fn require_field_type(
        &mut self,
        export: &ExportHir,
        index: &Index,
        ty: TypeId,
        depth: u64,
    ) -> Result<(), Error> {
        let path = WirePath::root();
        self.meter
            .check_semantic_depth(depth, &path)
            .map_err(resource)?;
        work(self.meter, self.field_types.len())?;
        if self.field_types.contains(&ty) {
            return Ok(());
        }
        self.meter
            .check_table_entries(self.field_types.len() as u64 + 1, &path)
            .map_err(resource)?;
        self.meter
            .charge_collection_slots(1, &path)
            .map_err(resource)?;
        self.meter.charge_nodes(1, &path).map_err(resource)?;
        self.field_types.insert(ty);
        work(self.meter, index.nodes.len())?;
        if matches!(export.types[ty], Type::Class(_)) {
            self.meter
                .charge_work(export.objects.len() as u64, &path)
                .map_err(resource)?;
        }
        match owner_resolution::from_type(export, ty) {
            Some(owner) => {
                if index.nodes.contains_key(&owner) {
                    self.require(owner, false)?;
                }
            }
            None if matches!(
                export.types[ty],
                Type::Tuple(_) | Type::Function(_) | Type::Param(_)
            ) => {}
            None => return Err(invalid("storage type has no source nominal identity")),
        }
        let children = match &export.types[ty] {
            Type::Struct(id) => export.struct_applications[*id].arguments.as_slice(),
            Type::Enum(id) => export.enum_applications[*id].arguments.as_slice(),
            Type::Class(id) => export.class_applications[*id].arguments.as_slice(),
            Type::Interface(id) => export.interface_applications[*id].arguments.as_slice(),
            Type::Tuple(elements) => elements.as_slice(),
            Type::Function(id) | Type::FunPtr(id) => {
                let signature = &export.function_types[*id];
                self.require_field_type(export, index, signature.return_type, depth + 1)?;
                signature.parameter_types.as_slice()
            }
            Type::Ptr(pointee) => {
                return self.require_field_type(export, index, *pointee, depth + 1);
            }
            Type::Unit
            | Type::Integer(_)
            | Type::Boolean
            | Type::String
            | Type::Any
            | Type::Param(_) => &[],
        };
        for child in children {
            self.require_field_type(export, index, *child, depth + 1)?;
        }
        Ok(())
    }
}
