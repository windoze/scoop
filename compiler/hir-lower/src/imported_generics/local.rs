use super::*;

impl Lowerer {
    pub(crate) fn register_imported_local_function(
        &mut self,
        parent: hir::ImportedCallableTemplateParent,
        descriptor: &hir::DefaultLocalFunctionV1,
    ) {
        self.imported_generic_templates.local_functions.insert(
            descriptor.declaration(),
            ImportedLocalFunctionSource {
                parent,
                descriptor: descriptor.clone(),
            },
        );
    }

    pub(crate) fn request_imported_local_function(
        &mut self,
        declaration: CallableTemplateOrigin,
    ) -> Result<Option<hir::ImportedGenericCallableTemplateId>, String> {
        let Some(source) = self
            .imported_generic_templates
            .local_functions
            .get(&declaration)
            .cloned()
        else {
            return Ok(None);
        };
        if let Some(id) = self
            .imported_generic_templates
            .by_declaration
            .get(&declaration)
        {
            return Ok(Some(*id));
        }
        let origin = hir::ImportedCallableTemplateOrigin::Local {
            parent: source.parent,
            descriptor: source.descriptor,
        };
        let body = self
            .dependencies
            .as_ref()
            .expect("an imported local function retains its dependency catalog")
            .callable_body(origin.body_owner())
            .ok_or("dependency local function is missing its implementation")?;
        let name = body
            .source_name()
            .ok_or("dependency local function has no source name")?
            .as_str()
            .to_owned();
        let source = PreparedImportedCallableSource::Body(body);
        let body = source.body();
        let definition = self.import_generic_definition(&source, body.definition_origin())?;
        let mut bindings = ImportedTypeBindings::new();
        let mut parameters = Vec::new();
        for (slot, key) in body.type_parameters().arguments().iter().enumerate() {
            let id = self.fresh_type_param(slot);
            let ty = self.intern_type(hir::Type::Param(id));
            bindings.insert(key.clone(), ty);
            parameters.push(id);
        }
        let (mut locals, selectors) = self.imported_body_locals(&source, &bindings)?;
        let hir::ImportedCallableTemplateOrigin::Local { descriptor, .. } = &origin else {
            unreachable!("the local descriptor was retained above")
        };
        let capture_count = descriptor.capture_count() as usize;
        if capture_count > body.parameters().len()
            || descriptor.owner_type_parameter_count() as usize > parameters.len()
        {
            return Err(
                "dependency local function descriptor does not match its body parameters".into(),
            );
        }
        let value_parameters = body
            .parameters()
            .iter()
            .enumerate()
            .map(|(index, selector)| {
                let local = selectors[selector];
                let name = if index < capture_count {
                    format!("$capture.{index}")
                } else {
                    format!("$argument.{}", index - capture_count)
                };
                locals[local].name = name.clone();
                hir::Param {
                    name,
                    ty: locals[local].ty,
                    local,
                }
            })
            .collect();
        let return_type = self.imported_generic_type(body.result(), &bindings)?;
        let (no_gc_type_params, gc_free_pointee_requirements) =
            self.imported_body_predicates(body, &bindings)?;
        let signature = hir::ImportedGenericCallableSignature {
            declaration: origin,
            name,
            type_parameters: hir::ImportedCallableTypeParameters::Substitution(parameters),
            no_gc_type_params,
            gc_free_pointee_requirements,
            parameters: value_parameters,
            return_type,
            effects: body.effects(),
            receiver: None,
            origin: definition,
            span: definition.span,
        };
        let prepared = PreparedImportedGeneric {
            signature,
            source,
            bindings,
            locals,
            statements: None,
        };
        Ok(Some(self.insert_imported_template(declaration, prepared)))
    }
}
