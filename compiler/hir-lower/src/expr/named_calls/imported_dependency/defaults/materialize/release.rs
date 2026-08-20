use super::*;

impl Lowerer {
    pub(crate) fn load_imported_release_hook(
        &mut self,
        declaration: &hir::ImportedNominalDeclaration,
        bindings: &crate::imported_core::ImportedTypeBindings,
    ) -> Result<hir::ExportReleaseHook, ImportedDefaultMaterializationError> {
        let owner = declaration.owner();
        let hir::SourceNominalId::GenericTemplate(generic) = owner else {
            return Err(ImportedDefaultMaterializationError::Plan(
                "parameter-free release hooks remain in their provider".into(),
            ));
        };
        let source = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.nominal_initialization(generic))
            .ok_or_else(|| {
                ImportedDefaultMaterializationError::Plan(
                    "generic release owner has no execution template".into(),
                )
            })?;
        let hook = source
            .initialization()
            .release_policy()
            .hook()
            .ok_or_else(|| {
                ImportedDefaultMaterializationError::Plan(
                    "generic release owner has no release body".into(),
                )
            })?;
        let location = source.definition_source(hook.definition_origin()).ok_or(
            ImportedDefinitionOriginError::MissingSource {
                context: hook.definition_origin().origin().context(),
            },
        )?;
        let origin =
            self.import_dependency_definition_origin(hook.definition_origin(), location)?;
        let parameters = self.loaded_class_definitions[&owner]
            .definition
            .type_params
            .clone();
        let requirements = match declaration.interface.declaration_details().release_policy() {
            hir::NominalReleasePolicyV1::SynchronousGcFree { requirements } => requirements
                .iter()
                .map(|binder| parameters[binder.index as usize].id)
                .collect(),
            hir::NominalReleasePolicyV1::None => {
                return Err(ImportedDefaultMaterializationError::Plan(
                    "release body has no declaration policy".into(),
                ));
            }
        };
        let saved_locals = std::mem::take(&mut self.locals);
        let saved_parameters =
            std::mem::replace(&mut self.type_params_in_scope, parameters.clone());
        let result = (|| {
            let mut context = ImportedDefaultContext {
                owner: ImportedTemplateSource::Nominal(&source),
                bindings,
                locals: BTreeMap::new(),
                local_bindings: BTreeMap::new(),
                lexical_arguments: parameters
                    .iter()
                    .map(|parameter| self.intern_type(hir::Type::Param(parameter.id)))
                    .collect(),
                captures: &[],
                loop_targets: Vec::new(),
            };
            for local in hook.body().locals().records() {
                let ty = self.materialize_imported_default_type(local.value_type(), &context)?;
                let definition = match local.definition() {
                    hir::TemplateLocalDefinitionV1::Source(source) => {
                        hir::LocalValueDefinitionSite::Source(
                            self.imported_default_definition_origin(source, &context)?,
                        )
                    }
                    hir::TemplateLocalDefinitionV1::Synthetic => {
                        hir::LocalValueDefinitionSite::Synthetic
                    }
                };
                let binding = self.fresh_binding();
                let id = self.locals.alloc(hir::Local {
                    binding,
                    selector: local.selector().clone(),
                    definition,
                    name: format!("$dependency.release.local.{}", self.locals.len()),
                    ty,
                    mutable: local.mutable().into(),
                });
                context
                    .local_bindings
                    .insert(local.selector().clone(), binding);
                context.locals.insert(
                    local.selector().clone(),
                    hir::Expr {
                        kind: hir::ExprKind::Local(id),
                        ty,
                        span: origin.span,
                        origin: hir::ExpressionOrigin::Definition(origin),
                    },
                );
            }
            hook.body()
                .statements()
                .iter()
                .map(|statement| {
                    self.materialize_imported_default_statement(statement, &mut context)
                })
                .collect::<Result<Vec<_>, _>>()
        })();
        let locals = std::mem::replace(&mut self.locals, saved_locals).into_arena();
        self.type_params_in_scope = saved_parameters;
        Ok(hir::ExportReleaseHook {
            owner,
            requirements,
            body: hir::Body {
                locals,
                statements: result?,
            },
            origin,
        })
    }
}
