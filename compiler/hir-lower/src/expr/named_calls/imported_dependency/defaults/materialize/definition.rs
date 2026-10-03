use super::*;
use crate::imported_core::ImportedTypeBindings;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(crate) fn load_default_expression(
        &mut self,
        owner: &dyn hir::ImportedCallableSource,
        template: &hir::ExportDefaultTemplateV1,
    ) -> Result<hir::DefaultExpression, ImportedDefaultMaterializationError> {
        let source = owner
            .definition_source(template.definition_origin())
            .ok_or(ImportedDefinitionOriginError::MissingSource {
                context: template.definition_origin().origin().context(),
            })?;
        let origin =
            self.import_dependency_definition_origin(template.definition_origin(), source)?;
        let (parameters, bindings) = self.default_definition_parameters(template, origin.span)?;
        let type_parameters = parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>();
        let saved_locals = std::mem::take(&mut self.locals);
        let saved_parameters = std::mem::replace(&mut self.type_params_in_scope, parameters);
        let result: Result<_, ImportedDefaultMaterializationError> = (|| {
            let mut context = ImportedDefaultContext {
                owner: ImportedTemplateSource::Default(owner),
                bindings: &bindings,
                locals: BTreeMap::new(),
                local_bindings: BTreeMap::new(),
                lexical_arguments: type_parameters
                    .iter()
                    .map(|parameter| self.intern_type(hir::Type::Param(*parameter)))
                    .collect(),
                captures: &[],
                loop_targets: Vec::new(),
            };
            let mut selectors = BTreeMap::new();
            for local in template.locals().records() {
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
                let binding = self.loaded_default_local_binding(
                    template.definition_root().template_owner(),
                    template.definition_path(),
                    local.selector(),
                    definition,
                );
                let id = self.locals.alloc(hir::Local {
                    binding,
                    selector: local.selector().clone(),
                    definition,
                    name: format!("$default.local.{}", self.locals.len()),
                    ty,
                    mutable: local.mutable().into(),
                });
                selectors.insert(local.selector().clone(), id);
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
            let statements = template
                .body()
                .statements()
                .iter()
                .map(|statement| {
                    self.materialize_imported_default_statement(statement, &mut context)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let value = self
                .materialize_imported_default_expression(template.body().value(), &mut context)?;
            let receiver = template.receiver().receiver().map(|receiver| {
                let local = selectors[receiver.local()];
                hir::ExportDefaultReceiver {
                    local,
                    ty: self.locals[local].ty,
                }
            });
            let value_parameters = template
                .value_parameters()
                .parameters()
                .iter()
                .map(|parameter| hir::ExportDefaultValueParameter {
                    position: parameter.position(),
                    local: selectors[parameter.local()],
                })
                .collect();
            Ok((statements, value, receiver, value_parameters))
        })();
        let locals = std::mem::replace(&mut self.locals, saved_locals);
        self.type_params_in_scope = saved_parameters;
        let (statements, value, receiver, value_parameters) = result?;
        Ok(hir::DefaultExpression {
            body: hir::Body { locals, statements },
            result_type: value.ty,
            value,
            type_parameters,
            receiver,
            value_parameters,
            origin,
        })
    }

    fn default_definition_parameters(
        &mut self,
        template: &hir::ExportDefaultTemplateV1,
        span: Span,
    ) -> Result<(Vec<hir::TypeParamDecl>, ImportedTypeBindings), ImportedDefaultMaterializationError>
    {
        let dependencies = self
            .dependencies
            .as_ref()
            .expect("a loaded default retains its declaration catalog");
        let declaration = dependencies
            .callable_declaration(template.definition_root().declaration())
            .map_err(|error| ImportedDefaultMaterializationError::Plan(error.to_string()))?;
        let nominal = match declaration.interface().owner() {
            hir::PublicDeclarationOwnerV1::Nominal(owner) => {
                dependencies.nominal_declaration(owner).cloned()
            }
            _ => None,
        };
        let host = nominal.as_ref().map_or(&[][..], |nominal| {
            nominal.interface.type_parameters().binders()
        });
        let own = declaration.interface().type_parameters().binders();
        let shape =
            hir::DefaultTemplateProviderShapeV1::try_new(host.len() as u32, own.len() as u32)
                .expect("validated callable binder arities fit the source format");
        let signatures = (0..shape.binder_arity())
            .map(|position| {
                shape
                    .identity_binder_at(position)
                    .expect("a declared binder has its canonical signature position")
            })
            .collect::<Vec<_>>();
        let binders = host.iter().chain(own).collect::<Vec<_>>();
        self.prepare_imported_type_parameters(&binders, &signatures, span)
            .map_err(ImportedDefaultMaterializationError::Plan)
    }
}
