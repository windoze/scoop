use super::*;

impl Lowerer {
    pub(crate) fn declare_struct<'a>(
        &mut self,
        decl: &'a ast::StructDecl,
        pending: &mut Vec<(StructId, &'a ast::StructDecl, usize)>,
        pending_methods: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
        file_index: usize,
        owner: Option<Owner>,
    ) -> Option<StructId> {
        let mut checked = self.check_struct_annotations(decl);
        if owner.is_some() && checked.intrinsic.is_some() {
            self.error(
                decl.span,
                "a static nested struct cannot define a compiler intrinsic type".to_string(),
            );
            checked.intrinsic = None;
        }
        if let Some(kind) = self.type_namespace_conflict(
            owner,
            &decl.name.text,
            file_index,
            crate::namespace::is_file_private(decl.visibility),
        ) {
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
        let mut type_params = Vec::new();
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
            let parameter = self.fresh_type_param(type_params.len());
            type_params.push(lower_type_param_decl(param, parameter));
        }
        let self_application =
            hir::StructApplicationId::from_raw((self.struct_applications.len() as u32).into());
        let representation = match checked.intrinsic {
            Some(spec) => {
                self.validate_intrinsic_type_source_shape(
                    spec,
                    &decl.name,
                    &decl.type_params,
                    decl.where_clause.as_ref(),
                    decl.fields.is_omitted(),
                    decl.span,
                );
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
        match owner {
            Some(owner) => {
                self.nested_nominals_by_owner
                    .insert((owner, decl.name.text.clone()), NominalTarget::Struct(id));
            }
            None => {
                self.top_level_namespaces.register_type(
                    file_index,
                    decl.name.text.clone(),
                    crate::namespace::TopLevelTypeTarget::Nominal(NominalTarget::Struct(id)),
                    self.structs[id].access.declared == hir::DeclaredVisibility::Private,
                );
            }
        }
        self.struct_files.insert(id, file_index);
        if let hir::StructRepresentation::Intrinsic(intrinsic) = self.structs[id].representation {
            self.register_intrinsic_type(intrinsic, IntrinsicTypeOwner::Struct(id), decl.span);
        }
        for method in decl.functions() {
            self.declare_method(method, Owner::Struct(id), pending_methods, file_index);
        }
        pending.push((id, decl, file_index));
        Some(id)
    }
}
