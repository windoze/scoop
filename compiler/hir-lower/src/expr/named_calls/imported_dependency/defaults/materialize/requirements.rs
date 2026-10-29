use super::*;
use hir::{DefaultRequirementBindingValueV1 as Portable, MemberRequirementBindingValue as Binding};

impl Lowerer {
    pub(super) fn materialize_lexical_requirements(
        &mut self,
        source: &[hir::DefaultRequirementBindingV1],
        arguments: &[hir::TypeId],
        context: &ImportedDefaultContext<'_>,
    ) -> Result<Vec<hir::MemberRequirementBinding>, ImportedDefaultMaterializationError> {
        source
            .iter()
            .map(|binding| {
                let requirement = self.materialize_requirement_ref(binding.requirement)?;
                let value = match &binding.value {
                    Portable::Forwarded(reference) => {
                        let id = self.materialize_requirement_ref(*reference)?;
                        let parameters = self.member_requirement_declarations(id);
                        let substitution = parameters
                            .iter()
                            .map(|parameter| parameter.id)
                            .zip(arguments.iter().copied())
                            .collect::<Vec<_>>();
                        let mut contract = self.declared_member_requirement(id).contract;
                        contract.map_types(|ty| self.instantiate_method_ty(ty, &substitution));
                        self.check_member_requirement(id, &contract)
                            .map_err(ImportedDefaultMaterializationError::Plan)?
                    }
                    Portable::Implementation { .. } => {
                        self.materialize_requirement_binding(&binding.value, context)?
                    }
                };
                Ok(hir::MemberRequirementBinding { requirement, value })
            })
            .collect()
    }

    pub(super) fn materialize_requirement_ref(
        &mut self,
        reference: hir::DefaultMemberRequirementRefV1,
    ) -> Result<hir::MemberRequirementId, ImportedDefaultMaterializationError> {
        let origin = match reference.declaration {
            hir::DefaultCallableDeclarationV1::Function(id) => {
                scoop_identity::CallableTemplateOrigin::Function(id)
            }
            hir::DefaultCallableDeclarationV1::GenericFunction(id) => {
                scoop_identity::CallableTemplateOrigin::GenericFunction(id)
            }
            hir::DefaultCallableDeclarationV1::PropertyAccessor(id) => {
                scoop_identity::CallableTemplateOrigin::Accessor(id)
            }
            hir::DefaultCallableDeclarationV1::Generated(_) => {
                return Err(ImportedDefaultMaterializationError::Plan(
                    "requirements belong to original source callables".into(),
                ));
            }
        };
        let template = if let Some(template) = self
            .request_imported_local_function(origin)
            .map_err(ImportedDefaultMaterializationError::Plan)?
        {
            template
        } else {
            let declaration = self
                .dependencies
                .as_ref()
                .and_then(|dependencies| dependencies.callable_declaration(origin).ok())
                .ok_or(ImportedDefaultMaterializationError::MissingCallable(origin))?;
            self.request_imported_generic_template(declaration)
                .map_err(ImportedDefaultMaterializationError::Plan)?
        };
        let parameters = self.imported_generic_templates[template]
            .type_parameters
            .declarations()
            .to_vec();
        self.record_member_requirement_owner(
            hir::MemberRequirementOwner::Imported(template),
            &parameters,
        );
        parameters
            .get(reference.parameter as usize)
            .and_then(|parameter| parameter.requirements.get(reference.requirement as usize))
            .map(|requirement| requirement.id)
            .ok_or_else(|| {
                ImportedDefaultMaterializationError::Plan(
                    "member requirement reference is outside its original declaration".into(),
                )
            })
    }

    pub(super) fn materialize_requirement_binding(
        &mut self,
        source: &Portable,
        outer: &ImportedDefaultContext<'_>,
    ) -> Result<Binding, ImportedDefaultMaterializationError> {
        let Portable::Implementation {
            body,
            inputs,
            type_arguments,
        } = source
        else {
            let Portable::Forwarded(reference) = source else {
                unreachable!()
            };
            return self
                .materialize_requirement_ref(*reference)
                .map(Binding::Forwarded);
        };
        let arguments = type_arguments
            .iter()
            .map(|ty| self.materialize_imported_default_type(ty, outer))
            .collect::<Result<Vec<_>, _>>()?;
        let mut bindings = crate::imported_core::ImportedTypeBindings::new();
        let mut lexical_arguments = Vec::new();
        for index in 0..arguments.len() {
            let parameter = self.fresh_type_param(index);
            let ty = self.intern_type(hir::Type::Param(parameter));
            bindings.insert(
                scoop_identity::SignatureTypeKey::Binder {
                    depth: 0,
                    index: index as u32,
                },
                ty,
            );
            lexical_arguments.push(ty);
        }
        let saved = std::mem::take(&mut self.locals);
        let result: Result<_, ImportedDefaultMaterializationError> = (|| {
            let mut context = ImportedDefaultContext {
                owner: outer.owner,
                bindings: &bindings,
                locals: BTreeMap::new(),
                local_bindings: BTreeMap::new(),
                lexical_arguments,
                captures: &[],
                loop_targets: Vec::new(),
            };
            let result = body.results().first().ok_or_else(|| {
                ImportedDefaultMaterializationError::Plan(
                    "member implementation has no result".into(),
                )
            })?;
            let origin = hir::ExpressionOrigin::Definition(
                self.imported_default_definition_origin(result.definition_origin(), &context)?,
            );
            let mut local_ids = Vec::new();
            for record in body.locals().records() {
                let ty = self.materialize_imported_default_type(record.value_type(), &context)?;
                let definition = match record.definition() {
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
                    selector: record.selector().clone(),
                    definition,
                    name: record.name().to_owned(),
                    ty,
                    mutable: record.mutable().into(),
                });
                local_ids.push(id);
                context
                    .local_bindings
                    .insert(record.selector().clone(), binding);
                context.locals.insert(
                    record.selector().clone(),
                    hir::Expr {
                        kind: hir::ExprKind::Local(id),
                        ty,
                        span: origin.definition().span,
                        origin,
                    },
                );
            }
            let inputs = inputs
                .iter()
                .map(|&index| {
                    local_ids.get(index as usize).copied().ok_or_else(|| {
                        ImportedDefaultMaterializationError::Plan(
                            "member implementation input is out of range".into(),
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let statements =
                self.materialize_imported_default_statements(body.statements(), &mut context)?;
            let result = self.materialize_imported_default_expression(result, &mut context)?;
            Ok((inputs, statements, result))
        })();
        let locals = std::mem::replace(&mut self.locals, saved).into_arena();
        let (inputs, statements, result) = result?;
        let implementation =
            self.member_requirement_implementations
                .alloc(hir::MemberRequirementImplementation {
                    inputs,
                    body: hir::Body { locals, statements },
                    result,
                });
        Ok(Binding::Implementation {
            implementation,
            type_arguments: arguments,
        })
    }
}
