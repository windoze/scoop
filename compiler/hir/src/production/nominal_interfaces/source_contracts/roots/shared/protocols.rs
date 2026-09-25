use super::*;

impl SourceRoots {
    pub(super) fn protocol(
        &mut self,
        export: &ExportHir,
        protocol: &ExportParameterInterface,
        index: &super::super::Index,
        roots: &mut Roots,
    ) -> Result<(), Error> {
        if let ExportParameterOwner::Function(id) = protocol.owner {
            super::super::callables::function(export, id, &mut |ty| {
                roots.require_field_type(export, index, ty)
            })?;
        }
        for parameter in &protocol.parameters {
            let source = match parameter.calling {
                ExportParameterCalling::Required { value_type } => {
                    roots.require_field_type(export, index, value_type)?;
                    None
                }
                ExportParameterCalling::Default { value_type, source } => {
                    roots.require_field_type(export, index, value_type)?;
                    Some(source)
                }
                ExportParameterCalling::Vararg {
                    parameter_type,
                    omission,
                } => {
                    let parameter = &export.export_vararg_parameter_types[parameter_type];
                    roots.require_field_type(export, index, parameter.element_type)?;
                    roots.require_field_type(export, index, parameter.array_type)?;
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
        roots: &mut Roots,
    ) -> Result<(), Error> {
        let property = &export.properties[id];
        roots.require_field_type(export, index, property.ty)?;
        if let PropertyOwner::Extension(id) = property.owner {
            let extension = &export.extension_properties[id];
            roots.require_field_type(export, index, extension.receiver_ty)?;
            super::super::callables::bounds(export, &extension.type_params, &mut |ty| {
                roots.require_field_type(export, index, ty)
            })?;
        }
        Ok(())
    }
}
