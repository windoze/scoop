//! Class / interface resolution and inheritance checks (M6,
//! milestone6 DESIGN.md 2.2).
//!
//! Pass 2 (`resolve_class`): constructor properties (duplicate names
//! diagnosed), the base-class clause (the base must be an `open` or
//! `abstract` class) and the interface list.
//!
//! Pass 2.5 (`resolve_method_signature`): member signatures. A generic
//! host's parameters form the prefix of the method's parameter space and
//! parameters declared by the method form the suffix. Generic member
//! functions are static-only (spec 3.2). Bodyless declarations — interface
//! methods and `abstract`
//! class methods — get their parameter-only body here (`this` plus
//! the declared parameters; the statements are empty because there is
//! nothing to execute — MIR only ever reaches them through a vtable /
//! itable slot that names a concrete override).
//!
//! Pass 2.75 (`check_inheritance`): inheritance cycles, property
//! shadowing (M6 simplification: a property may not reuse a base
//! property's name), the `override` rules (overriding without the
//! modifier and the modifier without an overridden method are both
//! diagnostics; implementations of interface methods require it,
//! DESIGN.md 5.2) and interface implementation (every interface
//! method must have a same-signature concrete method on the class or
//! its base chain — name, parameter types and return type all equal;
//! overloads only match exactly, M7).
//!
//! Object layout decision (see the crate docs): a subclass object is
//! laid out as the base class's fields followed by its own, with
//! consecutive indices — `FieldRef::ClassField::index` is the absolute
//! layout index and `class_id` the class that declared the property.
//! This is orthogonal to the vtable layout (mir-lower's job).

use scoop_ast as ast;
use scoop_hir as hir;

use hir::{ClassId, FunctionId, Type, TypeId};

use crate::{FnParam, FnSig, ForbiddenSuspendContext, Lowerer, Owner, SuspensionContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypePosition {
    Covariant,
    Contravariant,
    Invariant,
}

impl TypePosition {
    fn through(self, variance: hir::Variance) -> Self {
        match (self, variance) {
            (Self::Invariant, _) | (_, hir::Variance::Invariant) => Self::Invariant,
            (Self::Covariant, hir::Variance::Out) | (Self::Contravariant, hir::Variance::In) => {
                Self::Covariant
            }
            (Self::Covariant, hir::Variance::In) | (Self::Contravariant, hir::Variance::Out) => {
                Self::Contravariant
            }
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Covariant => "covariant",
            Self::Contravariant => "contravariant",
            Self::Invariant => "invariant",
        }
    }
}

impl Lowerer {
    pub(crate) fn check_interface_inheritance_cycles(
        &mut self,
        pending: &[(hir::InterfaceId, &ast::InterfaceDecl, usize)],
    ) {
        for &(interface, declaration, file) in pending {
            self.current_file = file;
            let mut visiting = Vec::new();
            if self.interface_reaches(interface, interface, &mut visiting) {
                self.error(
                    declaration.span,
                    format!(
                        "interface `{}` directly or indirectly inherits from itself",
                        declaration.name.text
                    ),
                );
            }
        }
    }

    fn interface_reaches(
        &self,
        current: hir::InterfaceId,
        target: hir::InterfaceId,
        visiting: &mut Vec<hir::InterfaceId>,
    ) -> bool {
        if visiting.contains(&current) {
            return false;
        }
        visiting.push(current);
        let reaches = self.interfaces[current].parents.iter().any(|parent| {
            let parent = self.interface_applications[*parent].template;
            parent == target || self.interface_reaches(parent, target, visiting)
        });
        visiting.pop();
        reaches
    }

    /// Validate declaration-site variance against every resolved method
    /// signature. Nested interface applications compose their own variance;
    /// all currently invariant constructors collapse the nested position.
    pub(crate) fn check_interface_variance(&mut self) {
        let interfaces: Vec<hir::InterfaceId> = self.interfaces.iter().map(|(id, _)| id).collect();
        for interface in interfaces {
            if self.interfaces[interface].type_params.is_empty() {
                continue;
            }
            let params = self.interfaces[interface].type_params.clone();
            let methods: Vec<_> = self.interface_methods[&interface]
                .iter()
                .map(|&method| {
                    let signature = &self.signatures[&method];
                    (
                        self.functions[method]
                            .name
                            .rsplit('.')
                            .next()
                            .expect("interface methods are qualified")
                            .to_string(),
                        signature
                            .params
                            .iter()
                            .map(|param| param.ty)
                            .collect::<Vec<_>>(),
                        signature.return_ty,
                        self.functions[method].span,
                    )
                })
                .collect();
            if let Some(&method) = self.interface_methods[&interface].first()
                && let Some(&file) = self.function_files.get(&method)
            {
                self.current_file = file;
            }
            for (method, method_params, return_ty, method_span) in methods {
                for ty in method_params {
                    self.check_variance_position(
                        ty,
                        TypePosition::Contravariant,
                        &params,
                        &method,
                        method_span,
                    );
                }
                self.check_variance_position(
                    return_ty,
                    TypePosition::Covariant,
                    &params,
                    &method,
                    method_span,
                );
            }
        }
    }

    fn check_variance_position(
        &mut self,
        ty: TypeId,
        position: TypePosition,
        params: &[hir::TypeParamDecl],
        method: &str,
        span: ast::Span,
    ) {
        match self.types[ty].clone() {
            Type::Param(id) => {
                // Parameters declared by a generic interface method follow
                // the interface's own prefix and do not participate in the
                // declaration-site variance of that prefix.
                let Some(param) = params.iter().find(|parameter| parameter.id == id) else {
                    return;
                };
                let valid = matches!(param.variance, hir::Variance::Invariant)
                    || matches!(
                        (param.variance, position),
                        (hir::Variance::Out, TypePosition::Covariant)
                            | (hir::Variance::In, TypePosition::Contravariant)
                    );
                if !valid {
                    let declared = match param.variance {
                        hir::Variance::Out => "covariant",
                        hir::Variance::In => "contravariant",
                        hir::Variance::Invariant => unreachable!(),
                    };
                    self.error(
                        span,
                        format!(
                            "{declared} type parameter `{}` occurs in {} position in interface method `{method}`",
                            param.name,
                            position.name()
                        ),
                    );
                }
            }
            Type::Interface(application) => {
                let application = self.interface_applications[application].clone();
                let variances: Vec<_> = self.interfaces[application.template]
                    .type_params
                    .iter()
                    .map(|param| param.variance)
                    .collect();
                for (arg, variance) in application.arguments.into_iter().zip(variances) {
                    self.check_variance_position(
                        arg,
                        position.through(variance),
                        params,
                        method,
                        span,
                    );
                }
            }
            Type::Struct(application) => {
                let args = self.struct_applications[application].arguments.clone();
                for arg in args {
                    self.check_variance_position(
                        arg,
                        TypePosition::Invariant,
                        params,
                        method,
                        span,
                    );
                }
            }
            Type::Class(application) => {
                let args = self.class_applications[application].arguments.clone();
                for arg in args {
                    self.check_variance_position(
                        arg,
                        TypePosition::Invariant,
                        params,
                        method,
                        span,
                    );
                }
            }
            Type::Enum(application) => {
                let args = self.enum_applications[application].arguments.clone();
                for arg in args {
                    self.check_variance_position(
                        arg,
                        TypePosition::Invariant,
                        params,
                        method,
                        span,
                    );
                }
            }
            Type::Tuple(args) => {
                for arg in args {
                    self.check_variance_position(
                        arg,
                        TypePosition::Invariant,
                        params,
                        method,
                        span,
                    );
                }
            }
            Type::Ptr(pointee) => {
                self.check_variance_position(pointee, TypePosition::Invariant, params, method, span)
            }
            Type::FunPtr(id) => {
                let function = self.function_types[id].clone();
                for parameter in function.parameter_types {
                    self.check_variance_position(
                        parameter,
                        TypePosition::Invariant,
                        params,
                        method,
                        span,
                    );
                }
                self.check_variance_position(
                    function.return_type,
                    TypePosition::Invariant,
                    params,
                    method,
                    span,
                );
            }
            Type::Function(id) => {
                let function = self.function_types[id].clone();
                for parameter in function.parameter_types {
                    self.check_variance_position(
                        parameter,
                        position.through(hir::Variance::In),
                        params,
                        method,
                        span,
                    );
                }
                self.check_variance_position(
                    function.return_type,
                    position.through(hir::Variance::Out),
                    params,
                    method,
                    span,
                );
            }
            Type::Unit | Type::Int | Type::UInt | Type::Boolean | Type::String | Type::Any => {}
        }
    }

    /// Resolve a class's constructor properties, base-class clause and
    /// interface list (pass 2).
    pub(crate) fn resolve_class(&mut self, id: ClassId, decl: &ast::ClassDecl) {
        self.type_params_in_scope = self.classes[id].type_params.clone();
        if matches!(
            self.classes[id].representation,
            hir::ClassRepresentation::Intrinsic(_)
        ) {
            let interfaces = self.resolve_interface_list(&decl.interfaces);
            self.classes[id].interfaces = interfaces;
            self.type_params_in_scope.clear();
            return;
        }
        let mut seen = std::collections::HashSet::new();
        let mut props = Vec::new();
        for prop in &decl.constructor {
            if !seen.insert(prop.name.text.clone()) {
                self.error(
                    prop.name.span,
                    format!(
                        "duplicate property `{}` in class `{}`",
                        prop.name.text, decl.name.text
                    ),
                );
                continue;
            }
            let Some(ty) = self.resolve_type_ref(&prop.ty) else {
                continue; // diagnostic already recorded
            };
            props.push(hir::ConstructorField {
                parameter: hir::ConstructorParamId::from_raw(props.len() as u32),
                name: prop.name.text.clone(),
                ty,
                mutable: prop.mutable,
            });
        }
        self.classes[id].representation = hir::ClassRepresentation::Declared(props);

        if let Some((base_ref, _)) = &decl.base_class
            && let Some(base_ty) = self.resolve_type_ref(base_ref)
        {
            let Type::Class(base_application) = self.types[base_ty] else {
                self.error(
                    base_ref.span,
                    format!("`{}` is not a class", self.type_name(base_ty)),
                );
                self.type_params_in_scope.clear();
                return;
            };
            let base_id = self.class_applications[base_application].template;
            if self.classes[base_id].modifier == hir::ClassModifier::Final {
                self.error(
                    base_ref.span,
                    format!(
                        "class `{}` is final and cannot be inherited",
                        self.classes[base_id].name
                    ),
                );
            } else {
                // Constructor arguments are lowered in pass 3.
                self.classes[id].base_class = Some((base_ty, Vec::new()));
            }
        }

        let interfaces = self.resolve_interface_list(&decl.interfaces);
        self.classes[id].interfaces = interfaces;
        self.type_params_in_scope.clear();
    }

    /// Resolve an interface list (`: I1, I2`) on any declaration —
    /// classes, structs and enums share the rules (spec 9.1 / 4.4.3):
    /// every name must be an interface, duplicates are dropped.
    pub(crate) fn resolve_interface_list(&mut self, refs: &[ast::TypeRef]) -> Vec<TypeId> {
        let mut interfaces = Vec::new();
        for ty_ref in refs {
            let Some(ty) = self.resolve_type_ref(ty_ref) else {
                continue;
            };
            if !matches!(self.types[ty], Type::Interface(..)) {
                self.error(
                    ty_ref.span,
                    format!("`{}` is not an interface", self.type_name(ty)),
                );
                continue;
            }
            if !interfaces.iter().any(|&other| self.types_equal(other, ty)) {
                interfaces.push(ty);
            }
        }
        interfaces
    }

    /// Resolve a member function's signature (pass 2.5). The implicit
    /// `this` is not part of the `FnSig` (calls are checked against the
    /// declared parameters only); it becomes `params[0]` of the
    /// `hir::Function` when the body (or the parameter-only body of a
    /// bodyless declaration) is built.
    pub(crate) fn resolve_method_signature(
        &mut self,
        id: FunctionId,
        decl: &ast::FunctionDecl,
        owner: Owner,
    ) {
        let short = decl.name.text.clone();
        if !decl.type_params.is_empty() {
            if matches!(owner, Owner::Interface(_)) {
                self.error(
                    decl.name.span,
                    format!("interface method `{short}` cannot declare method type parameters"),
                );
            }
            if matches!(owner, Owner::Class(_)) && decl.modifier != ast::MethodModifier::Final {
                self.error(
                    decl.name.span,
                    format!("generic member function `{short}` in a class must be final"),
                );
            }
            if decl.is_override {
                self.error(
                    decl.name.span,
                    format!("generic member function `{short}` cannot be an override"),
                );
            }
        }
        self.check_method_body_shape(id, decl, owner);

        let mut type_params = self.owner_type_params(owner);
        let owner_type_param_count = type_params.len();
        for param in &decl.type_params {
            if type_params
                .iter()
                .any(|existing| existing.name == param.name.text)
            {
                self.error(
                    param.span,
                    format!("duplicate type parameter `{}`", param.name.text),
                );
                continue;
            }
            let parameter = self.fresh_type_param(type_params.len());
            let param = crate::lower_type_param_decl(param, parameter);
            type_params.push(param.clone());
        }
        type_params = self.resolve_type_parameter_constraints(
            type_params,
            owner_type_param_count,
            &decl.type_params,
            decl.where_clause.as_ref(),
            "method",
        );
        let mut owner_parameters = type_params.clone();
        let method_parameters = owner_parameters.split_off(owner_type_param_count);
        self.register_method_parameters(id, owner_parameters, method_parameters);
        self.type_params_in_scope = type_params.clone();
        let mut params = Vec::with_capacity(decl.params.len());
        for param in &decl.params {
            if let Some(ty) = self.resolve_type_ref(&param.ty) {
                params.push(FnParam {
                    name: param.name.clone(),
                    ty,
                });
            }
        }
        let return_ty = match &decl.return_ty {
            Some(ty_ref) => self.resolve_type_ref(ty_ref).unwrap_or(self.unit),
            None => self.unit,
        };
        self.type_params_in_scope.clear();

        self.functions[id].return_ty = return_ty;
        self.signatures.insert(
            id,
            FnSig {
                is_suspend: decl.is_suspend,
                attributes: self.functions[id].attributes,
                owner_type_param_count,
                type_params,
                params,
                return_ty,
            },
        );

        // Bodyless declarations get their parameter-only body here;
        // concrete methods are lowered in pass 3.
        let host_ty = self.owner_ty(owner);
        if decl.modifier == ast::MethodModifier::Abstract || matches!(owner, Owner::Interface(_)) {
            let (body, _) = self.build_params_only_body(id, host_ty);
            self.functions[id].kind = hir::FunctionKind::User(body);
        }
    }

    /// Body-shape rules for member declarations: `abstract` only in
    /// abstract classes, interface methods always bodyless, concrete
    /// methods always with a body.
    fn check_method_body_shape(&mut self, id: FunctionId, decl: &ast::FunctionDecl, owner: Owner) {
        if matches!(self.functions[id].kind, hir::FunctionKind::Intrinsic(_)) {
            return;
        }
        let short = decl.name.text.clone();
        match owner {
            Owner::Interface(_) => {
                if decl.is_override {
                    self.error(
                        decl.name.span,
                        format!("`{short}` is marked `override` but does not override any method"),
                    );
                }
                if !matches!(decl.body, ast::FunctionBody::None) {
                    self.error(
                        decl.name.span,
                        format!(
                            "interface method `{}` must not have a body",
                            self.functions[id].name
                        ),
                    );
                }
            }
            Owner::Class(class_id) => {
                let abstract_class =
                    self.classes[class_id].modifier == hir::ClassModifier::Abstract;
                if decl.modifier == ast::MethodModifier::Abstract && !abstract_class {
                    self.error(
                        decl.name.span,
                        format!("abstract function `{short}` is only allowed in abstract classes"),
                    );
                }
                if decl.modifier == ast::MethodModifier::Open
                    && !decl.is_override
                    && self.classes[class_id].modifier == hir::ClassModifier::Final
                {
                    self.error(
                        decl.name.span,
                        format!(
                            "open function `{short}` is only allowed in open or abstract classes"
                        ),
                    );
                }
                match (&decl.body, decl.modifier == ast::MethodModifier::Abstract) {
                    (ast::FunctionBody::None, false) => self.error(
                        decl.name.span,
                        format!("function `{short}` must have a body"),
                    ),
                    (ast::FunctionBody::Block(_) | ast::FunctionBody::Expr(_), true) => self.error(
                        decl.name.span,
                        format!("abstract function `{short}` must not have a body"),
                    ),
                    _ => {}
                }
            }
            Owner::Struct(_) | Owner::Enum(_) => {
                if decl.modifier == ast::MethodModifier::Abstract {
                    self.error(
                        decl.name.span,
                        format!("abstract function `{short}` is only allowed in abstract classes"),
                    );
                }
                if decl.modifier == ast::MethodModifier::Open && !decl.is_override {
                    self.error(
                        decl.name.span,
                        format!("open function `{short}` is only allowed in class declarations"),
                    );
                }
                // `override` on a value-type method is checked in pass
                // 2.75 (it is required exactly when the method
                // implements an interface method, DESIGN.md 5.2).
                if matches!(decl.body, ast::FunctionBody::None) {
                    self.error(
                        decl.name.span,
                        format!("function `{short}` must have a body"),
                    );
                }
            }
        }
    }

    /// The body of a bodyless declaration (interface / abstract
    /// method): locals for `this` and the declared parameters, no
    /// statements. Fills `Function::params` (receiver first) and
    /// returns a second copy of the declared-parameter list for the
    /// interface's `MethodSig` (`hir::Param` is not `Clone`; both
    /// copies refer to the same locals of this body).
    fn build_params_only_body(
        &mut self,
        id: FunctionId,
        host_ty: TypeId,
    ) -> (hir::Body, Vec<hir::Param>) {
        let sig = self.signatures[&id].clone();
        let this = self.alloc_local("this".to_string(), host_ty, false);
        let mut params = vec![hir::Param {
            name: "this".to_string(),
            ty: host_ty,
            local: this,
        }];
        let mut declared = Vec::with_capacity(sig.params.len());
        for param in &sig.params {
            let local = self.alloc_local(param.name.text.clone(), param.ty, false);
            params.push(hir::Param {
                name: param.name.text.clone(),
                ty: param.ty,
                local,
            });
            declared.push(hir::Param {
                name: param.name.text.clone(),
                ty: param.ty,
                local,
            });
        }
        self.functions[id].params = params;
        let body = hir::Body {
            locals: std::mem::take(&mut self.locals),
            statements: Vec::new(),
        };
        (body, declared)
    }

    /// Inheritance checks (pass 2.75): cycles, property shadowing,
    /// `override` rules and interface implementation — for classes and
    /// for value types implementing interfaces (spec 4.4.3).
    pub(crate) fn check_inheritance(
        &mut self,
        pending_classes: &[(ClassId, &ast::ClassDecl, usize)],
        pending_structs: &[(hir::StructId, &ast::StructDecl, usize)],
        pending_enums: &[(hir::EnumId, &ast::EnumDecl, usize)],
        pending_methods: &[(FunctionId, &ast::FunctionDecl, usize, Owner)],
    ) {
        for &(id, decl, file_index) in pending_classes {
            self.current_file = file_index;
            self.check_inheritance_cycle(id, decl);
            self.check_property_shadowing(id, decl);
        }
        for &(id, decl, file_index, owner) in pending_methods {
            self.current_file = file_index;
            self.check_override_rules(id, decl, owner);
        }
        for &(id, decl, file_index) in pending_classes {
            self.current_file = file_index;
            self.check_interface_implementation(id, decl);
        }
        for &(id, decl, file_index) in pending_structs {
            self.current_file = file_index;
            self.check_value_interface_implementation(Owner::Struct(id), decl.span);
        }
        for &(id, decl, file_index) in pending_enums {
            self.current_file = file_index;
            self.check_value_interface_implementation(Owner::Enum(id), decl.span);
        }
    }

    /// A class may not directly or indirectly inherit from itself.
    fn check_inheritance_cycle(&mut self, id: ClassId, decl: &ast::ClassDecl) {
        let mut seen = vec![id];
        let mut current = id;
        while let Some((base_ty, _)) = self.classes[current].base_class {
            let Type::Class(base_application) = self.types[base_ty] else {
                unreachable!("resolved class bases are class applications")
            };
            let base = self.class_applications[base_application].template;
            if seen.contains(&base) {
                let name = self.classes[id].name.clone();
                self.error(
                    decl.span,
                    format!("class `{name}` directly or indirectly inherits from itself"),
                );
                return;
            }
            seen.push(base);
            current = base;
        }
    }

    /// M6 simplification: a constructor property may not reuse the name
    /// of a base-class property (no field shadowing).
    fn check_property_shadowing(&mut self, id: ClassId, decl: &ast::ClassDecl) {
        let Some((base_ty, _)) = self.classes[id].base_class else {
            return;
        };
        let Type::Class(base_application) = self.types[base_ty] else {
            unreachable!("resolved class bases are class applications")
        };
        let base = self.class_applications[base_application].template;
        for prop in &decl.constructor {
            if let Some((declaring, _, _, _)) = self.find_class_field(base, &prop.name.text) {
                let base_name = self.classes[declaring].name.clone();
                self.error(
                    prop.name.span,
                    format!(
                        "property `{}` of class `{}` shadows a property of base class `{base_name}`",
                        prop.name.text, decl.name.text
                    ),
                );
            }
        }
    }

    /// The `override` rules for one member function: an overriding
    /// method must be marked, a marked method must override. For a
    /// class the candidates are the base chain's methods and the
    /// methods of every interface the class (or a base class)
    /// implements; for a value type they are the methods of its own
    /// interface list (spec 4.4.3 — implementations require the
    /// modifier there too, DESIGN.md 5.2). Interface methods have no
    /// candidates in M6 (no superinterfaces).
    fn check_override_rules(&mut self, id: FunctionId, decl: &ast::FunctionDecl, owner: Owner) {
        let candidates: Vec<(FunctionId, Vec<TypeId>)> = match owner {
            Owner::Class(class_id) => {
                let mut candidates: Vec<_> = self
                    .base_chain_methods(class_id)
                    .into_iter()
                    .map(|candidate| {
                        let arguments = match candidate.owner {
                            crate::CallableCandidateOwner::Method(owner) => {
                                self.method_owner_arguments(owner).to_vec()
                            }
                            crate::CallableCandidateOwner::Function { owner_arguments } => {
                                owner_arguments
                            }
                        };
                        (candidate.function, arguments)
                    })
                    .collect();
                for interface_ty in self.class_interfaces_all(class_id) {
                    let (iface, args) = self.interface_application(interface_ty);
                    candidates.extend(
                        self.interface_methods[&iface]
                            .iter()
                            .copied()
                            .map(|method| (method, args.clone())),
                    );
                }
                candidates
            }
            Owner::Struct(struct_id) => {
                self.interface_method_candidates(&self.structs[struct_id].interfaces.clone())
            }
            Owner::Enum(enum_id) => {
                self.interface_method_candidates(&self.enums[enum_id].interfaces.clone())
            }
            Owner::Interface(_) => return,
        };
        let sig = self.signatures[&id].clone();
        let short = decl.name.text.clone();
        let overrides = candidates
            .iter()
            .find(|(candidate, args)| {
                self.same_instantiated_signature(*candidate, &short, &sig, args)
            })
            .cloned();
        if overrides.is_none()
            && let Some((candidate, _)) = candidates.iter().find(|(candidate, args)| {
                self.same_instantiated_signature_shape(*candidate, &short, &sig, args)
            })
        {
            let target = self.functions[*candidate].name.clone();
            self.error(
                decl.name.span,
                format!("`{short}` must have the same `suspend` modifier as `{target}`"),
            );
            return;
        }
        if let Some((candidate, _)) = overrides.as_ref()
            && matches!(self.function_owner.get(candidate), Some(Owner::Class(_)))
            && self.functions[*candidate]
                .method
                .is_some_and(|method| method.modifier == hir::MethodModifier::Final)
        {
            let owner = self.functions[*candidate].name.clone();
            self.error(
                decl.name.span,
                format!("`{short}` cannot override final method `{owner}`"),
            );
            return;
        }
        match (overrides, decl.is_override) {
            (Some((candidate, _)), false) => {
                let owner = self.functions[candidate].name.clone();
                self.error(
                    decl.name.span,
                    format!("`{short}` overrides `{owner}` and must be marked `override`"),
                );
            }
            (None, true) => {
                self.error(
                    decl.name.span,
                    format!("`{short}` is marked `override` but does not override any method"),
                );
            }
            _ => {}
        }
    }

    /// Every method of every implemented interface (including
    /// interfaces inherited from base classes) must have a
    /// same-signature concrete method on the class or its base chain.
    /// Abstract classes may leave methods unimplemented.
    fn check_interface_implementation(&mut self, id: ClassId, decl: &ast::ClassDecl) {
        self.classes[id].interface_implementations.clear();
        let abstract_class = self.classes[id].modifier == hir::ClassModifier::Abstract;
        let class_name = self.classes[id].name.clone();
        for interface_ty in self.class_interfaces_all(id) {
            let (iface, _args) = self.interface_application(interface_ty);
            let application = match self.types[interface_ty] {
                Type::Interface(application) => application,
                _ => unreachable!("class interface closure contains interface applications"),
            };
            let methods = self.interface_member_instances(application);
            let mut implementations = Vec::with_capacity(methods.len());
            for (member, method, member_arguments) in methods {
                if self.functions[method].method_type_param_count() != 0 {
                    // Interface methods with their own type parameters are
                    // rejected while their declarations are resolved.  Do
                    // not manufacture a dispatch application for that
                    // invalid declaration during error recovery.
                    continue;
                }
                let qualified = self.functions[method].name.clone();
                let sig = self.instantiated_signature(method, &member_arguments, &[]);
                let short = qualified.rsplit('.').next().expect("methods are qualified");
                let own_owner =
                    hir::MethodOwnerApplication::Class(self.classes[id].self_application);
                let mut candidates = self.base_chain_methods(id);
                candidates.extend(
                    self.classes[id]
                        .methods
                        .iter()
                        .copied()
                        .map(|function| crate::CallableCandidate::method(function, own_owner)),
                );
                let implemented = candidates.into_iter().find(|candidate| {
                    let owner_arguments = match &candidate.owner {
                        crate::CallableCandidateOwner::Method(owner) => {
                            self.method_owner_arguments(*owner).to_vec()
                        }
                        crate::CallableCandidateOwner::Function { owner_arguments } => {
                            owner_arguments.clone()
                        }
                    };
                    self.same_instantiated_signature(
                        candidate.function,
                        short,
                        &sig,
                        &owner_arguments,
                    )
                });
                match implemented {
                    Some(candidate)
                        if abstract_class || !self.is_abstract_method(candidate.function) =>
                    {
                        let crate::CallableCandidateOwner::Method(owner) = candidate.owner else {
                            unreachable!("interface implementations are methods")
                        };
                        let application = self.record_method_application(candidate.function, owner);
                        implementations.push(hir::InterfaceMethodImplementation {
                            member,
                            target: hir::InterfaceImplementationTarget::Method(application),
                        });
                    }
                    _ if abstract_class => {
                        implementations.push(hir::InterfaceMethodImplementation {
                            member,
                            target: hir::InterfaceImplementationTarget::Subclass,
                        });
                    }
                    _ => {
                        let iface_name = self.interfaces[iface].name.clone();
                        self.error(
                        decl.span,
                        format!(
                            "class `{class_name}` does not implement interface method `{iface_name}.{short}`"
                        ),
                    );
                    }
                }
            }
            self.classes[id]
                .interface_implementations
                .push(hir::InterfaceImplementation {
                    interface: application,
                    methods: implementations,
                });
        }
    }

    /// Every method of every interface a value type implements must
    /// have a same-signature method among the value type's own methods
    /// (spec 4.4.3; value types have no base chain to inherit from,
    /// and their methods are always concrete).
    fn check_value_interface_implementation(&mut self, owner: Owner, span: ast::Span) {
        let (declared_interfaces, own_methods): (Vec<TypeId>, Vec<FunctionId>) = match owner {
            Owner::Struct(id) => (
                self.structs[id].interfaces.clone(),
                self.structs[id].methods.clone(),
            ),
            Owner::Enum(id) => (
                self.enums[id].interfaces.clone(),
                self.enums[id].methods.clone(),
            ),
            // Only called for value types.
            Owner::Class(_) | Owner::Interface(_) => return,
        };
        let mut interfaces = Vec::new();
        for interface in declared_interfaces {
            self.append_interface_closure(interface, &mut interfaces);
        }
        match owner {
            Owner::Struct(id) => self.structs[id].interface_implementations.clear(),
            Owner::Enum(id) => self.enums[id].interface_implementations.clear(),
            Owner::Class(_) | Owner::Interface(_) => {}
        }
        for interface_ty in interfaces {
            let (iface, _args) = self.interface_application(interface_ty);
            let application = match self.types[interface_ty] {
                Type::Interface(application) => application,
                _ => unreachable!("value interface list contains interface applications"),
            };
            let methods = self.interface_member_instances(application);
            let mut implementations = Vec::with_capacity(methods.len());
            for (member, method, member_arguments) in methods {
                if self.functions[method].method_type_param_count() != 0 {
                    // The declaration-site diagnostic is authoritative;
                    // an illegal generic interface member has no itable
                    // identity for conformance recovery to complete.
                    continue;
                }
                let qualified = self.functions[method].name.clone();
                let sig = self.instantiated_signature(method, &member_arguments, &[]);
                let short = qualified.rsplit('.').next().expect("methods are qualified");
                let implemented = own_methods
                    .iter()
                    .copied()
                    .find(|&candidate| self.same_signature(candidate, short, &sig));
                if let Some(function) = implemented {
                    let owner_application =
                        self.method_owner_application(owner, self.owner_type_args(owner));
                    let application = self.record_method_application(function, owner_application);
                    implementations.push(hir::InterfaceMethodImplementation {
                        member,
                        target: hir::InterfaceImplementationTarget::Method(application),
                    });
                } else {
                    let iface_name = self.interfaces[iface].name.clone();
                    let host = owner.describe(self);
                    self.error(
                        span,
                        format!(
                            "{host} does not implement interface method `{iface_name}.{short}`"
                        ),
                    );
                }
            }
            let implementation = hir::InterfaceImplementation {
                interface: application,
                methods: implementations,
            };
            match owner {
                Owner::Struct(id) => self.structs[id]
                    .interface_implementations
                    .push(implementation),
                Owner::Enum(id) => self.enums[id]
                    .interface_implementations
                    .push(implementation),
                Owner::Class(_) | Owner::Interface(_) => unreachable!(),
            }
        }
    }

    /// Whether the method is `abstract` (bodyless class method).
    fn is_abstract_method(&self, id: FunctionId) -> bool {
        self.functions[id]
            .method
            .is_some_and(|method| method.modifier == hir::MethodModifier::Abstract)
    }

    /// Signature equality for override / implementation matching:
    /// name, parameter types and return type all equal (overloads
    /// only match exactly, M7).
    fn same_signature(&self, candidate: FunctionId, name: &str, sig: &FnSig) -> bool {
        self.same_signature_shape(candidate, name, sig)
            && self.functions[candidate].is_suspend == sig.is_suspend
            && self.functions[candidate].attributes == sig.attributes
    }

    fn same_signature_shape(&self, candidate: FunctionId, name: &str, sig: &FnSig) -> bool {
        let function = &self.functions[candidate];
        if function.name.rsplit('.').next() != Some(name) {
            return false;
        }
        let Some(candidate_sig) = self.signatures.get(&candidate) else {
            return false;
        };
        let candidate_own_count =
            candidate_sig.type_params.len() - candidate_sig.owner_type_param_count;
        let expected_own_count = sig.type_params.len() - sig.owner_type_param_count;
        candidate_own_count == expected_own_count
            && candidate_sig.params.len() == sig.params.len()
            && candidate_sig
                .params
                .iter()
                .zip(&sig.params)
                .all(|(a, b)| self.types_equal(a.ty, b.ty))
            && self.types_equal(candidate_sig.return_ty, sig.return_ty)
    }

    fn instantiated_signature(
        &mut self,
        method: FunctionId,
        owner_arguments: &[TypeId],
        target_method_parameters: &[hir::TypeParamDecl],
    ) -> FnSig {
        let sig = self.signatures[&method].clone();
        let (owner_parameters, method_parameters) = match &self.functions[method].genericity {
            hir::FunctionGenericity::Plain => (Vec::new(), Vec::new()),
            hir::FunctionGenericity::OwnerParameterizedMethod {
                owner_parameters, ..
            } => (owner_parameters.clone(), Vec::new()),
            hir::FunctionGenericity::GenericMethod {
                owner_parameters,
                method_parameters,
                ..
            } => (
                owner_parameters.clone(),
                method_parameters.iter().cloned().collect(),
            ),
            hir::FunctionGenericity::Generic { .. } => {
                unreachable!("nominal methods do not use generic-function identity")
            }
        };
        assert_eq!(owner_parameters.len(), owner_arguments.len());
        assert_eq!(method_parameters.len(), target_method_parameters.len());
        let mut bindings = owner_parameters
            .iter()
            .zip(owner_arguments.iter().copied())
            .map(|(parameter, argument)| (parameter.id, argument))
            .collect::<Vec<_>>();
        let target_method_types = target_method_parameters
            .iter()
            .map(|parameter| (parameter.id, self.intern_type(Type::Param(parameter.id))))
            .collect::<Vec<_>>();
        bindings.extend(
            method_parameters
                .iter()
                .zip(&target_method_types)
                .map(|(source, (_, target))| (source.id, *target)),
        );
        FnSig {
            is_suspend: sig.is_suspend,
            attributes: sig.attributes,
            owner_type_param_count: 0,
            type_params: target_method_parameters.to_vec(),
            params: sig
                .params
                .into_iter()
                .map(|param| FnParam {
                    name: param.name,
                    ty: self.instantiate_method_ty(param.ty, &bindings),
                })
                .collect(),
            return_ty: self.instantiate_method_ty(sig.return_ty, &bindings),
        }
    }

    fn same_instantiated_signature(
        &mut self,
        candidate: FunctionId,
        name: &str,
        sig: &FnSig,
        args: &[TypeId],
    ) -> bool {
        self.same_instantiated_signature_shape(candidate, name, sig, args)
            && self.functions[candidate].is_suspend == sig.is_suspend
    }

    fn same_instantiated_signature_shape(
        &mut self,
        candidate: FunctionId,
        name: &str,
        sig: &FnSig,
        args: &[TypeId],
    ) -> bool {
        let target_method_parameters = &sig.type_params[sig.owner_type_param_count..];
        if self.functions[candidate].method_type_param_count() != target_method_parameters.len() {
            return false;
        }
        let candidate_sig = self.instantiated_signature(candidate, args, target_method_parameters);
        let candidate_own_count =
            candidate_sig.type_params.len() - candidate_sig.owner_type_param_count;
        let expected_own_count = sig.type_params.len() - sig.owner_type_param_count;
        self.functions[candidate].name.rsplit('.').next() == Some(name)
            && candidate_own_count == expected_own_count
            && candidate_sig.params.len() == sig.params.len()
            && candidate_sig
                .params
                .iter()
                .zip(&sig.params)
                .all(|(a, b)| self.types_equal(a.ty, b.ty))
            && self.types_equal(candidate_sig.return_ty, sig.return_ty)
    }

    fn interface_application(&self, ty: TypeId) -> (hir::InterfaceId, Vec<TypeId>) {
        match &self.types[ty] {
            Type::Interface(application) => {
                let application = &self.interface_applications[*application];
                (application.template, application.arguments.clone())
            }
            _ => unreachable!("resolved interface lists only contain interface applications"),
        }
    }

    fn interface_method_candidates(
        &mut self,
        interfaces: &[TypeId],
    ) -> Vec<(FunctionId, Vec<TypeId>)> {
        let mut candidates = Vec::new();
        for &interface_ty in interfaces {
            let Type::Interface(application) = self.types[interface_ty] else {
                unreachable!("resolved interface lists contain interface applications")
            };
            candidates.extend(
                self.interface_member_instances(application)
                    .into_iter()
                    .map(|(_, method, arguments)| (method, arguments)),
            );
        }
        candidates
    }

    /// Lower the base-constructor delegation arguments of
    /// `class B(...) : A(args)` (pass 3): arity and types are checked
    /// against A's own constructor properties. The arguments are
    /// lowered in an empty scope — constructor properties are not in
    /// scope there (an M6 simplification: HIR has no body to host the
    /// locals a reference would need).
    pub(crate) fn lower_base_args(&mut self, id: ClassId, decl: &ast::ClassDecl) {
        let Some((_base_ty, args)) = &decl.base_class else {
            return;
        };
        let resolved_base = match self.classes[id].base_class.as_ref() {
            Some((base, _)) => *base,
            None => return, // the clause was rejected in pass 2
        };
        let Type::Class(base_application) = self.types[resolved_base] else {
            unreachable!("resolved class bases are class applications")
        };
        let base_application = self.class_applications[base_application].clone();
        let base_id = base_application.template;
        let base_type_args = base_application.arguments;
        let base_name = self.classes[base_id].name.clone();
        let base_constructor = self.classes[base_id].semantic_constructor().to_vec();
        let props: Vec<(String, TypeId)> = base_constructor
            .iter()
            .map(|field| {
                (
                    field.name.clone(),
                    self.instantiate_ty(field.ty, &base_type_args),
                )
            })
            .collect();
        if args.len() != props.len() {
            let expected = props.len();
            let supplied = args.len();
            let noun = if expected == 1 {
                "argument"
            } else {
                "arguments"
            };
            self.error(
                decl.span,
                format!(
                    "constructor of class `{base_name}` takes exactly {expected} {noun}, but {supplied} were supplied"
                ),
            );
            return;
        }
        let mut lowered_args = Vec::with_capacity(args.len());
        let mut ok = true;
        self.type_params_in_scope = self.classes[id].type_params.clone();
        self.constructor_params_in_scope = self.classes[id]
            .semantic_constructor()
            .iter()
            .map(|parameter| (parameter.name.clone(), (parameter.parameter, parameter.ty)))
            .collect();
        self.push_scope();
        self.push_suspension_context(SuspensionContext::Forbidden(
            ForbiddenSuspendContext::ConstructorDelegation,
        ));
        self.current_return_ty = self.unit;
        self.current_fn_name = format!("<init {base_name}>");
        for (arg, (prop_name, prop_ty)) in args.iter().zip(&props) {
            let mut sink = Vec::new();
            let Some(arg) = self.lower_expr(arg, &mut sink, Some(*prop_ty)) else {
                ok = false;
                break;
            };
            if !sink.is_empty() {
                self.error(
                    arg.span,
                    "`?.` and `?:` are not allowed in base constructor arguments".to_string(),
                );
                ok = false;
                break;
            }
            if !self.is_subtype(arg.ty, *prop_ty) {
                let expected = self.type_name(*prop_ty);
                let found = self.type_name(arg.ty);
                self.error(
                    arg.span,
                    format!(
                        "argument for constructor property `{prop_name}` of class `{base_name}` must be of type {expected}, found {found}"
                    ),
                );
                ok = false;
                break;
            }
            lowered_args.push(self.adapt_to(arg, *prop_ty));
        }
        self.pop_suspension_context();
        self.pop_scope();
        self.constructor_params_in_scope.clear();
        self.type_params_in_scope.clear();
        if ok {
            self.classes[id].base_class = Some((resolved_base, lowered_args));
        }
    }

    // --- member lookup helpers ---

    fn direct_base_class(&self, class: ClassId) -> Option<ClassId> {
        let (base, _) = self.classes[class].base_class.as_ref()?;
        let Type::Class(application) = self.types[*base] else {
            unreachable!("resolved class bases are class applications")
        };
        Some(self.class_applications[application].template)
    }

    /// Every interface implemented by class `c` or its base classes,
    /// deduplicated, own list first (cycle-safe).
    pub(crate) fn class_interfaces_all(&mut self, c: ClassId) -> Vec<TypeId> {
        self.class_interfaces_for_application(self.classes[c].self_application)
    }

    /// The methods of the base classes of `c`, nearest base first
    /// (cycle-safe).
    fn base_chain_methods(&mut self, c: ClassId) -> Vec<crate::CallableCandidate> {
        let mut result = Vec::new();
        let mut current = self.classes[c].self_application;
        let mut seen = vec![current];
        loop {
            let application = self.class_applications[current].clone();
            let Some((base, _)) = self.classes[application.template].base_class.clone() else {
                break;
            };
            let base = self.instantiate_ty(base, &application.arguments);
            let Type::Class(base_application) = self.types[base] else {
                unreachable!("class bases are resolved class applications")
            };
            if seen.contains(&base_application) {
                break;
            }
            seen.push(base_application);
            let base = self.class_applications[base_application].clone();
            result.extend(
                self.classes[base.template]
                    .methods
                    .iter()
                    .copied()
                    .map(|function| {
                        crate::CallableCandidate::method(
                            function,
                            hir::MethodOwnerApplication::Class(base_application),
                        )
                    }),
            );
            current = base_application;
        }
        result
    }

    /// The total number of constructor properties in the base chain of
    /// `c` — the layout offset of `c`'s own properties (base fields
    /// prefix, cycle-safe).
    pub(crate) fn base_field_total(&self, c: ClassId) -> u32 {
        let mut total = 0;
        let mut seen = vec![c];
        let mut current = Some(c);
        while let Some(id) = current {
            current = match self.direct_base_class(id) {
                Some(base) if !seen.contains(&base) => {
                    seen.push(base);
                    total += self.classes[base].semantic_constructor().len() as u32;
                    Some(base)
                }
                _ => None,
            };
        }
        total
    }

    /// Find a constructor property by name on class `c` or its base
    /// chain. Returns the declaring class, the absolute layout index
    /// (base fields prefix + own fields, consecutive), the type and
    /// the mutability.
    pub(crate) fn find_class_field(
        &self,
        c: ClassId,
        name: &str,
    ) -> Option<(ClassId, u32, TypeId, bool)> {
        if let Some(index) = self.classes[c]
            .semantic_constructor()
            .iter()
            .position(|field| field.name == name)
        {
            let abs = self.base_field_total(c) + index as u32;
            let ty = self.classes[c].semantic_constructor()[index].ty;
            let mutable = self.classes[c].semantic_constructor()[index].mutable;
            return Some((c, abs, ty, mutable));
        }
        let base = self.direct_base_class(c)?;
        self.find_class_field(base, name)
    }

    /// Field lookup on a complete class application. The declaration/layout
    /// identity remains the declaring `ClassId`, while the returned field type
    /// is fully substituted through every generic base application.
    pub(crate) fn find_class_application_field(
        &mut self,
        application: hir::ClassApplicationId,
        name: &str,
    ) -> Option<(hir::ClassApplicationId, u32, TypeId, bool)> {
        let application_value = self.class_applications[application].clone();
        let class = application_value.template;
        if let Some(index) = self.classes[class]
            .semantic_constructor()
            .iter()
            .position(|field| field.name == name)
        {
            let abs = self.base_field_total(class) + index as u32;
            let field_ty = self.classes[class].semantic_constructor()[index].ty;
            let ty = self.instantiate_ty(field_ty, &application_value.arguments);
            let mutable = self.classes[class].semantic_constructor()[index].mutable;
            return Some((application, abs, ty, mutable));
        }
        let (base, _) = self.classes[class].base_class.clone()?;
        let base = self.instantiate_ty(base, &application_value.arguments);
        let Type::Class(base_application) = self.types[base] else {
            unreachable!("resolved class bases are class applications")
        };
        self.find_class_application_field(base_application, name)
    }

    /// All visible methods named `name` on a receiver type (M7 overload
    /// candidates). A class contributes its own methods followed by
    /// the base chain nearest-first; an overriding signature suppresses
    /// the corresponding base declaration so one dynamic-dispatch slot
    /// never appears as two ambiguous overload candidates.
    ///
    /// An `Any` receiver additionally resolves the three `Any`
    /// members (`equals` / `hashCode` / `toString`), synthesized by
    /// `synthesize_any_members`: mir-lower dispatches them virtually
    /// through the fixed vtable prefix (slots 0..2), so every runtime
    /// value behind the `Any` (boxed value types, class objects)
    /// reaches its per-type implementation. Narrower static types
    /// deliberately do not get this fallback yet: mir-lower only
    /// virtualizes these three on an exactly-`Any` receiver (a class
    /// receiver would come out as a direct call, an interface
    /// receiver has no matching itable slot).
    pub(crate) fn methods_by_name(
        &mut self,
        ty: TypeId,
        name: &str,
    ) -> Vec<crate::CallableCandidate> {
        let mut declared = Vec::<(crate::CallableCandidate, usize, usize)>::new();
        match self.types[ty].clone() {
            Type::Class(mut application) => {
                let mut depth = 0;
                loop {
                    let application_value = self.class_applications[application].clone();
                    let class = application_value.template;
                    declared.extend(self.classes[class].methods.iter().copied().map(|function| {
                        (
                            crate::CallableCandidate::method(
                                function,
                                hir::MethodOwnerApplication::Class(application),
                            ),
                            depth,
                            0,
                        )
                    }));
                    let Some((base, _)) = self.classes[class].base_class.clone() else {
                        break;
                    };
                    let base = self.instantiate_ty(base, &application_value.arguments);
                    let Type::Class(base_application) = self.types[base] else {
                        unreachable!("resolved class bases are class applications")
                    };
                    application = base_application;
                    depth += 1;
                }
            }
            Type::Interface(application) => {
                self.collect_interface_method_candidates(
                    application,
                    0,
                    0,
                    None,
                    &mut Vec::new(),
                    &mut declared,
                );
            }
            Type::Struct(application) => {
                let application_value = self.struct_applications[application].clone();
                declared.extend(
                    self.structs[application_value.template]
                        .methods
                        .iter()
                        .copied()
                        .map(|function| {
                            (
                                crate::CallableCandidate::method(
                                    function,
                                    hir::MethodOwnerApplication::Struct(application),
                                ),
                                0,
                                0,
                            )
                        }),
                );
            }
            Type::Ptr(pointee) => {
                if let Some(owner) = self.ffi_ptr {
                    let application = self.struct_application_id(owner, vec![pointee]);
                    declared.extend(self.structs[owner].methods.iter().copied().map(|function| {
                        (
                            crate::CallableCandidate::method(
                                function,
                                hir::MethodOwnerApplication::Struct(application),
                            ),
                            0,
                            0,
                        )
                    }));
                }
            }
            Type::Enum(application) => {
                let application_value = self.enum_applications[application].clone();
                declared.extend(
                    self.enums[application_value.template]
                        .methods
                        .iter()
                        .copied()
                        .map(|function| {
                            (
                                crate::CallableCandidate::method(
                                    function,
                                    hir::MethodOwnerApplication::Enum(application),
                                ),
                                0,
                                0,
                            )
                        }),
                );
            }
            Type::Any => {
                if let Some(function) = self.any_method(name) {
                    declared.push((
                        crate::CallableCandidate::method(
                            function,
                            hir::MethodOwnerApplication::Any,
                        ),
                        0,
                        0,
                    ));
                }
            }
            Type::Param(receiver_parameter) => {
                let bounds = self
                    .type_params_in_scope
                    .iter()
                    .find(|parameter| parameter.id == receiver_parameter)
                    .expect("the receiver parameter is in the active declaration scope")
                    .interface_bounds()
                    .to_vec();
                for (root, bound) in bounds.into_iter().enumerate() {
                    self.collect_interface_method_candidates(
                        bound.application,
                        0,
                        root,
                        Some((receiver_parameter, bound.application)),
                        &mut Vec::new(),
                        &mut declared,
                    );
                }
            }
            _ => {}
        }

        let mut visible = Vec::new();
        for (candidate, depth, root) in declared {
            if self.functions[candidate.function].name.rsplit('.').next() != Some(name) {
                continue;
            }
            let mut duplicate = false;
            for (existing, existing_depth, existing_root) in &visible {
                if *existing_root == root
                    && *existing_depth < depth
                    && self.same_applied_method_signature(existing, &candidate, name)
                {
                    duplicate = true;
                    break;
                }
            }
            if !duplicate {
                visible.push((candidate, depth, root));
            }
        }
        visible
            .into_iter()
            .map(|(candidate, _, _)| candidate)
            .collect()
    }

    fn collect_interface_method_candidates(
        &mut self,
        application: hir::InterfaceApplicationId,
        depth: usize,
        root: usize,
        bound: Option<(hir::TypeParamId, hir::InterfaceApplicationId)>,
        seen: &mut Vec<hir::InterfaceApplicationId>,
        out: &mut Vec<(crate::CallableCandidate, usize, usize)>,
    ) {
        if seen.contains(&application) {
            return;
        }
        seen.push(application);
        let application_value = self.interface_applications[application].clone();
        for &member in &self.interfaces[application_value.template].methods {
            let function = self.interface_method_entities[member].function;
            let source = match bound {
                Some((receiver_parameter, bound)) => crate::CallableCandidateSource::Bound {
                    receiver_parameter,
                    bound,
                    member,
                },
                None => crate::CallableCandidateSource::Direct,
            };
            out.push((
                crate::CallableCandidate {
                    function,
                    owner: crate::CallableCandidateOwner::Method(
                        hir::MethodOwnerApplication::Interface(application),
                    ),
                    source,
                },
                depth,
                root,
            ));
        }
        for parent in self.interfaces[application_value.template].parents.clone() {
            let parent = self.interface_applications[parent].canonical_type;
            let parent = self.instantiate_ty(parent, &application_value.arguments);
            let Type::Interface(parent) = self.types[parent] else {
                unreachable!("interface parent substitutions stay interface applications")
            };
            self.collect_interface_method_candidates(parent, depth + 1, root, bound, seen, out);
        }
    }

    fn interface_member_instances(
        &mut self,
        application: hir::InterfaceApplicationId,
    ) -> Vec<(hir::InterfaceMethodId, FunctionId, Vec<TypeId>)> {
        let mut result = Vec::new();
        self.collect_interface_member_instances(application, &mut Vec::new(), &mut result);
        result
    }

    fn collect_interface_member_instances(
        &mut self,
        application: hir::InterfaceApplicationId,
        seen: &mut Vec<hir::InterfaceApplicationId>,
        out: &mut Vec<(hir::InterfaceMethodId, FunctionId, Vec<TypeId>)>,
    ) {
        if seen.contains(&application) {
            return;
        }
        seen.push(application);
        let application_value = self.interface_applications[application].clone();
        for &member in &self.interfaces[application_value.template].methods {
            out.push((
                member,
                self.interface_method_entities[member].function,
                application_value.arguments.clone(),
            ));
        }
        for parent in self.interfaces[application_value.template].parents.clone() {
            let parent = self.interface_applications[parent].canonical_type;
            let parent = self.instantiate_ty(parent, &application_value.arguments);
            let Type::Interface(parent) = self.types[parent] else {
                unreachable!("interface parent substitutions stay interface applications")
            };
            self.collect_interface_member_instances(parent, seen, out);
        }
    }

    fn same_applied_method_signature(
        &mut self,
        left: &crate::CallableCandidate,
        right: &crate::CallableCandidate,
        name: &str,
    ) -> bool {
        if self.functions[left.function].method_type_param_count()
            != self.functions[right.function].method_type_param_count()
        {
            return false;
        }
        let left_arguments = match &left.owner {
            crate::CallableCandidateOwner::Method(owner) => {
                self.method_owner_arguments(*owner).to_vec()
            }
            crate::CallableCandidateOwner::Function { owner_arguments } => owner_arguments.clone(),
        };
        let right_arguments = match &right.owner {
            crate::CallableCandidateOwner::Method(owner) => {
                self.method_owner_arguments(*owner).to_vec()
            }
            crate::CallableCandidateOwner::Function { owner_arguments } => owner_arguments.clone(),
        };
        let canonical_method_parameters = match &self.functions[left.function].genericity {
            hir::FunctionGenericity::GenericMethod {
                method_parameters, ..
            } => method_parameters.iter().cloned().collect::<Vec<_>>(),
            hir::FunctionGenericity::Plain
            | hir::FunctionGenericity::OwnerParameterizedMethod { .. } => Vec::new(),
            hir::FunctionGenericity::Generic { .. } => {
                unreachable!("member candidates do not use generic-function identity")
            }
        };
        let left_sig = self.instantiated_signature(
            left.function,
            &left_arguments,
            &canonical_method_parameters,
        );
        let right_sig = self.instantiated_signature(
            right.function,
            &right_arguments,
            &canonical_method_parameters,
        );
        self.functions[left.function].name.rsplit('.').next() == Some(name)
            && self.functions[right.function].name.rsplit('.').next() == Some(name)
            && left_sig.is_suspend == right_sig.is_suspend
            && left_sig.attributes == right_sig.attributes
            && left_sig.type_params.len() == right_sig.type_params.len()
            && left_sig.params.len() == right_sig.params.len()
            && left_sig
                .params
                .iter()
                .zip(&right_sig.params)
                .all(|(left, right)| self.types_equal(left.ty, right.ty))
            && self.types_equal(left_sig.return_ty, right_sig.return_ty)
    }

    /// The synthesized `Any` member named `name`
    /// (`synthesize_any_members`), when `name` is one of `equals` /
    /// `hashCode` / `toString`.
    fn any_method(&self, name: &str) -> Option<FunctionId> {
        let index = match name {
            "equals" => 0,
            "hashCode" => 1,
            "toString" => 2,
            _ => return None,
        };
        Some(self.any_methods[index])
    }
}
