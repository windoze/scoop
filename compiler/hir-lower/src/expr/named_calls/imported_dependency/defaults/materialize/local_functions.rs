use super::*;

impl Lowerer {
    pub(super) fn materialize_imported_local_declaration(
        &mut self,
        descriptor: &hir::DefaultLocalFunctionV1,
        origin: hir::DefinitionOrigin,
        context: &ImportedDefaultContext<'_>,
    ) -> Result<hir::LocalFunctionId, ImportedDefaultMaterializationError> {
        let bindings = descriptor
            .captures()
            .iter()
            .map(|capture| self.materialize_capture_binding(capture, context))
            .collect::<Result<Vec<_>, _>>()?;
        self.register_imported_local_function(descriptor, bindings.clone());
        let template = self
            .request_imported_local_function(descriptor.declaration())
            .map_err(ImportedDefaultMaterializationError::Plan)?
            .expect("the local declaration was registered before its implementation request");
        let signature = self.imported_generic_templates[template].signature.clone();
        let declaration_type = self.intern_function_type(
            signature.is_suspend,
            signature
                .params
                .iter()
                .skip(descriptor.capture_count() as usize)
                .map(|parameter| parameter.ty)
                .collect(),
            signature.return_ty,
        );
        let hir::Type::Function(declaration_function_type) = self.types[declaration_type] else {
            unreachable!("a lexical declaration has a function signature");
        };
        let parameters = signature.type_parameters.ids();
        let own_parameters = &parameters[descriptor.owner_type_parameter_count() as usize..];
        let mut signature_bindings = context.bindings.clone();
        if !own_parameters.is_empty() {
            signature_bindings = context
                .bindings
                .iter()
                .map(|(key, &ty)| {
                    let key = match key {
                        scoop_identity::SignatureTypeKey::Binder { depth, index } => {
                            scoop_identity::SignatureTypeKey::Binder {
                                depth: depth
                                    .checked_add(1)
                                    .expect("validated lexical depth fits u32"),
                                index: *index,
                            }
                        }
                        _ => key.clone(),
                    };
                    (key, ty)
                })
                .collect();
            for (index, &parameter) in own_parameters.iter().enumerate() {
                signature_bindings.insert(
                    scoop_identity::SignatureTypeKey::Binder {
                        depth: 0,
                        index: index as u32,
                    },
                    self.intern_type(hir::Type::Param(parameter)),
                );
            }
        }
        let function_type = self
            .imported_default_type_with_bindings(descriptor.function_type(), &signature_bindings)
            .map_err(|error| ImportedDefaultMaterializationError::Plan(error.to_string()))?;
        let hir::Type::Function(function_type) = self.types[function_type] else {
            unreachable!("a validated local declaration carries a function type");
        };
        let captures = self.materialize_imported_captures(
            descriptor.captures(),
            &bindings,
            hir::ExpressionOrigin::Definition(origin),
            context,
        )?;
        Ok(self.local_functions.alloc(hir::LocalFunction {
            definition: hir::LocalFunctionDefinition::Template(template),
            definition_path: descriptor.definition_path().clone(),
            declaration_function_type,
            function_type,
            captures,
            owner_type_param_count: descriptor.owner_type_parameter_count() as usize,
            origin,
            span: origin.span,
        }))
    }
}
