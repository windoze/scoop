use super::*;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(super) fn prepare_imported_generic(
        &mut self,
        declaration: hir::ImportedCallableDeclaration,
        identity: PersistentGenericFunctionId,
    ) -> Result<PreparedImportedGeneric, String> {
        let body = declaration
            .callable_body()
            .ok_or("dependency generic callable has no body")?;
        let interface = declaration.interface();
        if body.owner() != hir::DefaultCallableDeclarationV1::GenericFunction(identity)
            || body.effects() != interface.effects()
            || body.result() != interface.result()
        {
            return Err("dependency callable body does not match its declaration header".into());
        }
        let declared_parameters = interface
            .receiver()
            .into_iter()
            .chain(
                interface
                    .parameters()
                    .parameters()
                    .iter()
                    .map(|parameter| parameter.value_type()),
            )
            .collect::<Vec<_>>();
        if declared_parameters.len() != body.parameters().len()
            || declared_parameters
                .iter()
                .zip(body.parameters())
                .any(|(ty, selector)| {
                    body.locals()
                        .get(selector)
                        .is_none_or(|local| local.value_type() != *ty)
                })
        {
            return Err(
                "dependency callable body parameters do not match its declaration signature".into(),
            );
        }
        let origin = self.import_generic_definition(&declaration, body.definition_origin())?;
        let span = origin.span;
        let binders = declaration.interface().type_parameters().binders();
        if binders.len() != body.type_parameters().arguments().len() {
            return Err(
                "dependency callable signature and body have different binder arity".into(),
            );
        }
        let mut bindings = ImportedTypeBindings::new();
        let mut type_parameters = Vec::new();
        let mut parameter_ids = Vec::new();
        for (slot, signature) in body.type_parameters().arguments().iter().enumerate() {
            let id = self.fresh_type_param(slot);
            let ty = self.intern_type(hir::Type::Param(id));
            bindings.insert(signature.clone(), ty);
            parameter_ids.push(id);
        }
        for (binder, id) in binders.iter().zip(parameter_ids) {
            let bounds = match binder.bounds() {
                hir::TypeParameterBoundsV1::Unconstrained => hir::TypeParamBounds::Unconstrained,
                hir::TypeParameterBoundsV1::Value => hir::TypeParamBounds::Value { span },
                hir::TypeParameterBoundsV1::Ref => hir::TypeParamBounds::Ref { span },
                hir::TypeParameterBoundsV1::Nominal(bounds) => {
                    self.imported_generic_nominal_bounds(bounds, &bindings, span)?
                }
            };
            type_parameters.push(hir::TypeParamDecl {
                id,
                name: binder.name().as_str().to_owned(),
                bounds,
                span,
            });
        }
        let mut locals = Arena::new();
        let mut selectors = BTreeMap::new();
        for local in body.locals().records() {
            let ty = self.imported_generic_type(local.value_type(), &bindings)?;
            let definition = match local.definition() {
                hir::TemplateLocalDefinitionV1::Source(source) => {
                    hir::LocalValueDefinitionSite::Source(
                        self.import_generic_definition(&declaration, source)?,
                    )
                }
                hir::TemplateLocalDefinitionV1::Synthetic => {
                    hir::LocalValueDefinitionSite::Synthetic
                }
            };
            let local_id = locals.alloc(hir::Local {
                binding: self.fresh_binding(),
                selector: local.selector().clone(),
                definition,
                name: format!("$dependency.local.{}", locals.len()),
                ty,
                mutable: local.mutable().into(),
            });
            selectors.insert(local.selector().clone(), local_id);
        }
        let source_parameters = declaration
            .source_interface()
            .ok_or("dependency callable is missing its source parameters")?
            .parameters()
            .parameters();
        let receiver_count = usize::from(interface.receiver().is_some());
        let parameters = body
            .parameters()
            .iter()
            .enumerate()
            .map(|(index, selector)| {
                let local = *selectors
                    .get(selector)
                    .expect("body parameters reference validated locals");
                locals[local].name = if index < receiver_count {
                    "this".to_owned()
                } else {
                    source_parameters[index - receiver_count]
                        .name()
                        .as_str()
                        .to_owned()
                };
                hir::Param {
                    name: locals[local].name.clone(),
                    ty: locals[local].ty,
                    local,
                }
            })
            .collect::<Vec<_>>();
        let receiver = declaration
            .interface()
            .receiver()
            .map(|ty| self.imported_generic_type(ty, &bindings))
            .transpose()?;
        let return_type = self.imported_generic_type(body.result(), &bindings)?;
        let predicate = |key: &scoop_identity::SignatureTypeKey| {
            let ty = bindings
                .get(key)
                .ok_or("dependency predicate names an absent binder")?;
            match self.types[*ty] {
                hir::Type::Param(id) => Ok(id),
                _ => Err("dependency predicate is not a parameter"),
            }
        };
        let no_gc_type_params = body
            .predicates()
            .no_gc()
            .arguments()
            .iter()
            .map(predicate)
            .collect::<Result<_, _>>()?;
        let gc_free_pointee_requirements = body
            .predicates()
            .gc_free_pointees()
            .arguments()
            .iter()
            .map(|key| predicate(key).map(|type_param| hir::RequiresGcFreePointee { type_param }))
            .collect::<Result<_, _>>()?;
        let signature = hir::ImportedGenericCallableSignature {
            declaration: identity,
            name: declaration.name().to_owned(),
            type_parameters,
            no_gc_type_params,
            gc_free_pointee_requirements,
            parameters,
            return_type,
            receiver,
            effects: body.effects(),
            origin,
            span,
        };
        Ok(PreparedImportedGeneric {
            signature,
            declaration,
            bindings,
            locals,
            statements: None,
        })
    }

    fn imported_generic_type(
        &mut self,
        signature: &scoop_identity::SignatureTypeKey,
        bindings: &ImportedTypeBindings,
    ) -> Result<hir::TypeId, String> {
        self.imported_signature_type_with_bindings(signature, bindings)
            .map_err(|error| {
                format!("cannot resolve dependency signature type {signature:?}: {error:?}")
            })
    }

    fn import_generic_definition(
        &mut self,
        declaration: &hir::ImportedCallableDeclaration,
        source: &hir::ExportDefinitionSourceV1,
    ) -> Result<hir::DefinitionOrigin, String> {
        let definition = declaration
            .definition_source(source)
            .ok_or("dependency body is missing its definition source")?;
        self.import_dependency_definition_origin(source, definition)
            .map_err(|error| error.to_string())
    }

    fn imported_generic_nominal_bounds(
        &mut self,
        bounds: &hir::NominalTypeParameterBoundsV1,
        bindings: &ImportedTypeBindings,
        span: hir::Span,
    ) -> Result<hir::TypeParamBounds, String> {
        let class = bounds
            .class()
            .map(|key| {
                let ty = self.imported_generic_type(key, bindings)?;
                Ok::<_, String>(hir::ImportedNominalTypeBound { ty, span })
            })
            .transpose()?;
        let interfaces = bounds
            .interfaces()
            .values()
            .iter()
            .map(|key| {
                let ty = self.imported_generic_type(key, bindings)?;
                Ok::<_, String>(hir::ImportedNominalTypeBound { ty, span })
            })
            .collect::<Result<_, String>>()?;
        Ok(hir::TypeParamBounds::ImportedNominal(
            hir::ImportedNominalBounds { class, interfaces },
        ))
    }
}
