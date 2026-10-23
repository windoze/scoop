use super::*;

impl Lowerer {
    pub(crate) fn declare_class<'a>(
        &mut self,
        decl: ast::ClassSource<'a>,
        is_core: bool,
        pending: &mut Vec<(ClassId, ast::ClassSource<'a>, usize)>,
        pending_methods: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
        file_index: usize,
        owner: Option<Owner>,
    ) -> Option<ClassId> {
        let diagnostics_before = self.diagnostics.len();
        let enclosing = if let Some(span) = decl.inner_span {
            let Some(Owner::Class(outer)) = owner else {
                self.error(
                    span,
                    "`inner class` must be declared directly in a class body".into(),
                );
                return None;
            };
            Some(outer)
        } else {
            None
        };
        let mut checked = self.check_class_annotations(decl);
        if owner.is_some() && checked.intrinsic.is_some() {
            self.error(
                decl.span,
                "a static nested class cannot define a compiler intrinsic type".to_string(),
            );
            checked.intrinsic = None;
        }
        if let Some(kind) = self.type_namespace_conflict(
            owner,
            &decl.name.text,
            file_index,
            crate::namespace::is_file_private(decl.visibility),
        ) {
            let what = if kind == "a class" {
                format!("duplicate class `{}`", decl.name.text)
            } else {
                format!(
                    "duplicate type `{}` (already declared as {kind})",
                    decl.name.text
                )
            };
            self.error(decl.name.span, what);
            return None;
        }
        let modifier = match decl.modifier {
            ast::ClassModifier::Final => hir::ClassModifier::Final,
            ast::ClassModifier::Open => hir::ClassModifier::Open,
            ast::ClassModifier::Abstract | ast::ClassModifier::Sealed => {
                hir::ClassModifier::Abstract
            }
        };
        let mut type_params = enclosing
            .map(|outer| self.classes[outer].type_params.clone())
            .unwrap_or_else(|| self.local_type_parameters());
        for param in &decl.type_params {
            if type_params
                .iter()
                .any(|existing: &hir::TypeParamDecl| existing.name == param.name.text)
            {
                self.error(
                    param.span,
                    format!("duplicate type parameter `{}`", param.name.text),
                );
                continue;
            }
            let parameter = self.fresh_source_type_param(param, type_params.len());
            type_params.push(lower_type_param_decl(param, parameter));
        }
        let self_application =
            hir::ClassApplicationId::from_raw((self.class_applications.len() as u32).into());
        let representation = match checked.intrinsic {
            Some(spec) => {
                if !self.validate_intrinsic_type_source_shape(
                    spec,
                    &decl.name,
                    &decl.type_params,
                    decl.where_clause.as_ref(),
                    decl.constructor.is_omitted(),
                    decl.span,
                ) {
                    return None;
                }
                let required_modifier = if spec.kind == hir::IntrinsicTypeKind::Any {
                    hir::ClassModifier::Abstract
                } else {
                    hir::ClassModifier::Final
                };
                if modifier != required_modifier {
                    self.error(
                        decl.span,
                        if spec.kind == hir::IntrinsicTypeKind::Any {
                            "intrinsic type `core_any` must be abstract".to_string()
                        } else {
                            "an intrinsic class declaration must be final".to_string()
                        },
                    );
                }
                if matches!(
                    spec.kind,
                    hir::IntrinsicTypeKind::Any | hir::IntrinsicTypeKind::Nothing
                ) {
                    if !decl.supertypes.is_empty() {
                        self.error(
                            decl.span,
                            "a root intrinsic type cannot declare supertypes".to_string(),
                        );
                    }
                    let member_spans = decl
                        .members
                        .iter()
                        .map(ast::ClassMember::span)
                        .chain(decl.recovery.methods.iter().map(|method| method.span))
                        .chain(decl.recovery.nested.iter().map(ast::RecoveredNominal::span));
                    for span in member_spans {
                        self.error(
                            span,
                            "a root intrinsic type cannot declare members".to_string(),
                        );
                    }
                    if !matches!(
                        decl.visibility,
                        ast::VisibilitySyntax::Explicit {
                            visibility: ast::DeclaredVisibility::Public,
                            ..
                        }
                    ) {
                        self.error(
                            decl.name.span,
                            "a root intrinsic type must be public".to_string(),
                        );
                    }
                }
                if decl
                    .supertypes
                    .iter()
                    .any(|supertype| supertype.constructor_arguments().is_some())
                {
                    self.error(
                        decl.span,
                        "an intrinsic class declaration cannot have a base class".to_string(),
                    );
                }
                hir::ClassRepresentation::Intrinsic(spec.kind)
            }
            None => hir::ClassRepresentation::Declared,
        };
        let access = match owner {
            Some(owner) => self.nested_nominal_access(
                decl.visibility,
                decl.name.span,
                "class",
                owner,
                file_index,
            ),
            None => self.nominal_access(decl.visibility, decl.name.span, "class", file_index),
        };
        let identity = self.prepare_nominal_identity(NominalIdentityInput {
            name: &decl.name.text,
            parent: owner,
            access: access.declared,
            type_parameter_count: type_params.len(),
            kind: SourceNominalKind::Class,
            file: file_index,
            span: decl.span,
        })?;
        let id = self.classes.alloc(ClassDecl {
            name: decl.name.text.clone(),
            owner: owner.map(Owner::as_nominal_owner),
            access,
            definition: hir::ClassDefinition {
                inheritance_family: if decl.modifier == ast::ClassModifier::Sealed {
                    hir::InheritanceFamily::Sealed(Vec::new())
                } else {
                    hir::InheritanceFamily::Open
                },
                release_policy: Default::default(),
                modifier,
                self_application,
                type_params: type_params.clone(),
                representation,
                fields: Vec::new(),
                base_class: None,
                interfaces: Vec::new(),
                interface_implementations: Vec::new(),

                gc_free_pointee_requirements: Vec::new(),
            },
            // Filled in pass 2; resolution failures are diagnosed, so
            // these never reach the output unfinished.
            fields: Vec::new(),
            properties: Vec::new(),
            constructors: Vec::new(),
            methods: Vec::new(),
            span: decl.span,
        });
        self.register_nominal_identity(Owner::Class(id), identity);
        if self.local_type_declaration.is_some() {
            let ty = self.unit;
            let field = self.allocate_class_field(
                id,
                hir::Field {
                    name: "$captures".into(),
                    ty,
                },
                hir::ClassFieldSource::CapturedEnvironment,
                decl.span,
            );
            self.classes[id].fields.push(field);
        }
        if let Some(outer) = enclosing {
            let ty = self.owner_ty(Owner::Class(outer));
            let field = self.allocate_class_field(
                id,
                hir::Field {
                    name: "$outer".into(),
                    ty,
                },
                hir::ClassFieldSource::EnclosingInstance,
                decl.inner_span
                    .expect("an inner declaration has its modifier span"),
            );
            self.classes[id].fields.push(field);
        }
        let parameter_ids = type_params
            .iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>();
        let type_args = parameter_ids
            .into_iter()
            .map(|parameter| self.intern_type(Type::Param(parameter)))
            .collect();
        let ty = self.class_application(id, type_args);
        if matches!(
            self.classes[id].representation,
            hir::ClassRepresentation::Declared
        ) {
            assert_eq!(self.types[ty], Type::Class(self_application));
        }
        self.register_nominal_name(
            owner,
            &decl.name,
            NominalTarget::Class(id),
            file_index,
            decl.visibility,
        );
        self.record_nominal_file(Owner::Class(id), file_index);
        self.classes[id].access.inheritance =
            hir::InheritanceDomain(if modifier == hir::ClassModifier::Final {
                hir::AccessDomain::empty()
            } else {
                self.classes[id]
                    .access
                    .lookup
                    .0
                    .intersect(&hir::AccessDomain::from_constraints([
                        hir::AccessConstraint::SubclassesOf(
                            self.nominal_identity(Owner::Class(id)).declaration_id(),
                        ),
                    ]))
            });
        if let hir::ClassRepresentation::Intrinsic(intrinsic) = self.classes[id].representation {
            self.register_intrinsic_type(intrinsic, IntrinsicTypeOwner::Class(id), decl.span);
        }
        if is_core && owner.is_none() && decl.name.text == "Throwable" {
            self.throwable_candidates.push((id, ty));
        }
        self.record_editing_owner_errors(Owner::Class(id), diagnostics_before);
        for method in decl.functions() {
            self.declare_method(method, Owner::Class(id), pending_methods, file_index);
        }
        pending.push((id, decl, file_index));
        Some(id)
    }
}
