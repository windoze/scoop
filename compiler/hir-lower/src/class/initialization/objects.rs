use super::*;

impl Lowerer {
    pub(super) fn lower_object_initialization(
        &mut self,
        object: hir::ObjectId,
        source: crate::declarations::ObjectSource<'_>,
    ) {
        let backing = self.objects[object].backing_class;
        let constructor = self.classes[backing]
            .constructors
            .first()
            .copied()
            .expect("every object has one hidden primary constructor");
        let application = self.classes[backing].self_application;
        self.initialization_context = Some(crate::InitializationContext {
            receiver: InitializingReceiver::Class {
                application,
                initialized: self.inherited_fields(backing),
            },
            step: format!("initialization of object `{}`", self.objects[object].name),
            capture_depth: self.capture_contexts.len(),
        });
        let unit = self.singleton_values[self.objects[object].singleton_value].initialization;
        let previous_unit = self.current_initialization_unit.replace(unit);
        let common = self.lower_common_initialization(
            backing,
            source.members(),
            constructor,
            false,
            &format!("object `{}`", self.objects[object].name),
        );
        self.current_initialization_unit = previous_unit;
        let hir::ClassConstructorKind::Primary {
            common_initialization,
            ..
        } = &mut self.class_constructors[constructor].kind
        else {
            unreachable!("the hidden object constructor is primary")
        };
        *common_initialization = common;
        self.initialization_context = None;

        let object_declaration = self.objects[object].clone();
        let singleton = self.singleton_values[object_declaration.singleton_value];
        let constructor_application = self.class_constructor_application(constructor, application);
        let initializer = self.initialization_units[singleton.initialization].initializer;
        let previous_context = self.current_source_context;
        self.set_source_context(hir::SourceContextSubject::Function(initializer));
        let origin = self.expression_origin(source.span());
        self.current_source_context = previous_context;
        self.functions[initializer].kind = hir::FunctionKind::User(hir::Body {
            locals: la_arena::Arena::new(),
            statements: vec![hir::Statement {
                kind: hir::StatementKind::Assign {
                    target: hir::AssignTarget::SingletonPublishedRoot(singleton.published_root),
                    value: hir::Expr {
                        kind: hir::ExprKind::ClassInit {
                            constructor: constructor_application,
                            args: Vec::new(),
                        },
                        ty: self.object_types[object_declaration.object_type].canonical_type,
                        span: source.span(),
                        origin,
                    },
                },
                span: source.span(),
            }],
        });
    }
}
