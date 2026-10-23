use super::*;

use crate::call_resolution::candidates::NominalConstructorSource;

mod context;
mod imported;
mod secondary;
mod structs;
mod validation;

pub(crate) use context::ConstructorSource;

impl Lowerer {
    pub(crate) fn struct_primary_constructor(
        &self,
        structure: hir::StructId,
    ) -> Option<hir::StructConstructorId> {
        self.structs[structure]
            .constructors
            .iter()
            .copied()
            .find(|constructor| {
                matches!(
                    self.struct_constructors[*constructor].kind,
                    hir::StructConstructorKind::Primary
                )
            })
    }

    pub(crate) fn resolve_constructor_graphs(
        &mut self,
        classes: &[(ClassId, ast::ClassSource<'_>, usize)],
        structs: &[(hir::StructId, ast::StructSource<'_>, usize)],
        objects: &[(hir::ObjectId, crate::declarations::ObjectSource<'_>, usize)],
    ) {
        self.check_duplicate_constructor_signatures(classes, structs);
        let outer_owner = self.current_owner;
        for &(class, declaration, file) in classes {
            self.current_owner = Some(Owner::Class(class));
            self.current_file = file;
            let diagnostics_before_edges = self.diagnostics.len();
            self.resolve_class_constructor_edges(class, declaration);
            if self.diagnostics.len() == diagnostics_before_edges {
                self.check_class_constructor_cycles(class);
            }
            if self.diagnostics.len() != diagnostics_before_edges {
                self.fail_editing_construction(Owner::Class(class));
            }
        }
        for &(structure, declaration, file) in structs {
            self.current_owner = Some(Owner::Struct(structure));
            self.current_file = file;
            let diagnostics_before_edges = self.diagnostics.len();
            self.resolve_struct_constructor_edges(structure, declaration);
            if self.diagnostics.len() == diagnostics_before_edges {
                self.check_struct_constructor_cycles(structure);
            }
            if self.diagnostics.len() != diagnostics_before_edges {
                self.fail_editing_construction(Owner::Struct(structure));
            }
        }
        for &(object, declaration, file) in objects {
            self.current_owner = Some(Owner::Object(object));
            self.current_file = file;
            let before = self.diagnostics.len();
            self.resolve_object_base(object, declaration);
            if self.diagnostics.len() != before {
                self.fail_editing_construction(Owner::Object(object));
            }
        }
        self.current_owner = outer_owner;
    }

    fn resolve_object_base(
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
        let Some(base_ty) = self.classes[backing].base_class else {
            self.set_primary_base(constructor, hir::BaseInitialization::Root);
            return;
        };
        let specification = source.supertypes().iter().find(|specification| {
            self.resolve_type_ref(&specification.ty)
                .is_some_and(|ty| self.types_equal(ty, base_ty))
        });
        let Some(specification) = specification.cloned() else {
            return;
        };
        let arguments = specification.constructor_arguments().unwrap_or(&[]);
        let unit = self.singleton_values[self.objects[object].singleton_value].initialization;
        let previous_unit = self.current_initialization_unit.replace(unit);
        let Some(base) = self.lower_base_initialization(
            constructor,
            base_ty,
            arguments,
            specification.span,
            "object base constructor delegation",
        ) else {
            self.current_initialization_unit = previous_unit;
            return;
        };
        self.current_initialization_unit = previous_unit;
        self.set_primary_base(constructor, base);
    }

    fn resolve_class_constructor_edges(&mut self, id: ClassId, decl: ast::ClassSource<'_>) {
        if !self.classes[id].is_declared() {
            return;
        }
        let constructors = self.classes[id].constructors.clone();
        let primary = constructors.iter().copied().find(|constructor| {
            matches!(
                self.class_constructors[*constructor].kind,
                hir::ClassConstructorKind::Primary { .. }
            )
        });
        if let Some(primary) = primary {
            self.resolve_primary_base(id, primary, decl);
        } else if let Some(spec) = self.source_base_spec(id, decl)
            && spec.constructor_arguments().is_some()
        {
            self.error(
                spec.span,
                "a class without a primary constructor cannot pass base arguments in its header"
                    .into(),
            );
        }

        let secondary_ids = constructors
            .into_iter()
            .filter(|constructor| {
                matches!(
                    self.class_constructors[*constructor].kind,
                    hir::ClassConstructorKind::Secondary { .. }
                )
            })
            .collect::<Vec<_>>();
        for (constructor, source) in secondary_ids.into_iter().zip(decl.secondary_constructors()) {
            self.resolve_class_secondary_edge(id, primary, constructor, source);
        }
    }

    fn resolve_primary_base(
        &mut self,
        owner: ClassId,
        primary: hir::ClassConstructorId,
        decl: ast::ClassSource<'_>,
    ) {
        let Some(base_ty) = self.classes[owner].base_class else {
            self.set_primary_base(primary, hir::BaseInitialization::Root);
            return;
        };
        let Some(spec) = self.source_base_spec(owner, decl).cloned() else {
            return;
        };
        let arguments = spec.constructor_arguments().unwrap_or(&[]);
        let Some(base) = self.lower_base_initialization(
            primary,
            base_ty,
            arguments,
            spec.span,
            "primary constructor delegation",
        ) else {
            return;
        };
        self.set_primary_base(primary, base);
    }

    pub(crate) fn set_primary_base(
        &mut self,
        constructor: hir::ClassConstructorId,
        base: hir::BaseInitialization,
    ) {
        let hir::ClassConstructorKind::Primary { base: current, .. } =
            &mut self.class_constructors[constructor].kind
        else {
            unreachable!("a primary base belongs to a primary constructor")
        };
        *current = base;
    }

    pub(crate) fn lower_base_initialization(
        &mut self,
        source: hir::ClassConstructorId,
        base_ty: TypeId,
        arguments: &[ast::CallArgument],
        span: ast::Span,
        context: &str,
    ) -> Option<hir::BaseInitialization> {
        if self.dependency_nominal_application(base_ty).is_some() {
            return self
                .lower_imported_base_initialization(source, base_ty, arguments, span, context);
        }
        let Type::Class(base_application) = self.types[base_ty] else {
            unreachable!("a direct base type is a class application")
        };
        let base = self.class_applications[base_application].clone();
        let candidates = self
            .source_class_constructors(self.class_id(base.template))
            .map(NominalConstructorSource::Class)
            .collect::<Vec<_>>();
        let name = self.classes[self.class_id(base.template)].name.clone();
        let type_arguments = base.arguments;
        let explicit_type_arguments = type_arguments
            .iter()
            .skip(self.enclosing_parameter_count(self.class_id(base.template)))
            .copied()
            .map(|ty| crate::expr::ResolvedCallTypeArgument::Explicit { ty, span })
            .collect::<Vec<_>>();
        let resolved = self.with_constructor_source_body(
            source,
            hir::EditingConstructionBody::Delegation(hir::SourceContextConstructor::Class(source)),
            context,
            self.class_constructors[source].safety,
            |this, sink| {
                this.observe_constructor_delegation(
                    hir::SourceContextConstructor::Class(source),
                    span,
                );
                let resolved = this.resolve_nominal_constructor_overload(
                    &name,
                    &candidates,
                    crate::constructor_resolution::NominalConstructorCall {
                        enclosing: None,
                        explicit_type_args: &explicit_type_arguments,
                        expected_type_args: Some(&type_arguments),
                        arguments,
                        span,
                    },
                    sink,
                )?;
                this.observe_completed_delegation(span, base_ty);
                Some(resolved)
            },
        )?;
        let NominalConstructorSource::Class(target) = resolved.value.source else {
            unreachable!("a `super` edge contains class constructor candidates")
        };
        debug_assert_eq!(resolved.value.type_args, type_arguments);
        let target = self.class_constructor_application(target, base_application);
        Some(hir::BaseInitialization::Super {
            target: hir::BaseInitializerTarget::Local(target),
            arguments: hir::ConstructorArguments {
                locals: resolved.locals,
                statements: resolved.statements,
                args: resolved.value.args,
            },
        })
    }

    fn source_base_spec<'a>(
        &mut self,
        owner: ClassId,
        declaration: ast::ClassSource<'a>,
    ) -> Option<&'a ast::SupertypeSpec> {
        let base = self.classes[owner].base_class?;
        let outer = std::mem::replace(
            &mut self.type_params_in_scope,
            self.classes[owner].type_params.clone(),
        );
        let result = declaration.header.supertypes.iter().find(|spec| {
            self.resolve_type_ref(&spec.ty)
                .is_some_and(|ty| self.types_equal(ty, base))
        });
        self.type_params_in_scope = outer;
        result
    }
}
