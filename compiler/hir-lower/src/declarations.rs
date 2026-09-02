use super::*;

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

    /// Whether a type name is already taken in the shared type
    /// namespace (structs, enums, classes, interfaces). Returns the
    /// kind of the existing declaration for diagnostics.
    pub(super) fn type_namespace_conflict(&self, name: &str) -> Option<&'static str> {
        if self.structs_by_name.contains_key(name) {
            Some("a struct")
        } else if self.enums_by_name.contains_key(name) {
            Some("an enum")
        } else if self.classes_by_name.contains_key(name) {
            Some("a class")
        } else if self.interfaces_by_name.contains_key(name) {
            Some("an interface")
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
    ) {
        let checked = self.check_struct_annotations(decl);
        if let Some(kind) = self.type_namespace_conflict(&decl.name.text) {
            let what = if kind == "a struct" {
                format!("duplicate struct `{}`", decl.name.text)
            } else {
                format!(
                    "duplicate type `{}` (already declared as {kind})",
                    decl.name.text
                )
            };
            self.error(decl.name.span, what);
            return;
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
        let id = self.structs.alloc(StructDecl {
            name: decl.name.text.clone(),
            self_application,
            type_params: type_params.clone(),
            attributes: checked.attributes,
            representation,
            // Filled in pass 2 together with the fields.
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
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
        self.structs_by_name
            .insert(decl.name.text.clone(), (id, ty));
        self.struct_files.insert(id, file_index);
        if let hir::StructRepresentation::Intrinsic(intrinsic) = self.structs[id].representation {
            self.register_intrinsic_type(intrinsic, IntrinsicTypeOwner::Struct(id), decl.span);
        }
        for method in &decl.methods {
            self.declare_method(method, Owner::Struct(id), pending_methods, file_index);
        }
        pending.push((id, decl, file_index));
    }

    pub(super) fn declare_enum<'a>(
        &mut self,
        decl: &'a ast::EnumDecl,
        is_core: bool,
        pending: &mut Vec<(EnumId, &'a ast::EnumDecl, usize)>,
        pending_methods: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
        file_index: usize,
    ) {
        let no_gc = self.check_enum_annotations(decl);
        if let Some(kind) = self.type_namespace_conflict(&decl.name.text) {
            let what = if kind == "an enum" {
                format!("duplicate enum `{}`", decl.name.text)
            } else {
                format!(
                    "duplicate type `{}` (already declared as {kind})",
                    decl.name.text
                )
            };
            self.error(decl.name.span, what);
            return;
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
        let id = self.enums.alloc(EnumDecl {
            name: decl.name.text.clone(),
            self_application,
            type_params,
            no_gc,
            // Filled in pass 2; a resolution failure is diagnosed, so
            // empty variants never reach the output.
            variants: Vec::new(),
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            derived_equality: None,
            span: decl.span,
        });
        self.enums_by_name.insert(decl.name.text.clone(), id);
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
        if is_core && decl.name.text == "Option" {
            self.option_candidates
                .push((id, file_index, decl.span, decl.type_params.len()));
        }
        for method in &decl.methods {
            self.declare_method(method, Owner::Enum(id), pending_methods, file_index);
        }
        pending.push((id, decl, file_index));
    }

    pub(super) fn declare_class<'a>(
        &mut self,
        decl: &'a ast::ClassDecl,
        is_core: bool,
        pending: &mut Vec<(ClassId, &'a ast::ClassDecl, usize)>,
        pending_methods: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
        file_index: usize,
    ) {
        let checked = self.check_class_annotations(decl);
        if let Some(kind) = self.type_namespace_conflict(&decl.name.text) {
            let what = if kind == "a class" {
                format!("duplicate class `{}`", decl.name.text)
            } else {
                format!(
                    "duplicate type `{}` (already declared as {kind})",
                    decl.name.text
                )
            };
            self.error(decl.name.span, what);
            return;
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
                if decl.base_class.is_some() {
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
            None => hir::ClassRepresentation::Declared(Vec::new()),
        };
        let id = self.classes.alloc(ClassDecl {
            modifier,
            name: decl.name.text.clone(),
            self_application,
            type_params: type_params.clone(),
            // Filled in pass 2; resolution failures are diagnosed, so
            // these never reach the output unfinished.
            representation,
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
            hir::ClassRepresentation::Declared(_)
        ) {
            assert_eq!(self.types[ty], Type::Class(self_application));
        }
        self.classes_by_name
            .insert(decl.name.text.clone(), (id, ty));
        self.class_files.insert(id, file_index);
        if let hir::ClassRepresentation::Intrinsic(intrinsic) = self.classes[id].representation {
            self.register_intrinsic_type(intrinsic, IntrinsicTypeOwner::Class(id), decl.span);
        }
        if is_core && decl.name.text == "Throwable" {
            self.throwable_candidates.push((id, ty));
        }
        for method in &decl.methods {
            self.declare_method(method, Owner::Class(id), pending_methods, file_index);
        }
        pending.push((id, decl, file_index));
    }

    pub(super) fn declare_interface<'a>(
        &mut self,
        decl: &'a ast::InterfaceDecl,
        pending: &mut Vec<(InterfaceId, &'a ast::InterfaceDecl, usize)>,
        pending_methods: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
        file_index: usize,
    ) {
        self.reject_type_annotations("an interface", &decl.annotations);
        if let Some(kind) = self.type_namespace_conflict(&decl.name.text) {
            let what = if kind == "an interface" {
                format!("duplicate interface `{}`", decl.name.text)
            } else {
                format!(
                    "duplicate type `{}` (already declared as {kind})",
                    decl.name.text
                )
            };
            self.error(decl.name.span, what);
            return;
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
        let id = self.interfaces.alloc(InterfaceDecl {
            name: decl.name.text.clone(),
            self_application,
            type_params: type_params.clone(),
            parents: Vec::new(),
            // Filled in pass 2.5 together with the method signatures.
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
        let ty = self.intern_interface_application(id, type_args);
        assert_eq!(self.types[ty], Type::Interface(self_application));
        self.interfaces_by_name
            .insert(decl.name.text.clone(), (id, ty));
        self.interface_files.insert(id, file_index);
        self.interface_methods.insert(id, Vec::new());
        for method in &decl.methods {
            self.declare_method(method, Owner::Interface(id), pending_methods, file_index);
        }
        pending.push((id, decl, file_index));
    }

    /// Declare a member function (pass 1): methods live in per-owner
    /// namespaces where one name may collect several overloads (M7;
    /// same-signature duplicates are diagnosed in pass 2.6, once
    /// parameter types are known) and are named `Owner.method` for
    /// unambiguous symbols downstream; `Function::method` records the
    /// host type and effective modality. Signatures and bodies are filled in passes
    /// 2.5 / 3.
    pub(super) fn declare_method<'a>(
        &mut self,
        decl: &'a ast::FunctionDecl,
        owner: Owner,
        pending: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
        file_index: usize,
    ) {
        let checked = self.check_function_annotations(decl, FunctionTarget::Member(owner));
        let host_ty = self.owner_ty(owner);
        let modifier = match owner {
            Owner::Interface(_) => hir::MethodModifier::Abstract,
            Owner::Struct(_) | Owner::Enum(_) => hir::MethodModifier::Final,
            Owner::Class(class_id)
                if self.classes[class_id].modifier == hir::ClassModifier::Final
                    && decl.is_override
                    && decl.modifier == ast::MethodModifier::Open =>
            {
                // An override is open by default, but a final owner
                // makes it effectively final.
                hir::MethodModifier::Final
            }
            Owner::Class(_) => match decl.modifier {
                ast::MethodModifier::Final => hir::MethodModifier::Final,
                ast::MethodModifier::Open => hir::MethodModifier::Open,
                ast::MethodModifier::Abstract => hir::MethodModifier::Abstract,
            },
        };
        let kind = match checked.intrinsic {
            Some(intrinsic) => FunctionKind::Intrinsic(intrinsic),
            None => FunctionKind::User(hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            }),
        };
        let id = self.functions.alloc(Function {
            name: format!("{}.{}", owner.describe_name(self), decl.name.text),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: decl.is_suspend,
            // Filled in pass 2.5 (signature) and pass 3 (body and
            // parameter locals).
            params: Vec::new(),
            return_ty: self.unit,
            attributes: checked.attributes,
            kind,
            method: Some(hir::Method {
                owner: host_ty,
                modifier,
                dispatch: hir::MethodDispatch::Direct,
                operator: match (&decl.operator, decl.name.text.as_str()) {
                    (Some(_), "equals") => Some(hir::OperatorKind::Equals),
                    _ => None,
                },
            }),
            span: decl.span,
        });
        if let Some(intrinsic) = checked.intrinsic {
            self.register_intrinsic_function(id, intrinsic, decl.span);
        }
        self.function_owner.insert(id, owner);
        self.function_files.insert(id, file_index);
        match owner {
            Owner::Class(owner) => self.classes[owner].methods.push(id),
            Owner::Interface(owner) => {
                self.interface_methods
                    .get_mut(&owner)
                    .expect("the interface owner map was initialized above")
                    .push(id);
                let member = self.interface_method_entities.alloc(hir::InterfaceMethod {
                    owner,
                    function: id,
                });
                self.functions[id]
                    .method
                    .as_mut()
                    .expect("declared interface function is a method")
                    .dispatch = hir::MethodDispatch::Interface(member);
                self.interfaces[owner].methods.push(member);
            }
            Owner::Struct(owner) => self.structs[owner].methods.push(id),
            Owner::Enum(owner) => self.enums[owner].methods.push(id),
        }
        pending.push((id, decl, file_index, owner));
    }

    /// Declare a top-level function (pass 1). One name may collect
    /// several overloads (M7); same-signature duplicates are diagnosed
    /// in pass 2.6, once parameter types are known.
    pub(super) fn declare_function<'a>(
        &mut self,
        decl: &'a ast::FunctionDecl,
        pending: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize)>,
        file_index: usize,
    ) {
        if let Some(operator) = decl.operator {
            self.error(
                operator.span,
                "`operator` is only allowed on member functions".to_string(),
            );
        }
        let checked = self.check_function_annotations(decl, FunctionTarget::TopLevel);
        let kind = match (checked.intrinsic, checked.extern_) {
            (Some(intrinsic), _) => FunctionKind::Intrinsic(intrinsic),
            (None, Some(extern_)) => {
                let id = self.extern_functions.alloc(hir::ExternFunction {
                    source_name: decl.name.text.clone(),
                    native_symbol: extern_.native_symbol,
                    library: extern_.library,
                    abi: extern_.abi,
                    calling_convention: checked.attributes.calling_convention,
                    gc_effect: checked.attributes.gc_effect,
                    safety: checked.attributes.safety,
                    params: Vec::new(),
                    return_type: self.unit,
                });
                FunctionKind::Extern(id)
            }
            (None, None) => FunctionKind::User(hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            }),
        };
        let id = self.functions.alloc(Function {
            name: decl.name.text.clone(),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: decl.is_suspend,
            // Filled in pass 2.5 (signature) and pass 3 (parameter
            // locals); a resolution failure is diagnosed, so these
            // never reach the output.
            params: Vec::new(),
            return_ty: self.unit,
            attributes: checked.attributes,
            kind,
            method: None,
            span: decl.span,
        });
        if let Some(intrinsic) = checked.intrinsic {
            self.register_intrinsic_function(id, intrinsic, decl.span);
        }
        self.top_level.push(id);
        let namespace = if decl.receiver_ty.is_some() {
            &mut self.extensions_by_name
        } else {
            &mut self.functions_by_name
        };
        namespace
            .entry(decl.name.text.clone())
            .or_default()
            .push(id);
        self.function_files.insert(id, file_index);
        pending.push((id, decl, file_index));
    }

    /// Overload declaration check (pass 2.6, milestone7 DESIGN.md 1.1):
    /// within one name (top-level) or one host (members) two functions
    /// may share a name only when their signatures are distinguishable
    /// — a different parameter count or at least one different
    /// parameter type. Differing only in the return type is a
    /// duplicate. Override / interface-implementation matching is
    /// unaffected (it compares full signatures, pass 2.75). Iterates
    /// in declaration order so diagnostics are deterministic.
    pub(super) fn check_duplicate_signatures(
        &mut self,
        pending_functions: &[(FunctionId, &ast::FunctionDecl, usize)],
        pending_methods: &[(FunctionId, &ast::FunctionDecl, usize, Owner)],
    ) {
        for (index, &(id, decl, file_index)) in pending_functions.iter().enumerate() {
            let duplicate = pending_functions[..index].iter().any(|&(other, _, _)| {
                self.functions[other].name == decl.name.text
                    && self.same_parameter_signature(id, other)
            });
            if duplicate {
                self.current_file = file_index;
                self.error(
                    decl.name.span,
                    format!(
                        "function `{}` is already declared with the same signature",
                        decl.name.text
                    ),
                );
            }
        }
        for (index, &(id, decl, file_index, owner)) in pending_methods.iter().enumerate() {
            let duplicate = pending_methods[..index]
                .iter()
                .any(|&(other, _, _, other_owner)| {
                    other_owner == owner
                        && self.functions[other].name == self.functions[id].name
                        && self.same_parameter_signature(id, other)
                });
            if duplicate {
                let host = owner.describe(self);
                self.current_file = file_index;
                self.error(
                    decl.name.span,
                    format!(
                        "function `{}` in {host} is already declared with the same signature",
                        decl.name.text
                    ),
                );
            }
        }
    }

    /// Whether two functions have indistinguishable parameter
    /// signatures: same parameter count and pairwise-equal parameter
    /// types (the return type is not part of it).
    pub(super) fn same_parameter_signature(&self, a: FunctionId, b: FunctionId) -> bool {
        let (Some(a_sig), Some(b_sig)) = (self.signatures.get(&a), self.signatures.get(&b)) else {
            return false;
        };
        let a_sig = a_sig.clone();
        let b_sig = b_sig.clone();
        if a_sig.type_params.len() != b_sig.type_params.len() {
            return false;
        }
        let parameter_pairs = a_sig
            .type_params
            .iter()
            .zip(&b_sig.type_params)
            .map(|(a, b)| (a.id, b.id))
            .collect::<Vec<_>>();
        let receivers_match = match (
            self.extension_receivers.get(&a),
            self.extension_receivers.get(&b),
        ) {
            (Some(&a), Some(&b)) => self.signature_types_equal(a, b, &parameter_pairs),
            (None, None) => true,
            _ => false,
        };
        receivers_match
            && a_sig.params.len() == b_sig.params.len()
            && a_sig
                .params
                .iter()
                .zip(&b_sig.params)
                .all(|(x, y)| self.signature_types_equal(x.ty, y.ty, &parameter_pairs))
    }

    /// Compare two declaration-signature types under the exact
    /// alpha-renaming relation produced by their owning signatures. Type
    /// parameters keep globally unique identities in HIR; declaration
    /// equivalence therefore cannot use raw `TypeId` equality.
    pub(super) fn signature_types_equal(
        &self,
        left: TypeId,
        right: TypeId,
        parameter_pairs: &[(hir::TypeParamId, hir::TypeParamId)],
    ) -> bool {
        match (&self.types[left], &self.types[right]) {
            (Type::Param(left), Type::Param(right)) => {
                parameter_pairs
                    .iter()
                    .any(|&(expected_left, expected_right)| {
                        expected_left == *left && expected_right == *right
                    })
            }
            (Type::Struct(left), Type::Struct(right)) => {
                let left = &self.struct_applications[*left];
                let right = &self.struct_applications[*right];
                left.template == right.template
                    && self.signature_type_lists_equal(
                        &left.arguments,
                        &right.arguments,
                        parameter_pairs,
                    )
            }
            (Type::Class(left), Type::Class(right)) => {
                let left = &self.class_applications[*left];
                let right = &self.class_applications[*right];
                left.template == right.template
                    && self.signature_type_lists_equal(
                        &left.arguments,
                        &right.arguments,
                        parameter_pairs,
                    )
            }
            (Type::Interface(left), Type::Interface(right)) => {
                let left = &self.interface_applications[*left];
                let right = &self.interface_applications[*right];
                left.template == right.template
                    && self.signature_type_lists_equal(
                        &left.arguments,
                        &right.arguments,
                        parameter_pairs,
                    )
            }
            (Type::Enum(left), Type::Enum(right)) => {
                let left = &self.enum_applications[*left];
                let right = &self.enum_applications[*right];
                left.template == right.template
                    && self.signature_type_lists_equal(
                        &left.arguments,
                        &right.arguments,
                        parameter_pairs,
                    )
            }
            (Type::Tuple(left), Type::Tuple(right)) => {
                self.signature_type_lists_equal(left, right, parameter_pairs)
            }
            (Type::Function(left), Type::Function(right))
            | (Type::FunPtr(left), Type::FunPtr(right)) => {
                let left = &self.function_types[*left];
                let right = &self.function_types[*right];
                left.is_suspend == right.is_suspend
                    && self.signature_type_lists_equal(
                        &left.parameter_types,
                        &right.parameter_types,
                        parameter_pairs,
                    )
                    && self.signature_types_equal(
                        left.return_type,
                        right.return_type,
                        parameter_pairs,
                    )
            }
            (Type::Ptr(left), Type::Ptr(right)) => {
                self.signature_types_equal(*left, *right, parameter_pairs)
            }
            _ => self.types_equal(left, right),
        }
    }

    pub(super) fn signature_type_lists_equal(
        &self,
        left: &[TypeId],
        right: &[TypeId],
        parameter_pairs: &[(hir::TypeParamId, hir::TypeParamId)],
    ) -> bool {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(&left, &right)| self.signature_types_equal(left, right, parameter_pairs))
    }
}
