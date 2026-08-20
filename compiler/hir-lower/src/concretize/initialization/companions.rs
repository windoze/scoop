use super::*;
use scoop_identity::InitializationCallableRole;

impl Concretizer<'_> {
    pub(in crate::concretize) fn lower_companion_function(
        &mut self,
        key: &FunctionKey,
        template: export::ImportedCompanionTemplateId,
        role: InitializationCallableRole,
    ) -> crate::concretize::functions::PendingFunction {
        let template = &self.source.imported_companion_templates[template];
        let signature = &template.signature;
        let kind = match role {
            InitializationCallableRole::Ensure => concrete::FunctionKind::InitializationEnsure,
            InitializationCallableRole::Initializer => {
                let owner = self.source.imported_constructor_templates[template.constructor].owner;
                let ty = self.lower_type(owner, &key.arguments);
                let concrete::TypeKind::Class(class) = self.types[ty].kind else {
                    unreachable!("a companion initializer has a class representation")
                };
                let constructor = self.request_class_constructor_source(
                    export::ClassConstructorDefinition::Template(template.constructor),
                    class,
                );
                let value = self.singleton_value_for_type(ty);
                concrete::FunctionKind::User(concrete::Body {
                    locals: Arena::new(),
                    statements: vec![concrete::Statement {
                        span: signature.span,
                        kind: concrete::StatementKind::Assign {
                            target: concrete::AssignTarget::SingletonPublishedRoot(
                                self.singleton_root_map[&value],
                            ),
                            value: concrete::Expr {
                                kind: concrete::ExprKind::ClassNew {
                                    constructor,
                                    args: Vec::new(),
                                },
                                ty,
                                span: signature.span,
                                origin: export::ExpressionOrigin::Definition(template.origin)
                                    .concrete(),
                            },
                        },
                    }],
                })
            }
        };
        crate::concretize::functions::PendingFunction {
            name: format!("{}${role:?}", signature.name),
            is_suspend: false,
            modifiers: signature.modifiers,
            params: Vec::new(),
            capture_parameters: Vec::new(),
            return_ty: self.lower_type(self.source.unit, &[]),
            attributes: signature.attributes,
            kind,
            receiver: concrete::FunctionReceiver::None,
            span: signature.span,
        }
    }

    pub(super) fn finish_companion_initialization(
        &self,
        request: &InitializationRequest,
        template_id: export::ImportedCompanionTemplateId,
        identity: concrete::InitializationUnitIdentityRecord,
        cycle_thrower: concrete::InitializationCycleThrower,
    ) -> concrete::InitializationUnit {
        let template = &self.source.imported_companion_templates[template_id];
        let class =
            self.class_by_key[&(template.declaration.owner(), request.key.arguments.clone())];
        let value = self.singleton_value_for_type(self.class_type[&class]);
        let function = |role| {
            self.function_by_key[&self.function_key(
                FunctionSource::Companion(template_id, role),
                None,
                request.key.arguments.clone(),
            )]
        };
        concrete::InitializationUnit {
            identity,
            display_name: template.display_name.clone(),
            schedule: concrete::InitializationSchedule::LazyAccess,
            kind: concrete::InitializationUnitKind::LazySingleton {
                value,
                published_root: self.singleton_root_map[&value],
            },
            initializer: function(InitializationCallableRole::Initializer),
            ensure: function(InitializationCallableRole::Ensure),
            failure_root: request.failure_root,
            dependencies: request
                .dependencies
                .iter()
                .map(|unit| concrete::InitializationDependency { unit: *unit })
                .collect(),
            cycle_thrower,
        }
    }
}
