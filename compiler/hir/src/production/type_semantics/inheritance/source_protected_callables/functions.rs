use super::*;
use scoop_identity::DuplicateSignatureKey;

impl Projection<'_, '_> {
    pub(super) fn functions(&mut self) -> Result<(), Error> {
        for (id, function) in self.export.functions.iter() {
            work(self.meter, 1)?;
            let HirFunctionIdentity::Source(identity) = &self.export.function_identities[id] else {
                continue;
            };
            let (declaration, subject) = match identity {
                HirSourceFunctionIdentity::Plain(record) => (
                    CallableTemplateOrigin::Function(record.id()),
                    DefinitionOriginSubject::Function(record.id()),
                ),
                HirSourceFunctionIdentity::Generic(record) => (
                    CallableTemplateOrigin::GenericFunction(record.id()),
                    DefinitionOriginSubject::GenericFunction(record.id()),
                ),
            };
            if !self.take(declaration)? {
                continue;
            }
            if function.access.declared != DeclaredVisibility::Protected {
                return Err(invalid(
                    "protected callable inventory refers to another visibility",
                ));
            }
            let method = function
                .method
                .ok_or_else(|| invalid("protected function has no member owner"))?;
            let key = identity.declaration();
            let DuplicateSignatureKey::Function {
                receiver: scoop_identity::OptionalSignatureType::Absent,
                parameters: expected,
                ..
            } = key.duplicate_signature()
            else {
                return Err(invalid(
                    "protected member has an incompatible source signature key",
                ));
            };
            let visible = function.type_param_count();
            self.meter
                .check_table_entries(visible as u64, &WirePath::root())
                .map_err(resource)?;
            self.meter
                .charge_collection_slots(visible as u64, &WirePath::root())
                .map_err(resource)?;
            for parameter in function.type_params() {
                resources::binders(
                    self.export,
                    std::slice::from_ref(parameter),
                    visible,
                    self.meter,
                )?;
            }
            let binders = self
                .signatures
                .function_binders(function)
                .map_err(invalid)?;
            let type_parameters = match &function.genericity {
                FunctionGenericity::Plain | FunctionGenericity::OwnerParameterizedMethod { .. } => {
                    self.signatures.project_binder_list(&[], &binders)
                }
                FunctionGenericity::Generic { parameters, .. } => {
                    self.signatures.project_binder_list(parameters, &binders)
                }
                FunctionGenericity::GenericMethod {
                    method_parameters, ..
                } => self
                    .signatures
                    .project_binder_iter(method_parameters.iter(), &binders),
            }
            .map_err(invalid)?;
            let path = WirePath::root();
            self.meter
                .charge_work(
                    self.export.source_parameter_interfaces.len() as u64 * 2,
                    &path,
                )
                .map_err(resource)?;
            for interface in &self.export.source_parameter_interfaces {
                if interface.owner != ExportParameterOwner::Function(id) {
                    continue;
                }
                self.meter
                    .check_table_entries(interface.parameters.len() as u64, &path)
                    .map_err(resource)?;
                self.meter
                    .charge_collection_slots(interface.parameters.len() as u64 * 3, &path)
                    .map_err(resource)?;
                for parameter in &interface.parameters {
                    resources::name(&parameter.name, self.meter)?;
                    let ty = match parameter.calling {
                        ExportParameterCalling::Required { value_type }
                        | ExportParameterCalling::Default { value_type, .. } => value_type,
                        ExportParameterCalling::Vararg { parameter_type, .. } => {
                            self.export.export_vararg_parameter_types[parameter_type].array_type
                        }
                    };
                    resources::ty(self.export, ty, visible, 3, self.meter)?;
                }
            }
            let parameters = callable_interfaces::source_parameter_shapes(
                self.export,
                &self.signatures,
                ExportParameterOwner::Function(id),
                &binders,
                expected,
            )
            .map_err(invalid)?;
            resources::ty(self.export, function.return_ty, visible, 3, self.meter)?;
            let result = self
                .signatures
                .map_type(function.return_ty, &binders)
                .map_err(invalid)?;
            let access = self.access(key, subject)?;
            let payload = ProtectedCallablePayloadV1::try_new(
                declaration,
                owner(&access)?,
                type_parameters,
                parameters,
                result,
                callable_interfaces::source_function_effects(self.export, function)
                    .map_err(invalid)?,
                modality(method.modifier),
                self.slots(method)?,
            )
            .map_err(invalid)?;
            self.push(declaration, access, payload)?;
        }
        Ok(())
    }
}
