use super::constructor_work::{ClassConstructorSource, StructConstructorSource};
use super::*;

impl Concretizer<'_> {
    pub(super) fn lower_imported_class_constructor_application(
        &mut self,
        application: export::ImportedConstructorApplicationId,
        substitution: &[concrete::TypeId],
    ) -> concrete::ClassConstructorId {
        let application = self.source.imported_constructor_applications[application].clone();
        let owner = self.lower_type(application.owner, substitution);
        let concrete::TypeKind::Class(class) = self.types[owner].kind else {
            unreachable!("class initialization retains its class application")
        };
        self.request_class_constructor_source(
            ClassConstructorSource::Imported(application.template),
            class,
        )
    }

    pub(super) fn lower_imported_struct_constructor_application(
        &mut self,
        application: export::ImportedConstructorApplicationId,
        substitution: &[concrete::TypeId],
    ) -> concrete::StructConstructorId {
        let application = self.source.imported_constructor_applications[application].clone();
        let owner = self.lower_type(application.owner, substitution);
        let concrete::TypeKind::Struct(structure) = self.types[owner].kind else {
            unreachable!("struct construction retains its struct application")
        };
        self.request_struct_constructor_source(
            StructConstructorSource::Imported(application.template),
            structure,
        )
    }

    fn imported_constructor_parameters(
        &mut self,
        source: &export::ImportedConstructorTemplate,
        substitution: &[concrete::TypeId],
    ) -> Vec<concrete::ConstructorParameter> {
        source
            .parameters
            .iter()
            .map(|parameter| concrete::ConstructorParameter {
                id: concrete::ConstructorParamId::from_raw(parameter.id.into_raw()),
                binding: concrete::BindingId::from_raw(parameter.binding.into_raw()),
                definition: parameter.definition,
                name: parameter.name.clone(),
                ty: self.lower_type(parameter.ty, substitution),
            })
            .collect()
    }

    pub(super) fn lower_imported_struct_constructor(
        &mut self,
        id: export::ImportedConstructorTemplateId,
        structure: concrete::StructId,
        substitution: &[concrete::TypeId],
    ) -> PendingStructConstructor {
        let source = self.source.imported_constructor_templates[id].clone();
        let parameters = self.imported_constructor_parameters(&source, substitution);
        let kind = match &source.kind {
            export::ImportedConstructorKind::StructPrimary => {
                concrete::StructConstructorKind::Primary
            }
            export::ImportedConstructorKind::StructSecondary {
                target,
                arguments,
                body,
                gc_effect,
            } => {
                let target =
                    self.lower_imported_struct_constructor_application(*target, substitution);
                let (arguments, _) = self.lower_constructor_argument_plan(arguments, substitution);
                let (body, _) = self.lower_body(body, substitution);
                concrete::StructConstructorKind::Secondary {
                    target,
                    arguments,
                    body,
                    gc_effect: *gc_effect,
                }
            }
            export::ImportedConstructorKind::ClassTerminal { .. }
            | export::ImportedConstructorKind::ClassThis { .. } => {
                unreachable!("struct constructor requests retain their role")
            }
        };
        PendingStructConstructor {
            structure,
            source_discriminator: id.into_raw().into_u32(),
            safety: match source.effects.safety() {
                export::CallableSafetyV1::Safe => concrete::Safety::Safe,
                export::CallableSafetyV1::Unsafe => concrete::Safety::Unsafe,
            },
            origin: source.origin,
            parameters,
            kind,
        }
    }

    pub(super) fn lower_imported_class_constructor(
        &mut self,
        id: export::ImportedConstructorTemplateId,
        class: concrete::ClassId,
        substitution: &[concrete::TypeId],
    ) -> PendingClassConstructor {
        let source = self.source.imported_constructor_templates[id].clone();
        let parameters = self.imported_constructor_parameters(&source, substitution);
        let mut body = concrete::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        };
        let mut origin = export::ExpressionOrigin::Definition(source.origin).concrete();
        origin.evaluation.context = source.evaluation_context;
        let span = source.origin.span;
        let kind = match &source.kind {
            export::ImportedConstructorKind::ClassTerminal {
                base,
                body: source_body,
            } => {
                self.append_base_initialization(&mut body, base, substitution, span, origin);
                self.append_source_body(&mut body, source_body, substitution);
                concrete::ClassConstructorKind::Terminal { body }
            }
            export::ImportedConstructorKind::ClassThis {
                target,
                arguments,
                body: source_body,
            } => {
                let target =
                    self.lower_imported_class_constructor_application(*target, substitution);
                let args = self.append_constructor_arguments(&mut body, arguments, substitution);
                let receiver = self.constructor_receiver(self.class_type[&class], span, origin);
                let unit = self.lower_type(self.source.unit, &[]);
                body.statements.push(concrete::Statement {
                    span,
                    kind: concrete::StatementKind::Expr(concrete::Expr {
                        kind: concrete::ExprKind::ClassInitializerCall {
                            receiver: Box::new(receiver),
                            initializer: concrete::ClassInitializerTarget::Local(target),
                            args,
                        },
                        ty: unit,
                        span,
                        origin,
                    }),
                });
                self.append_source_body(&mut body, source_body, substitution);
                concrete::ClassConstructorKind::This { target, body }
            }
            export::ImportedConstructorKind::StructPrimary
            | export::ImportedConstructorKind::StructSecondary { .. } => {
                unreachable!("class constructor requests retain their role")
            }
        };
        PendingClassConstructor {
            class,
            source_discriminator: id.into_raw().into_u32(),
            safety: match source.effects.safety() {
                export::CallableSafetyV1::Safe => concrete::Safety::Safe,
                export::CallableSafetyV1::Unsafe => concrete::Safety::Unsafe,
            },
            origin: source.origin,
            parameters,
            kind,
        }
    }
}
