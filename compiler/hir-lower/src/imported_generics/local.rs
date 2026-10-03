use super::*;

impl Lowerer {
    pub(crate) fn register_imported_local_function(
        &mut self,
        descriptor: &hir::DefaultLocalFunctionV1,
        capture_bindings: Vec<hir::BindingId>,
    ) {
        self.imported_generic_templates.local_functions.insert(
            descriptor.declaration(),
            ImportedLocalFunctionSource {
                descriptor: descriptor.clone(),
                capture_bindings,
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
        let owner = match declaration {
            CallableTemplateOrigin::Function(id) => hir::DefaultCallableDeclarationV1::Function(id),
            CallableTemplateOrigin::GenericFunction(id) => {
                hir::DefaultCallableDeclarationV1::GenericFunction(id)
            }
            _ => unreachable!("local declarations are named source functions"),
        };
        let body = self
            .dependencies
            .as_ref()
            .expect("an imported local function retains its dependency catalog")
            .callable_body(owner)
            .ok_or("dependency local function is missing its implementation")?;
        let parent = body
            .lexical_parent()
            .ok_or("dependency local function is missing its lexical parent")?;
        let origin = hir::ImportedCallableTemplateOrigin::Local {
            parent,
            descriptor: source.descriptor,
            capture_bindings: source.capture_bindings,
        };
        let name = body
            .source_name()
            .ok_or("dependency local function has no source name")?
            .as_str()
            .to_owned();
        let source = PreparedImportedCallableSource::Body(body);
        let prepared = self.prepare_imported_lexical_callable(origin, name, source)?;
        Ok(Some(self.insert_imported_template(declaration, prepared)))
    }

    pub(super) fn prepare_imported_lexical_callable(
        &mut self,
        origin: hir::ImportedCallableTemplateOrigin,
        name: String,
        source: PreparedImportedCallableSource,
    ) -> Result<PreparedImportedGeneric, String> {
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
        let capture_count = match &origin {
            hir::ImportedCallableTemplateOrigin::Local { descriptor, .. } => {
                let count = descriptor.capture_count() as usize;
                if count > body.parameters().len()
                    || descriptor.owner_type_parameter_count() as usize > parameters.len()
                {
                    return Err(
                        "dependency local function descriptor does not match its body parameters"
                            .into(),
                    );
                }
                count
            }
            hir::ImportedCallableTemplateOrigin::Closure { .. } => 0,
            _ => unreachable!("lexical preparation receives a local function or closure"),
        };
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
            signature: hir::CallableSignature::from_source_effects(
                name,
                value_parameters,
                return_type,
                body.effects(),
                definition.span,
            ),
            type_parameters: hir::ImportedCallableTypeParameters::Substitution(parameters),
            no_gc_type_params,
            gc_free_pointee_requirements,
            receiver: None,
            origin: definition,
        };
        Ok(PreparedImportedGeneric {
            signature,
            source,
            bindings,
            locals,
            statements: None,
        })
    }
}
