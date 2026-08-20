use super::*;

use crate::InitializingReceiver;

mod fields;
mod objects;
mod properties;

pub(crate) struct InitializingField {
    pub(crate) read: hir::Expr,
    pub(crate) write: Option<hir::AssignTarget>,
}

impl Lowerer {
    pub(crate) fn lower_constructor_initialization(
        &mut self,
        classes: &[(ClassId, &ast::ClassDecl, usize)],
        structs: &[(hir::StructId, &ast::StructDecl, usize)],
        objects: &[(hir::ObjectId, crate::declarations::ObjectSource<'_>, usize)],
    ) {
        for &(class, declaration, file) in classes {
            self.current_file = file;
            self.lower_class_initialization(class, declaration);
        }
        for &(structure, declaration, file) in structs {
            self.current_file = file;
            self.lower_struct_constructor_bodies(structure, declaration);
        }
        for &(object, declaration, file) in objects {
            self.current_file = file;
            self.lower_object_initialization(object, declaration);
        }
    }

    fn lower_class_initialization(&mut self, owner: ClassId, declaration: &ast::ClassDecl) {
        if !self.classes[owner].is_declared() {
            return;
        }
        let constructors = self.classes[owner].constructors.clone();
        let primary = constructors.iter().copied().find(|constructor| {
            matches!(
                self.class_constructors[*constructor].kind,
                hir::ClassConstructorKind::Primary { .. }
            )
        });
        let context_constructor = primary.or_else(|| constructors.first().copied());
        let Some(context_constructor) = context_constructor else {
            return;
        };

        let application = self.classes[owner].self_application;
        let mut initialized = self.inherited_fields(owner);
        initialized.extend(self.enclosing_class_field(owner));
        let mut stores = Vec::new();
        if primary.is_some() {
            for field in self.classes[owner].fields.clone() {
                let declaration = &self.class_fields[field];
                if let hir::ClassFieldSource::PrimaryParameter { parameter, .. } =
                    declaration.source
                {
                    let span = declaration.span;
                    stores.push(hir::PrimaryFieldStore {
                        field: self.initializing_class_field_reference(application, field),
                        parameter,
                        span,
                    });
                    initialized.insert(field);
                }
            }
        }
        let initial_context = crate::InitializationContext {
            receiver: InitializingReceiver::Class {
                application,
                initialized,
            },
            step: format!("initialization of class `{}`", self.classes[owner].name),
            capture_depth: self.capture_contexts.len(),
        };
        self.initialization_context = Some(initial_context.clone());

        let common = self.lower_class_common_initialization(
            owner,
            declaration,
            context_constructor,
            primary.is_some(),
        );

        if let Some(primary) = primary {
            let hir::ClassConstructorKind::Primary {
                primary_stores,
                common_initialization,
                ..
            } = &mut self.class_constructors[primary].kind
            else {
                unreachable!("the selected constructor is primary")
            };
            *primary_stores = stores;
            *common_initialization = common.clone();
        }
        for &constructor in &constructors {
            if !matches!(
                self.class_constructors[constructor].kind,
                hir::ClassConstructorKind::Secondary {
                    delegation: hir::ClassSecondaryDelegation::Terminal { .. },
                    ..
                }
            ) {
                continue;
            }
            let common = if self.enclosing_class_field(owner).is_some()
                && constructor != context_constructor
            {
                self.initialization_context = Some(initial_context.clone());
                self.lower_class_common_initialization(owner, declaration, constructor, false)
            } else {
                common.clone()
            };
            let hir::ClassConstructorKind::Secondary { delegation, .. } =
                &mut self.class_constructors[constructor].kind
            else {
                continue;
            };
            if let hir::ClassSecondaryDelegation::Terminal {
                common_initialization,
                ..
            } = delegation
            {
                *common_initialization = common;
            }
        }

        let all_initialized = self.all_class_fields(owner);
        let secondary = constructors
            .iter()
            .copied()
            .filter(|constructor| {
                matches!(
                    self.class_constructors[*constructor].kind,
                    hir::ClassConstructorKind::Secondary { .. }
                )
            })
            .collect::<Vec<_>>();
        for (constructor, source) in secondary
            .into_iter()
            .zip(declaration.secondary_constructors())
        {
            self.initialization_context = Some(crate::InitializationContext {
                receiver: InitializingReceiver::Class {
                    application,
                    initialized: all_initialized.clone(),
                },
                step: format!(
                    "secondary constructor body `{}`",
                    self.constructor_display_name(constructor)
                ),
                capture_depth: self.capture_contexts.len(),
            });
            let lowered = self.with_constructor_expression_context(
                constructor,
                "secondary constructor body",
                self.class_constructors[constructor].safety,
                |this, _| Some(this.lower_block(&source.body)),
            );
            if let Some(lowered) = lowered {
                let hir::ClassConstructorKind::Secondary { body, .. } =
                    &mut self.class_constructors[constructor].kind
                else {
                    unreachable!("the selected constructor is secondary")
                };
                *body = hir::Body {
                    locals: lowered.locals,
                    statements: lowered
                        .statements
                        .into_iter()
                        .chain(lowered.value)
                        .collect(),
                };
            }
        }
        self.initialization_context = None;
    }

    fn lower_class_common_initialization(
        &mut self,
        owner: ClassId,
        declaration: &ast::ClassDecl,
        constructor: hir::ClassConstructorId,
        primary: bool,
    ) -> Vec<hir::ClassInitializationStep> {
        let mut common = self.lower_interface_delegate_initialization(owner, constructor, primary);
        common.extend(self.lower_common_initialization(
            owner,
            &declaration.members,
            constructor,
            primary,
            &format!("class `{}`", self.classes[owner].name),
        ));
        common
    }

    fn lower_struct_constructor_bodies(
        &mut self,
        owner: hir::StructId,
        declaration: &ast::StructDecl,
    ) {
        if !matches!(
            self.structs[owner].representation,
            hir::StructRepresentation::Declared(_)
        ) {
            return;
        }
        let application = self.structs[owner].self_application;
        let secondary = self.structs[owner]
            .constructors
            .iter()
            .copied()
            .filter(|constructor| {
                matches!(
                    self.struct_constructors[*constructor].kind,
                    hir::StructConstructorKind::Secondary { .. }
                )
            })
            .collect::<Vec<_>>();
        for (constructor, source) in secondary
            .into_iter()
            .zip(declaration.secondary_constructors())
        {
            self.initialization_context = Some(crate::InitializationContext {
                receiver: InitializingReceiver::Struct { application },
                step: format!(
                    "secondary constructor body of `{}`",
                    self.structs[owner].name
                ),
                capture_depth: self.capture_contexts.len(),
            });
            let lowered = self.with_constructor_expression_context(
                constructor,
                "struct secondary constructor body",
                self.struct_constructors[constructor].safety,
                |this, _| Some(this.lower_block(&source.body)),
            );
            if let Some(lowered) = lowered {
                let hir::StructConstructorKind::Secondary { body, .. } =
                    &mut self.struct_constructors[constructor].kind
                else {
                    unreachable!("the selected constructor is secondary")
                };
                *body = hir::Body {
                    locals: lowered.locals,
                    statements: lowered
                        .statements
                        .into_iter()
                        .chain(lowered.value)
                        .collect(),
                };
            }
        }
        self.initialization_context = None;
    }

    pub(crate) fn inherited_fields(
        &self,
        owner: ClassId,
    ) -> std::collections::HashSet<hir::ClassFieldId> {
        let mut fields = std::collections::HashSet::new();
        let mut current = self.direct_base_class(owner);
        let mut seen = std::collections::HashSet::new();
        while let Some(class) = current {
            if !seen.insert(class) {
                break;
            }
            fields.extend(self.classes[class].fields.iter().copied());
            current = self.direct_base_class(class);
        }
        fields
    }

    fn all_class_fields(&self, owner: ClassId) -> std::collections::HashSet<hir::ClassFieldId> {
        let mut fields = self.inherited_fields(owner);
        fields.extend(self.classes[owner].fields.iter().copied());
        fields
    }

    fn constructor_display_name(&mut self, constructor: hir::ClassConstructorId) -> String {
        let declaration = self.class_constructors[constructor].clone();
        let owner = self.classes[declaration.owner].name.clone();
        let parameters = declaration
            .parameters
            .iter()
            .map(|parameter| self.type_name(parameter.ty))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{owner}({parameters})")
    }
}
