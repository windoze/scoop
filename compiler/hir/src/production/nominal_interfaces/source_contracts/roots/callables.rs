use super::*;

pub(super) fn visit_types(
    export: &ExportHir,
    local: LocalNominalId,
    mut visit: impl FnMut(TypeId) -> Result<(), Error>,
) -> Result<(), Error> {
    let (parameters, methods): (&[TypeParamDecl], &[FunctionId]) = match local {
        LocalNominalId::Struct(id) => {
            let nominal = &export.structs[id];
            for constructor in &nominal.constructors {
                for parameter in &export.struct_constructors[*constructor].parameters {
                    visit(parameter.ty)?;
                }
            }
            (&nominal.type_params, &nominal.methods)
        }
        LocalNominalId::Class(id) => {
            let nominal = &export.classes[id];
            for constructor in &nominal.constructors {
                if export.constructor_identities[*constructor]
                    .source_record()
                    .is_none()
                {
                    continue;
                }
                for parameter in &export.class_constructors[*constructor].parameters {
                    visit(parameter.ty)?;
                }
            }
            (&nominal.type_params, &nominal.methods)
        }
        LocalNominalId::Enum(id) => (&export.enums[id].type_params, &export.enums[id].methods),
        LocalNominalId::Object(id) => (
            &[],
            &export.classes[export.objects[id].backing_class].methods,
        ),
        LocalNominalId::Interface(id) => {
            let nominal = &export.interfaces[id];
            for member in &nominal.methods {
                let method = &export.interface_methods[*member];
                if method.role == InterfaceMemberRole::Function {
                    function(export, method.function, &mut visit)?;
                }
            }
            (&nominal.type_params, &nominal.private_methods)
        }
    };
    bounds(export, parameters, &mut visit)?;
    for method in methods {
        function(export, *method, &mut visit)?;
    }
    Ok(())
}

pub(super) fn function(
    export: &ExportHir,
    id: FunctionId,
    visit: &mut impl FnMut(TypeId) -> Result<(), Error>,
) -> Result<(), Error> {
    if !matches!(
        export.function_identities[id],
        HirFunctionIdentity::Source(_)
    ) {
        return Ok(());
    }
    let function = &export.functions[id];
    for parameter in &function.params {
        visit(parameter.ty)?;
    }
    visit(function.return_ty)?;
    match &function.genericity {
        FunctionGenericity::Plain => Ok(()),
        FunctionGenericity::Generic { parameters, .. } => bounds(export, parameters, visit),
        FunctionGenericity::OwnerParameterizedMethod {
            owner_parameters, ..
        } => bounds(export, owner_parameters, visit),
        FunctionGenericity::GenericMethod {
            owner_parameters,
            method_parameters,
            ..
        } => {
            bounds(export, owner_parameters, visit)?;
            for parameter in method_parameters.iter() {
                bounds(export, std::slice::from_ref(parameter), visit)?;
            }
            Ok(())
        }
    }
}

pub(super) fn bounds(
    export: &ExportHir,
    parameters: &[TypeParamDecl],
    visit: &mut impl FnMut(TypeId) -> Result<(), Error>,
) -> Result<(), Error> {
    for parameter in parameters {
        for bound in parameter.nominal_bounds_in_source_order() {
            let ty = match bound {
                NominalBoundRef::Class(bound) => {
                    export.class_applications[bound.application].canonical_type
                }
                NominalBoundRef::Interface(bound) => {
                    export.interface_applications[bound.application].canonical_type
                }
                NominalBoundRef::ImportedClass(bound)
                | NominalBoundRef::ImportedInterface(bound) => bound.ty,
            };
            visit(ty)?;
        }
    }
    Ok(())
}
