use super::*;

mod callables;
mod nested;
mod objects;
pub(crate) use nested::NestedDeclarationQueues;
pub(crate) use objects::ObjectSource;

impl Lowerer {
    pub(super) fn require_core_struct(
        &mut self,
        name: &str,
        files: &[ast::SourceFile],
    ) -> Option<StructId> {
        let candidate = self.structs_by_name.get(name).map(|(id, _)| *id);
        if let Some(id) = candidate
            && self
                .struct_files
                .get(&id)
                .copied()
                .unwrap_or(self.user_file_index)
                < self.user_file_index
        {
            return Some(id);
        }
        self.current_file = candidate
            .and_then(|id| self.struct_files.get(&id).copied())
            .unwrap_or(0);
        self.error(
            files[0].span,
            format!("scoop.core must define exactly one `{name}` struct"),
        );
        None
    }

    /// Whether a name is already taken in the shared type namespace. This
    /// includes compiler built-ins, top-level aliases and every nominal kind.
    /// Returns the kind of the existing declaration for diagnostics.
    pub(super) fn type_namespace_conflict(
        &self,
        owner: Option<Owner>,
        name: &str,
    ) -> Option<&'static str> {
        if let Some(target) = owner.and_then(|owner| {
            self.nested_nominals_by_owner
                .get(&(owner, name.to_string()))
        }) {
            return Some(match target {
                NominalTarget::Struct(_) => "a struct",
                NominalTarget::Enum(_) => "an enum",
                NominalTarget::Class(_) => "a class",
                NominalTarget::Interface(_) => "an interface",
                NominalTarget::Object(_) => "an object",
            });
        }
        if owner.is_some() {
            return None;
        }
        if matches!(name, "Unit" | "Any") {
            Some("a built-in type")
        } else if self.source_type_aliases_by_name.contains_key(name) {
            Some("a typealias")
        } else if self.structs_by_name.contains_key(name) {
            Some("a struct")
        } else if self.enums_by_name.contains_key(name) {
            Some("an enum")
        } else if self.classes_by_name.contains_key(name) {
            Some("a class")
        } else if self.interfaces_by_name.contains_key(name) {
            Some("an interface")
        } else if self.objects_by_name.contains_key(name) {
            Some("an object")
        } else {
            None
        }
    }

    pub(super) fn declare_struct<'a>(
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
        if let Some(kind) = self.type_namespace_conflict(owner, &decl.name.text) {
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
                hir::StructRepresentation::Intrinsic(hir::IntrinsicTypeDeclaration {
                    kind: spec.kind,
                    provider: self.current_intrinsic_provider(),
                })
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
        let id = self.structs.alloc(StructDecl {
            name: decl.name.text.clone(),
            owner: owner.map(Owner::as_nominal_owner),
            access,
            self_application,
            type_params: type_params.clone(),
            attributes: checked.attributes,
            representation,
            constructors: Vec::new(),
            // Filled in pass 2 together with the fields.
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            properties: Vec::new(),
            derived_equality: None,
            span: decl.span,
        });
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
                self.structs_by_name
                    .insert(decl.name.text.clone(), (id, ty));
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

    pub(super) fn declare_enum<'a>(
        &mut self,
        decl: &'a ast::EnumDecl,
        is_core: bool,
        pending: &mut Vec<(EnumId, &'a ast::EnumDecl, usize)>,
        pending_methods: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
        file_index: usize,
        owner: Option<Owner>,
    ) -> Option<EnumId> {
        let no_gc = self.check_enum_annotations(decl);
        if let Some(kind) = self.type_namespace_conflict(owner, &decl.name.text) {
            let what = if kind == "an enum" {
                format!("duplicate enum `{}`", decl.name.text)
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
            hir::EnumApplicationId::from_raw((self.enum_applications.len() as u32).into());
        let access = match owner {
            Some(owner) => self.nested_nominal_access(
                decl.visibility,
                decl.name.span,
                "enum",
                owner,
                file_index,
            ),
            None => self.nominal_access(decl.visibility, decl.name.span, "enum", file_index),
        };
        let id = self.enums.alloc(EnumDecl {
            name: decl.name.text.clone(),
            owner: owner.map(Owner::as_nominal_owner),
            access,
            self_application,
            type_params,
            no_gc,
            // Filled in pass 2; a resolution failure is diagnosed, so
            // empty variants never reach the output.
            variants: Vec::new(),
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            properties: Vec::new(),
            derived_equality: None,
            span: decl.span,
        });
        match owner {
            Some(owner) => {
                self.nested_nominals_by_owner
                    .insert((owner, decl.name.text.clone()), NominalTarget::Enum(id));
            }
            None => {
                self.enums_by_name.insert(decl.name.text.clone(), id);
            }
        }
        let parameter_ids = self.enums[id]
            .type_params
            .iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>();
        let type_args = parameter_ids
            .into_iter()
            .map(|parameter| self.intern_type(Type::Param(parameter)))
            .collect();
        let ty = self.enum_application(id, type_args);
        assert_eq!(self.types[ty], Type::Enum(self_application));
        self.enum_files.insert(id, file_index);
        if is_core && owner.is_none() && decl.name.text == "Option" {
            self.option_candidates
                .push((id, file_index, decl.span, decl.type_params.len()));
        }
        for method in &decl.methods {
            self.declare_method(method, Owner::Enum(id), pending_methods, file_index);
        }
        pending.push((id, decl, file_index));
        Some(id)
    }

    pub(super) fn declare_class<'a>(
        &mut self,
        decl: &'a ast::ClassDecl,
        is_core: bool,
        pending: &mut Vec<(ClassId, &'a ast::ClassDecl, usize)>,
        pending_methods: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
        file_index: usize,
        owner: Option<Owner>,
    ) -> Option<ClassId> {
        let mut checked = self.check_class_annotations(decl);
        if owner.is_some() && checked.intrinsic.is_some() {
            self.error(
                decl.span,
                "a static nested class cannot define a compiler intrinsic type".to_string(),
            );
            checked.intrinsic = None;
        }
        if let Some(kind) = self.type_namespace_conflict(owner, &decl.name.text) {
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
            ast::ClassModifier::Abstract => hir::ClassModifier::Abstract,
        };
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
            hir::ClassApplicationId::from_raw((self.class_applications.len() as u32).into());
        let representation = match checked.intrinsic {
            Some(spec) => {
                self.validate_intrinsic_type_source_shape(
                    spec,
                    &decl.name,
                    &decl.type_params,
                    decl.where_clause.as_ref(),
                    decl.constructor.is_omitted(),
                    decl.span,
                );
                if modifier != hir::ClassModifier::Final {
                    self.error(
                        decl.span,
                        "an intrinsic class declaration must be final".to_string(),
                    );
                }
                if decl
                    .supertypes
                    .iter()
                    .any(|supertype| supertype.constructor_arguments.is_some())
                {
                    self.error(
                        decl.span,
                        "an intrinsic class declaration cannot have a base class".to_string(),
                    );
                }
                hir::ClassRepresentation::Intrinsic(hir::IntrinsicTypeDeclaration {
                    kind: spec.kind,
                    provider: self.current_intrinsic_provider(),
                })
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
        let id = self.classes.alloc(ClassDecl {
            modifier,
            name: decl.name.text.clone(),
            owner: owner.map(Owner::as_nominal_owner),
            access,
            self_application,
            type_params: type_params.clone(),
            // Filled in pass 2; resolution failures are diagnosed, so
            // these never reach the output unfinished.
            representation,
            fields: Vec::new(),
            properties: Vec::new(),
            constructors: Vec::new(),
            base_class: None,
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            span: decl.span,
        });
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
        match owner {
            Some(owner) => {
                self.nested_nominals_by_owner
                    .insert((owner, decl.name.text.clone()), NominalTarget::Class(id));
            }
            None => {
                self.classes_by_name
                    .insert(decl.name.text.clone(), (id, ty));
            }
        }
        self.class_files.insert(id, file_index);
        self.classes[id].access.inheritance =
            hir::InheritanceDomain(if modifier == hir::ClassModifier::Final {
                hir::AccessDomain::empty()
            } else {
                self.classes[id]
                    .access
                    .lookup
                    .0
                    .intersect(&hir::AccessDomain::from_constraints([
                        hir::AccessConstraint::SubclassesOf(id),
                    ]))
            });
        if let hir::ClassRepresentation::Intrinsic(intrinsic) = self.classes[id].representation {
            self.register_intrinsic_type(intrinsic, IntrinsicTypeOwner::Class(id), decl.span);
        }
        if is_core && owner.is_none() && decl.name.text == "Throwable" {
            self.throwable_candidates.push((id, ty));
        }
        for method in decl.functions() {
            self.declare_method(method, Owner::Class(id), pending_methods, file_index);
        }
        pending.push((id, decl, file_index));
        Some(id)
    }

    pub(super) fn declare_interface<'a>(
        &mut self,
        decl: &'a ast::InterfaceDecl,
        pending: &mut Vec<(InterfaceId, &'a ast::InterfaceDecl, usize)>,
        pending_methods: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
        file_index: usize,
        owner: Option<Owner>,
    ) -> Option<InterfaceId> {
        self.reject_type_annotations("an interface", &decl.annotations);
        if let Some(kind) = self.type_namespace_conflict(owner, &decl.name.text) {
            let what = if kind == "an interface" {
                format!("duplicate interface `{}`", decl.name.text)
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
        let self_application = hir::InterfaceApplicationId::from_raw(
            (self.interface_applications.len() as u32).into(),
        );
        let access = match owner {
            Some(owner) => self.nested_nominal_access(
                decl.visibility,
                decl.name.span,
                "interface",
                owner,
                file_index,
            ),
            None => self.nominal_access(decl.visibility, decl.name.span, "interface", file_index),
        };
        let id = self.interfaces.alloc(InterfaceDecl {
            name: decl.name.text.clone(),
            owner: owner.map(Owner::as_nominal_owner),
            access,
            self_application,
            type_params: type_params.clone(),
            parents: Vec::new(),
            // Filled in pass 2.5 together with the method signatures.
            methods: Vec::new(),
            private_methods: Vec::new(),
            properties: Vec::new(),
            span: decl.span,
        });
        let parameter_ids = type_params
            .iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>();
        let type_args = parameter_ids
            .into_iter()
            .map(|parameter| self.intern_type(Type::Param(parameter)))
            .collect();
        let ty = self.intern_interface_application(id, type_args);
        assert_eq!(self.types[ty], Type::Interface(self_application));
        match owner {
            Some(owner) => {
                self.nested_nominals_by_owner.insert(
                    (owner, decl.name.text.clone()),
                    NominalTarget::Interface(id),
                );
            }
            None => {
                self.interfaces_by_name
                    .insert(decl.name.text.clone(), (id, ty));
            }
        }
        self.interface_files.insert(id, file_index);
        self.interface_methods.insert(id, Vec::new());
        for method in &decl.methods {
            self.declare_method(method, Owner::Interface(id), pending_methods, file_index);
        }
        pending.push((id, decl, file_index));
        Some(id)
    }
}
