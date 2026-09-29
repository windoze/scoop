use super::*;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(super) fn prepare_imported_generic(
        &mut self,
        declaration: hir::ImportedCallableDeclaration,
        identity: hir::ImportedCallableTemplateOrigin,
    ) -> Result<PreparedImportedGeneric, String> {
        let body = declaration
            .callable_body()
            .ok_or("dependency generic callable has no body")?;
        let interface = declaration.interface();
        if body.owner() != identity.body_owner()
            || body.effects() != interface.effects()
            || body.result() != interface.result()
        {
            return Err("dependency callable body does not match its declaration header".into());
        }
        let nominal = match &identity {
            hir::ImportedCallableTemplateOrigin::Nominal { owner, .. } => self
                .dependencies
                .as_ref()
                .and_then(|dependencies| dependencies.nominal_declaration(*owner))
                .cloned(),
            _ => None,
        };
        let extension = match &identity {
            hir::ImportedCallableTemplateOrigin::ExtensionAccessor(accessor) => self
                .dependencies
                .as_ref()
                .and_then(|dependencies| dependencies.property_for_accessor(*accessor))
                .cloned(),
            _ => None,
        };
        let receiver_signature = if nominal.is_some() {
            Some(
                body.locals()
                    .get(
                        body.parameters()
                            .first()
                            .ok_or("nominal member is missing its receiver")?,
                    )
                    .expect("validated parameters name body locals")
                    .value_type(),
            )
        } else {
            interface.receiver()
        };
        let declared_parameters = receiver_signature
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
        let source = PreparedImportedCallableSource::Declaration(Box::new(declaration.clone()));
        let origin = self.import_generic_definition(&source, body.definition_origin())?;
        let span = origin.span;
        let binders = nominal
            .as_ref()
            .into_iter()
            .flat_map(|nominal| nominal.interface.type_parameters().binders())
            .chain(
                extension
                    .as_ref()
                    .into_iter()
                    .flat_map(|property| property.type_parameters().binders()),
            )
            .chain(declaration.interface().type_parameters().binders())
            .collect::<Vec<_>>();
        if binders.len() != body.type_parameters().arguments().len() {
            return Err(
                "dependency callable signature and body have different binder arity".into(),
            );
        }
        let (type_parameters, bindings) = self.prepare_imported_type_parameters(
            &binders,
            body.type_parameters().arguments(),
            span,
        )?;
        let (mut locals, selectors) = self.imported_body_locals(&source, &bindings)?;
        let source_parameters = interface.parameters().parameters();
        let receiver_count = usize::from(receiver_signature.is_some());
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
        let receiver = receiver_signature
            .map(|ty| self.imported_generic_type(ty, &bindings))
            .transpose()?;
        let return_type = self.imported_generic_type(body.result(), &bindings)?;
        let (no_gc_type_params, gc_free_pointee_requirements) =
            self.imported_body_predicates(body, &bindings)?;
        let signature = hir::ImportedGenericCallableSignature {
            declaration: identity,
            name: declaration.name().to_owned(),
            type_parameters: hir::ImportedCallableTypeParameters::Declared(type_parameters),
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
            source,
            bindings,
            locals,
            statements: None,
        })
    }

    pub(crate) fn prepare_imported_type_parameters(
        &mut self,
        binders: &[&hir::TypeParameterBinderV1],
        signatures: &[scoop_identity::SignatureTypeKey],
        span: scoop_ast::Span,
    ) -> Result<(Vec<hir::TypeParamDecl>, ImportedTypeBindings), String> {
        let mut bindings = ImportedTypeBindings::new();
        let mut parameter_ids = Vec::new();
        for (slot, signature) in signatures.iter().enumerate() {
            let id = self.fresh_type_param(slot);
            let ty = self.intern_type(hir::Type::Param(id));
            bindings.insert(signature.clone(), ty);
            parameter_ids.push(id);
        }
        let mut parameters = Vec::new();
        for (binder, id) in binders.iter().zip(parameter_ids) {
            let bounds = match binder.bounds() {
                hir::TypeParameterBoundsV1::Unconstrained => hir::TypeParamBounds::Unconstrained,
                hir::TypeParameterBoundsV1::Value => hir::TypeParamBounds::Value { span },
                hir::TypeParameterBoundsV1::Ref => hir::TypeParamBounds::Ref { span },
                hir::TypeParameterBoundsV1::Nominal(bounds) => {
                    self.imported_generic_nominal_bounds(bounds, &bindings, span)?
                }
            };
            parameters.push(hir::TypeParamDecl {
                id,
                name: binder.name().as_str().to_owned(),
                bounds,
                span,
            });
        }
        Ok((parameters, bindings))
    }

    pub(crate) fn imported_generic_type(
        &mut self,
        signature: &scoop_identity::SignatureTypeKey,
        bindings: &ImportedTypeBindings,
    ) -> Result<hir::TypeId, String> {
        self.imported_signature_type_with_bindings(signature, bindings)
            .map_err(|error| {
                format!("cannot resolve dependency signature type {signature:?}: {error:?}")
            })
    }

    pub(super) fn import_generic_definition(
        &mut self,
        declaration: &PreparedImportedCallableSource,
        source: &hir::ExportDefinitionSourceV1,
    ) -> Result<hir::DefinitionOrigin, String> {
        let definition = declaration
            .definition_source(source)
            .ok_or("dependency body is missing its definition source")?;
        self.import_dependency_definition_origin(source, definition)
            .map_err(|error| error.to_string())
    }

    pub(super) fn imported_body_locals(
        &mut self,
        source: &PreparedImportedCallableSource,
        bindings: &ImportedTypeBindings,
    ) -> Result<
        (
            Arena<hir::Local>,
            BTreeMap<scoop_identity::LocalValueSelector, hir::LocalId>,
        ),
        String,
    > {
        let mut locals = Arena::new();
        let mut selectors = BTreeMap::new();
        for local in source.body().locals().records() {
            let ty = self.imported_generic_type(local.value_type(), bindings)?;
            let definition = match local.definition() {
                hir::TemplateLocalDefinitionV1::Source(origin) => {
                    hir::LocalValueDefinitionSite::Source(
                        self.import_generic_definition(source, origin)?,
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
        Ok((locals, selectors))
    }

    pub(super) fn imported_body_predicates(
        &self,
        body: &hir::ExportGenericCallableBodyV1,
        bindings: &ImportedTypeBindings,
    ) -> Result<(Vec<hir::TypeParamId>, Vec<hir::RequiresGcFreePointee>), String> {
        self.imported_template_predicates(body.predicates(), bindings)
    }

    pub(crate) fn imported_template_predicates(
        &self,
        predicates: &hir::GenericTemplatePredicatesV1,
        bindings: &ImportedTypeBindings,
    ) -> Result<(Vec<hir::TypeParamId>, Vec<hir::RequiresGcFreePointee>), String> {
        let predicate = |key: &scoop_identity::SignatureTypeKey| {
            let ty = bindings
                .get(key)
                .ok_or("dependency predicate names an absent binder")?;
            match self.types[*ty] {
                hir::Type::Param(id) => Ok(id),
                _ => Err("dependency predicate is not a parameter"),
            }
        };
        let no_gc = predicates
            .no_gc()
            .arguments()
            .iter()
            .map(predicate)
            .collect::<Result<_, _>>()?;
        let pointees = predicates
            .gc_free_pointees()
            .arguments()
            .iter()
            .map(|key| predicate(key).map(|type_param| hir::RequiresGcFreePointee { type_param }))
            .collect::<Result<_, _>>()?;
        Ok((no_gc, pointees))
    }

    pub(crate) fn imported_generic_nominal_bounds(
        &mut self,
        bounds: &hir::NominalTypeParameterBoundsV1,
        bindings: &ImportedTypeBindings,
        span: hir::Span,
    ) -> Result<hir::TypeParamBounds, String> {
        let class = bounds
            .class()
            .map(|key| {
                let ty = self.imported_generic_type(key, bindings)?;
                Ok::<_, String>(hir::ClassUpperBound { ty, span })
            })
            .transpose()?;
        let interfaces = bounds
            .interfaces()
            .values()
            .iter()
            .map(|key| {
                let ty = self.imported_generic_type(key, bindings)?;
                Ok::<_, String>(hir::InterfaceUpperBound { ty, span })
            })
            .collect::<Result<_, String>>()?;
        Ok(hir::TypeParamBounds::Nominal(hir::NominalBounds {
            class,
            interfaces,
        }))
    }
}
