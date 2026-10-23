use super::*;

impl Lowerer {
    pub(crate) fn declare_struct<'a>(
        &mut self,
        decl: ast::StructSource<'a>,
        pending: &mut Vec<(StructId, ast::StructSource<'a>, usize)>,
        pending_methods: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
        file_index: usize,
        owner: Option<Owner>,
    ) -> Option<StructId> {
        let diagnostics_before = self.diagnostics.len();
        let mut checked = self.check_struct_annotations(decl);
        if owner.is_some() && checked.intrinsic.is_some() {
            self.error(
                decl.span,
                "a static nested struct cannot define a compiler intrinsic type".to_string(),
            );
            checked.intrinsic = None;
        }
        let intrinsic_unit = checked
            .intrinsic
            .is_some_and(|spec| spec.kind == hir::IntrinsicTypeKind::Unit);
        let conflict = if intrinsic_unit {
            self.top_level_namespaces
                .type_conflict(
                    file_index,
                    &decl.name.text,
                    crate::namespace::is_file_private(decl.visibility),
                )
                .map(crate::namespace::TopLevelTypeTarget::description)
        } else {
            self.type_namespace_conflict(
                owner,
                &decl.name.text,
                file_index,
                crate::namespace::is_file_private(decl.visibility),
            )
        };
        if let Some(kind) = conflict {
            let what = if kind == "a struct" {
                format!("duplicate struct `{}`", decl.name.text)
            } else {
                format!(
                    "duplicate type `{}` (already declared as {kind})",
                    decl.name.text
                )
            };
            self.error(decl.name.span, what);
            return None;
        }
        let mut type_params = self.local_type_parameters();
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
            hir::StructApplicationId::from_raw((self.struct_applications.len() as u32).into());
        let representation = match checked.intrinsic {
            Some(spec) => {
                if !self.validate_intrinsic_type_source_shape(
                    spec,
                    &decl.name,
                    &decl.type_params,
                    decl.where_clause.as_ref(),
                    decl.fields.is_omitted(),
                    decl.span,
                ) {
                    return None;
                }
                hir::StructRepresentation::Intrinsic(spec.kind)
            }
            None => hir::StructRepresentation::Declared(Vec::new()),
        };
        let access = match owner {
            Some(owner) => self.nested_nominal_access(
                decl.visibility,
                decl.name.span,
                "struct",
                owner,
                file_index,
            ),
            None => self.nominal_access(decl.visibility, decl.name.span, "struct", file_index),
        };
        let identity = self.prepare_nominal_identity(NominalIdentityInput {
            name: &decl.name.text,
            parent: owner,
            access: access.declared,
            type_parameter_count: type_params.len(),
            kind: SourceNominalKind::Struct,
            file: file_index,
            span: decl.span,
        })?;
        if intrinsic_unit
            && identity.declaration_id()
                != hir::SourceNominalId::Concrete(
                    scoop_identity::CoreBuiltinNominal::Unit
                        .identity_record()
                        .id(),
                )
        {
            self.error(
                decl.name.span,
                "the intrinsic Unit declaration must use the public root-package Unit identity"
                    .to_string(),
            );
            return None;
        }
        let id = self.structs.alloc(StructDecl {
            name: decl.name.text.clone(),
            owner: owner.map(Owner::as_nominal_owner),
            access,
            constructors: Vec::new(),
            methods: Vec::new(),
            properties: Vec::new(),
            derived_equality: None,
            span: decl.span,
            definition: hir::StructDefinition {
                self_application,
                type_params: type_params.clone(),
                representation,
                interfaces: Vec::new(),
                interface_implementations: Vec::new(),

                gc_free_pointee_requirements: Vec::new(),
                attributes: checked.attributes,
            },
        });
        self.register_nominal_identity(Owner::Struct(id), identity);
        let parameter_ids = type_params
            .iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>();
        let type_args = parameter_ids
            .into_iter()
            .map(|parameter| self.intern_type(Type::Param(parameter)))
            .collect();
        let ty = self.struct_application(id, type_args);
        if matches!(
            self.structs[id].representation,
            hir::StructRepresentation::Declared(_)
        ) {
            assert_eq!(self.types[ty], Type::Struct(self_application));
        }
        self.register_nominal_name(
            owner,
            &decl.name,
            NominalTarget::Struct(id),
            file_index,
            decl.visibility,
        );
        self.record_nominal_file(Owner::Struct(id), file_index);
        if let hir::StructRepresentation::Intrinsic(intrinsic) = self.structs[id].representation {
            self.register_intrinsic_type(intrinsic, IntrinsicTypeOwner::Struct(id), decl.span);
        }
        self.record_editing_owner_errors(Owner::Struct(id), diagnostics_before);
        for method in decl.functions() {
            self.declare_method(method, Owner::Struct(id), pending_methods, file_index);
        }
        pending.push((id, decl, file_index));
        Some(id)
    }
}
