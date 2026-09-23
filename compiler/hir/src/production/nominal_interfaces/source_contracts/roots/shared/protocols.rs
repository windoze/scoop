use super::*;

impl SourceRoots {
    pub(super) fn protocol(
        &mut self,
        export: &ExportHir,
        protocol: &ExportParameterInterface,
        index: &super::super::Index,
        roots: &mut Roots<'_>,
    ) -> Result<(), Error> {
        if let ExportParameterOwner::Function(id) = protocol.owner {
            super::super::callables::function(export, id, &mut |ty| {
                roots.require_field_type(export, index, ty, 1)
            })?;
        }
        for parameter in &protocol.parameters {
            roots
                .meter
                .charge_work(1, &WirePath::root())
                .map_err(resource)?;
            let source = match parameter.calling {
                ExportParameterCalling::Required { value_type } => {
                    roots.require_field_type(export, index, value_type, 1)?;
                    None
                }
                ExportParameterCalling::Default { value_type, source } => {
                    roots.require_field_type(export, index, value_type, 1)?;
                    Some(source)
                }
                ExportParameterCalling::Vararg {
                    parameter_type,
                    omission,
                } => {
                    let parameter = &export.export_vararg_parameter_types[parameter_type];
                    roots.require_field_type(export, index, parameter.element_type, 1)?;
                    roots.require_field_type(export, index, parameter.array_type, 1)?;
                    match omission {
                        ExportVarargOmission::EmptyArray => None,
                        ExportVarargOmission::Default(source) => Some(source),
                    }
                }
            };
            if let Some(source) = source {
                self.default_source(export, source, index, roots)?;
            }
        }
        Ok(())
    }

    pub(super) fn property_types(
        &mut self,
        export: &ExportHir,
        id: PropertyId,
        index: &super::super::Index,
        roots: &mut Roots<'_>,
    ) -> Result<(), Error> {
        let property = &export.properties[id];
        roots.require_field_type(export, index, property.ty, 1)?;
        if let PropertyOwner::Extension(id) = property.owner {
            let extension = &export.extension_properties[id];
            roots.require_field_type(export, index, extension.receiver_ty, 1)?;
            super::super::callables::bounds(export, &extension.type_params, &mut |ty| {
                roots.require_field_type(export, index, ty, 1)
            })?;
        }
        Ok(())
    }
}
