use super::*;
use crate::imported_constructors::{PreparedImportedConstructor, source_constructor};

impl Lowerer {
    pub(crate) fn materialize_imported_constructor(
        &mut self,
        template: &PreparedImportedConstructor,
    ) -> Result<hir::ImportedConstructorKind, ImportedDefaultMaterializationError> {
        use hir::ExportConstructorInitializationKindV1 as Source;
        use hir::ImportedConstructorKind as Target;
        let executable = &template.initialization.constructors()[template.constructor];
        match executable.kind() {
            Source::StructPrimary => Ok(Target::StructPrimary),
            Source::StructSecondary { delegation, body } => {
                let hir::ConstructorApplicationRef::Struct(target) =
                    self.imported_constructor_application(&delegation.target, &template.bindings)?
                else {
                    unreachable!("validated struct delegation retains its role")
                };
                let arguments =
                    self.imported_constructor_fragment(template, &delegation.arguments)?;
                let body = self.imported_constructor_body(template, body)?;
                Ok(Target::StructSecondary {
                    target,
                    arguments,
                    body,
                    gc_effect: match executable.effects().gc_effect() {
                        scoop_identity::GcEffect::NoGc => hir::GcEffect::NoGc,
                        scoop_identity::GcEffect::Managed => hir::GcEffect::Managed,
                    },
                })
            }
            Source::ClassSecondaryThis { delegation, body } => {
                let hir::ConstructorApplicationRef::Class(target) =
                    self.imported_constructor_application(&delegation.target, &template.bindings)?
                else {
                    unreachable!("validated class delegation retains its role")
                };
                let arguments =
                    self.imported_constructor_fragment(template, &delegation.arguments)?;
                let body = self.imported_constructor_body(template, body)?;
                Ok(Target::ClassThis {
                    target,
                    arguments,
                    body,
                })
            }
            Source::ClassPrimary {
                base,
                primary_stores,
            } => {
                let base = self.imported_constructor_base(template, base.as_ref())?;
                let saved_locals = std::mem::take(&mut self.locals);
                let mut statements = Vec::new();
                let result = (|| {
                    for store in primary_stores {
                        let LocalValueSelector::Parameter { declaration_index } = store.parameter
                        else {
                            unreachable!("validated primary stores name constructor parameters")
                        };
                        let parameter = &template.signature.parameters[declaration_index as usize];
                        let value = hir::Expr {
                            kind: hir::ExprKind::ConstructorParam(parameter.id),
                            ty: parameter.ty,
                            span: parameter.definition.span,
                            origin: template.expression_origin(parameter.definition),
                        };
                        statements.push(self.imported_constructor_store(
                            template,
                            &store.field,
                            value,
                        )?);
                    }
                    self.append_imported_common_initialization(template, &mut statements)
                })();
                let locals = std::mem::replace(&mut self.locals, saved_locals);
                result?;
                Ok(Target::ClassTerminal {
                    base,
                    body: hir::Body { locals, statements },
                })
            }
            Source::ClassSecondaryTerminal { base, body } => {
                let base = self.imported_constructor_base(template, base.as_ref())?;
                let saved_locals = std::mem::take(&mut self.locals);
                let mut statements = Vec::new();
                let result = (|| {
                    self.append_imported_common_initialization(template, &mut statements)?;
                    self.append_imported_constructor_fragment(template, body, &mut statements)?;
                    Ok::<_, ImportedDefaultMaterializationError>(())
                })();
                let locals = std::mem::replace(&mut self.locals, saved_locals);
                result?;
                Ok(Target::ClassTerminal {
                    base,
                    body: hir::Body { locals, statements },
                })
            }
        }
    }

    fn imported_constructor_body(
        &mut self,
        template: &PreparedImportedConstructor,
        fragment: &hir::ExportTemplateFragmentV1,
    ) -> Result<hir::Body, ImportedDefaultMaterializationError> {
        let plan = self.imported_constructor_fragment(template, fragment)?;
        debug_assert!(plan.args.is_empty());
        Ok(hir::Body {
            locals: plan.locals,
            statements: plan.statements,
        })
    }

    fn imported_constructor_fragment(
        &mut self,
        template: &PreparedImportedConstructor,
        fragment: &hir::ExportTemplateFragmentV1,
    ) -> Result<hir::ConstructorArguments, ImportedDefaultMaterializationError> {
        let saved_locals = std::mem::take(&mut self.locals);
        let mut statements = Vec::new();
        let args = self.append_imported_constructor_fragment(template, fragment, &mut statements);
        let locals = std::mem::replace(&mut self.locals, saved_locals);
        Ok(hir::ConstructorArguments {
            locals,
            statements,
            args: args?,
        })
    }

    fn append_imported_constructor_fragment(
        &mut self,
        template: &PreparedImportedConstructor,
        fragment: &hir::ExportTemplateFragmentV1,
        statements: &mut Vec<hir::Statement>,
    ) -> Result<Vec<hir::Expr>, ImportedDefaultMaterializationError> {
        let mut context = ImportedDefaultContext {
            owner: ImportedTemplateSource::Default(&template.source),
            bindings: &template.bindings,
            locals: BTreeMap::new(),
            local_bindings: BTreeMap::new(),
            lexical_arguments: template
                .signature
                .type_parameters
                .iter()
                .map(|parameter| self.intern_type(hir::Type::Param(parameter.id)))
                .collect(),
            captures: &[],
            loop_targets: Vec::new(),
            parent: scoop_identity::CallableTemplateOwner::Constructor(
                template.signature.declaration,
            ),
        };
        for local in fragment.locals().records() {
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
            let kind = match local.selector() {
                LocalValueSelector::This => hir::ExprKind::ConstructorReceiver,
                LocalValueSelector::Parameter { declaration_index } => {
                    context.local_bindings.insert(
                        local.selector().clone(),
                        template.signature.parameters[*declaration_index as usize].binding,
                    );
                    hir::ExprKind::ConstructorParam(
                        template.signature.parameters[*declaration_index as usize].id,
                    )
                }
                _ => {
                    let binding = self.fresh_binding();
                    context
                        .local_bindings
                        .insert(local.selector().clone(), binding);
                    let id = self.locals.alloc(hir::Local {
                        binding,
                        selector: local.selector().clone(),
                        definition,
                        name: format!("$dependency.constructor.local.{}", self.locals.len()),
                        ty,
                        mutable: local.mutable().into(),
                    });
                    hir::ExprKind::Local(id)
                }
            };
            context.locals.insert(
                local.selector().clone(),
                hir::Expr {
                    kind,
                    ty,
                    span: template.signature.origin.span,
                    origin: template.expression_origin(template.signature.origin),
                },
            );
        }
        for statement in fragment.statements() {
            statements.push(self.materialize_imported_default_statement(statement, &mut context)?);
        }
        self.materialize_imported_default_expressions(fragment.results(), &mut context)
    }

    fn append_imported_common_initialization(
        &mut self,
        template: &PreparedImportedConstructor,
        statements: &mut Vec<hir::Statement>,
    ) -> Result<(), ImportedDefaultMaterializationError> {
        for step in template.initialization.common() {
            match step {
                hir::ExportCommonInitializationStepV1::Field { field, value } => {
                    let mut values =
                        self.append_imported_constructor_fragment(template, value, statements)?;
                    let value = values
                        .pop()
                        .expect("validated field initialization has one result");
                    statements.push(self.imported_constructor_store(template, field, value)?);
                }
                hir::ExportCommonInitializationStepV1::Body(body) => {
                    self.append_imported_constructor_fragment(template, body, statements)?;
                }
            }
        }
        Ok(())
    }

    fn imported_constructor_store(
        &mut self,
        template: &PreparedImportedConstructor,
        field: &hir::DefaultFieldRefV1,
        value: hir::Expr,
    ) -> Result<hir::Statement, ImportedDefaultMaterializationError> {
        let field = self.materialize_imported_field_ref(field, &template.bindings)?;
        let origin = template.signature.origin;
        let receiver = hir::Expr {
            kind: hir::ExprKind::ConstructorReceiver,
            ty: template.signature.owner,
            span: origin.span,
            origin: template.expression_origin(origin),
        };
        Ok(hir::Statement {
            span: value.span,
            kind: hir::StatementKind::Assign {
                target: hir::AssignTarget::Field {
                    receiver: Box::new(receiver),
                    field,
                },
                value,
            },
        })
    }

    fn imported_constructor_base(
        &mut self,
        template: &PreparedImportedConstructor,
        base: Option<&hir::ExportConstructorDelegationV1>,
    ) -> Result<hir::BaseInitialization, ImportedDefaultMaterializationError> {
        let Some(base) = base else {
            return Ok(hir::BaseInitialization::Root);
        };
        let owner = self
            .imported_generic_type(base.target.owner_type(), &template.bindings)
            .map_err(ImportedDefaultMaterializationError::Plan)?;
        let target = if matches!(
            self.imported_nominal_owner(owner),
            Some(hir::SourceNominalId::GenericTemplate(_))
        ) {
            let hir::ConstructorApplicationRef::Class(target) =
                self.imported_constructor_application(&base.target, &template.bindings)?
            else {
                unreachable!("validated base initialization retains its class role")
            };
            hir::BaseInitializerTarget::Local(target)
        } else {
            let source = source_constructor(&base.target)
                .expect("source base delegation names a source constructor");
            let declaration = self
                .dependencies
                .as_ref()
                .expect("constructor has a dependency catalog")
                .callable_declaration(scoop_identity::CallableTemplateOrigin::Constructor(source))
                .map_err(|e| {
                    ImportedDefaultMaterializationError::DependencySelection(e.to_string())
                })?;
            let callable = self
                .select_imported_callable_declaration_use_with_kind(
                    declaration,
                    MemberCallKind::Ordinary,
                )
                .map_err(|e| {
                    ImportedDefaultMaterializationError::DependencySelection(e.to_string())
                })?;
            hir::BaseInitializerTarget::Imported { owner, callable }
        };
        let arguments = self.imported_constructor_fragment(template, &base.arguments)?;
        Ok(hir::BaseInitialization::Super { target, arguments })
    }

    pub(crate) fn imported_constructor_application(
        &mut self,
        source: &hir::DefaultConstructorRefV1,
        bindings: &crate::imported_core::ImportedTypeBindings,
    ) -> Result<hir::ConstructorApplicationRef, ImportedDefaultMaterializationError> {
        let owner = self
            .imported_generic_type(source.owner_type(), bindings)
            .map_err(ImportedDefaultMaterializationError::Plan)?;
        let declaration = source_constructor(source)
            .expect("source construction retains its constructor identity");
        let source = self
            .dependencies
            .as_ref()
            .expect("constructor has a dependency catalog")
            .callable_declaration(scoop_identity::CallableTemplateOrigin::Constructor(
                declaration,
            ))
            .map_err(|e| ImportedDefaultMaterializationError::DependencySelection(e.to_string()))?;
        let template = self
            .request_imported_constructor_template(source)
            .map_err(ImportedDefaultMaterializationError::Plan)?;
        Ok(self.constructor_template_application(template, owner))
    }

    pub(super) fn materialize_imported_field_ref(
        &mut self,
        field: &hir::DefaultFieldRefV1,
        bindings: &crate::imported_core::ImportedTypeBindings,
    ) -> Result<hir::FieldRef, ImportedDefaultMaterializationError> {
        match field {
            hir::DefaultFieldRefV1::Tuple { declaration_index } => {
                Ok(hir::FieldRef::TupleIndex(*declaration_index))
            }
            hir::DefaultFieldRefV1::Struct {
                declaration,
                owner_type,
            }
            | hir::DefaultFieldRefV1::Class {
                declaration,
                owner_type,
            } => {
                let owner = self
                    .imported_generic_type(owner_type, bindings)
                    .map_err(ImportedDefaultMaterializationError::Plan)?;
                Ok(if matches!(field, hir::DefaultFieldRefV1::Class { .. }) {
                    hir::FieldRef::ClassField {
                        owner,
                        field: *declaration,
                    }
                } else {
                    hir::FieldRef::StructField {
                        owner,
                        field: *declaration,
                    }
                })
            }
        }
    }
}
