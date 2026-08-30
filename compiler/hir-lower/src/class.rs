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
            let methods: Vec<_> = self.interfaces[interface]
                .methods
                .iter()
                .map(|method| {
                    (
                        method.name.clone(),
                        method
                            .params
                            .iter()
                            .map(|param| param.ty)
                            .collect::<Vec<_>>(),
                        method.return_ty,
                        method.span,
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
                let index = id.into_raw() as usize;
                // Parameters declared by a generic interface method follow
                // the interface's own prefix and do not participate in the
                // declaration-site variance of that prefix.
                if index >= params.len() {
                    return;
                }
                let param = &params[index];
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
            Type::Interface(id, args) => {
                let variances: Vec<_> = self.interfaces[id]
                    .type_params
                    .iter()
                    .map(|param| param.variance)
                    .collect();
                for (arg, variance) in args.into_iter().zip(variances) {
                    self.check_variance_position(
                        arg,
                        position.through(variance),
                        params,
                        method,
                        span,
                    );
                }
            }
            Type::Struct(_, args) | Type::Enum(_, args) | Type::Tuple(args) => {
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
            Type::Array(element) | Type::MutableArray(element) => {
                self.check_variance_position(element, TypePosition::Invariant, params, method, span)
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
            Type::Unit
            | Type::Int
            | Type::UInt
            | Type::Boolean
            | Type::String
            | Type::Class(_)
            | Type::Any => {}
        }
    }

    /// Resolve a class's constructor properties, base-class clause and
    /// interface list (pass 2).
    pub(crate) fn resolve_class(&mut self, id: ClassId, decl: &ast::ClassDecl) {
        let mut seen = std::collections::HashSet::new();
        let mut props = Vec::new();
        let mut mutability = Vec::new();
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
            props.push(hir::Field {
                name: prop.name.text.clone(),
                ty,
            });
            mutability.push(prop.mutable);
        }
        self.classes[id].constructor = props;
        self.class_prop_mutability.insert(id, mutability);

        if let Some((base_name, _)) = &decl.base_class {
            match self.classes_by_name.get(&base_name.text) {
                Some(&(base_id, _)) => {
                    if self.classes[base_id].modifier == hir::ClassModifier::Final {
                        self.error(
                            base_name.span,
                            format!(
                                "class `{}` is final and cannot be inherited",
                                base_name.text
                            ),
                        );
                    } else {
                        // The constructor arguments are lowered in
                        // pass 3 (`lower_base_args`).
                        self.classes[id].base_class = Some((base_id, Vec::new()));
                    }
                }
                None => {
                    let what = if self.type_namespace_conflict(&base_name.text).is_some() {
                        format!("`{}` is not a class", base_name.text)
                    } else {
                        format!("unknown type `{}`", base_name.text)
                    };
                    self.error(base_name.span, what);
                }
            }
        }

        let interfaces = self.resolve_interface_list(&decl.interfaces);
        self.classes[id].interfaces = interfaces;
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
        if !decl.type_params.is_empty()
            && matches!(owner, Owner::Class(_))
            && decl.modifier != ast::MethodModifier::Final
        {
            self.error(
                decl.name.span,
                format!("generic member function `{short}` in a class must be final"),
            );
        }
        self.check_method_body_shape(id, decl, owner);

        let mut type_params = self.owner_type_param_names(owner);
        let owner_type_param_count = type_params.len();
        let mut method_type_params = Vec::new();
        for param in &decl.type_params {
            if type_params.contains(&param.text) {
                self.error(
                    param.span,
                    format!("duplicate type parameter `{}`", param.text),
                );
                continue;
            }
            type_params.push(param.text.clone());
            method_type_params.push(param.text.clone());
        }
        if !type_params.is_empty() {
            self.register_generic(id);
        }
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

        self.functions[id].type_params = type_params.clone();
        self.functions[id].return_ty = return_ty;
        self.signatures.insert(
            id,
            FnSig {
                is_suspend: decl.is_suspend,
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
            let (body, declared) = self.build_params_only_body(id, host_ty);
            self.functions[id].kind = hir::FunctionKind::User(body);
            if let Owner::Interface(iface) = owner {
                self.interfaces[iface].methods.push(hir::MethodSig {
                    name: short,
                    is_suspend: decl.is_suspend,
                    type_params: method_type_params,
                    params: declared,
                    return_ty,
                    span: decl.span,
                });
            }
        }
    }

    /// Body-shape rules for member declarations: `abstract` only in
    /// abstract classes, interface methods always bodyless, concrete
    /// methods always with a body.
    fn check_method_body_shape(&mut self, id: FunctionId, decl: &ast::FunctionDecl, owner: Owner) {
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
        while let Some((base, _)) = self.classes[current].base_class {
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
        let Some((base, _)) = self.classes[id].base_class else {
            return;
        };
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
                    .map(|method| (method, Vec::new()))
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
        let target_owner_count = sig.owner_type_param_count;
        let short = decl.name.text.clone();
        let overrides = candidates
            .iter()
            .find(|(candidate, args)| {
                self.same_instantiated_signature(*candidate, &short, &sig, args, target_owner_count)
            })
            .cloned();
        if overrides.is_none()
            && let Some((candidate, _)) = candidates.iter().find(|(candidate, args)| {
                self.same_instantiated_signature_shape(
                    *candidate,
                    &short,
                    &sig,
                    args,
                    target_owner_count,
                )
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
        if self.classes[id].modifier == hir::ClassModifier::Abstract {
            return;
        }
        let class_name = self.classes[id].name.clone();
        for interface_ty in self.class_interfaces_all(id) {
            let (iface, args) = self.interface_application(interface_ty);
            let methods: Vec<(String, FunctionId)> = self.interface_methods[&iface]
                .iter()
                .map(|&m| (self.functions[m].name.clone(), m))
                .collect();
            for (qualified, method) in methods {
                let sig = self.instantiated_signature(method, &args, 0);
                let short = qualified.rsplit('.').next().expect("methods are qualified");
                let implemented = self
                    .base_chain_methods(id)
                    .into_iter()
                    .chain(self.class_methods[&id].iter().copied())
                    .any(|candidate| {
                        self.same_signature(candidate, short, &sig)
                            && !self.is_abstract_method(candidate)
                    });
                if !implemented {
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
    }

    /// Every method of every interface a value type implements must
    /// have a same-signature method among the value type's own methods
    /// (spec 4.4.3; value types have no base chain to inherit from,
    /// and their methods are always concrete).
    fn check_value_interface_implementation(&mut self, owner: Owner, span: ast::Span) {
        let (interfaces, own_methods): (Vec<TypeId>, Vec<FunctionId>) = match owner {
            Owner::Struct(id) => (
                self.structs[id].interfaces.clone(),
                self.struct_methods[&id].clone(),
            ),
            Owner::Enum(id) => (
                self.enums[id].interfaces.clone(),
                self.enum_methods[&id].clone(),
            ),
            // Only called for value types.
            Owner::Class(_) | Owner::Interface(_) => return,
        };
        let target_owner_count = self.owner_type_param_names(owner).len();
        for interface_ty in interfaces {
            let (iface, args) = self.interface_application(interface_ty);
            let methods: Vec<(String, FunctionId)> = self.interface_methods[&iface]
                .iter()
                .map(|&m| (self.functions[m].name.clone(), m))
                .collect();
            for (qualified, method) in methods {
                let sig = self.instantiated_signature(method, &args, target_owner_count);
                let short = qualified.rsplit('.').next().expect("methods are qualified");
                let implemented = own_methods
                    .iter()
                    .copied()
                    .any(|candidate| self.same_signature(candidate, short, &sig));
                if !implemented {
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
        args: &[TypeId],
        target_owner_count: usize,
    ) -> FnSig {
        let sig = self.signatures[&method].clone();
        let own_type_params = sig.type_params[sig.owner_type_param_count..].to_vec();
        let mut type_params = vec![String::new(); target_owner_count];
        type_params.extend(own_type_params);
        FnSig {
            is_suspend: sig.is_suspend,
            owner_type_param_count: target_owner_count,
            type_params,
            params: sig
                .params
                .into_iter()
                .map(|param| FnParam {
                    name: param.name,
                    ty: self.instantiate_method_owner_ty(
                        param.ty,
                        args,
                        sig.owner_type_param_count,
                        target_owner_count,
                    ),
                })
                .collect(),
            return_ty: self.instantiate_method_owner_ty(
                sig.return_ty,
                args,
                sig.owner_type_param_count,
                target_owner_count,
            ),
        }
    }

    fn same_instantiated_signature(
        &mut self,
        candidate: FunctionId,
        name: &str,
        sig: &FnSig,
        args: &[TypeId],
        target_owner_count: usize,
    ) -> bool {
        self.same_instantiated_signature_shape(candidate, name, sig, args, target_owner_count)
            && self.functions[candidate].is_suspend == sig.is_suspend
    }

    fn same_instantiated_signature_shape(
        &mut self,
        candidate: FunctionId,
        name: &str,
        sig: &FnSig,
        args: &[TypeId],
        target_owner_count: usize,
    ) -> bool {
        let candidate_sig = self.instantiated_signature(candidate, args, target_owner_count);
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
            Type::Interface(id, args) => (*id, args.clone()),
            _ => unreachable!("resolved interface lists only contain interface applications"),
        }
    }

    fn interface_method_candidates(&self, interfaces: &[TypeId]) -> Vec<(FunctionId, Vec<TypeId>)> {
        let mut candidates = Vec::new();
        for &interface_ty in interfaces {
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

    /// Lower the base-constructor delegation arguments of
    /// `class B(...) : A(args)` (pass 3): arity and types are checked
    /// against A's own constructor properties. The arguments are
    /// lowered in an empty scope — constructor properties are not in
    /// scope there (an M6 simplification: HIR has no body to host the
    /// locals a reference would need).
    pub(crate) fn lower_base_args(&mut self, id: ClassId, decl: &ast::ClassDecl) {
        let Some((base_name, args)) = &decl.base_class else {
            return;
        };
        let base_id = match self.classes[id].base_class.as_ref() {
            Some((base_id, _)) => *base_id,
            None => return, // the clause was rejected in pass 2
        };
        let base_name = base_name.text.clone();
        let props: Vec<(String, TypeId)> = self.classes[base_id]
            .constructor
            .iter()
            .map(|field| (field.name.clone(), field.ty))
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
        self.scopes.push();
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
        self.scopes.pop();
        if ok {
            self.classes[id].base_class = Some((base_id, lowered_args));
        }
    }

    // --- member lookup helpers ---

    /// Whether `a` inherits from `b` (a proper base-class-chain walk;
    /// cycle-safe).
    pub(crate) fn class_inherits(&self, a: ClassId, b: ClassId) -> bool {
        let mut seen = vec![a];
        let mut current = a;
        while let Some((base, _)) = self.classes[current].base_class {
            if base == b {
                return true;
            }
            if seen.contains(&base) {
                return false; // cyclic inheritance (diagnosed separately)
            }
            seen.push(base);
            current = base;
        }
        false
    }

    /// Every interface implemented by class `c` or its base classes,
    /// deduplicated, own list first (cycle-safe).
    pub(crate) fn class_interfaces_all(&self, c: ClassId) -> Vec<TypeId> {
        let mut result = Vec::new();
        let mut seen = vec![c];
        let mut current = Some(c);
        while let Some(id) = current {
            for &iface in &self.classes[id].interfaces {
                if !result.iter().any(|&other| self.types_equal(other, iface)) {
                    result.push(iface);
                }
            }
            current = match self.classes[id].base_class {
                Some((base, _)) if !seen.contains(&base) => {
                    seen.push(base);
                    Some(base)
                }
                _ => None,
            };
        }
        result
    }

    /// The methods of the base classes of `c`, nearest base first
    /// (cycle-safe).
    fn base_chain_methods(&self, c: ClassId) -> Vec<FunctionId> {
        let mut result = Vec::new();
        let mut seen = vec![c];
        let mut current = Some(c);
        while let Some(id) = current {
            current = match self.classes[id].base_class {
                Some((base, _)) if !seen.contains(&base) => {
                    seen.push(base);
                    result.extend(self.class_methods[&base].iter().copied());
                    Some(base)
                }
                _ => None,
            };
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
            current = match self.classes[id].base_class {
                Some((base, _)) if !seen.contains(&base) => {
                    seen.push(base);
                    total += self.classes[base].constructor.len() as u32;
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
            .constructor
            .iter()
            .position(|field| field.name == name)
        {
            let abs = self.base_field_total(c) + index as u32;
            let ty = self.classes[c].constructor[index].ty;
            let mutable = self.class_prop_mutability[&c][index];
            return Some((c, abs, ty, mutable));
        }
        let (base, _) = self.classes[c].base_class.as_ref()?;
        self.find_class_field(*base, name)
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
    pub(crate) fn methods_by_name(&self, ty: TypeId, name: &str) -> Vec<FunctionId> {
        let add_visible = |result: &mut Vec<FunctionId>, methods: &[FunctionId]| {
            for method in methods
                .iter()
                .copied()
                .filter(|&method| self.functions[method].name.rsplit('.').next() == Some(name))
            {
                let sig = &self.signatures[&method];
                if !result
                    .iter()
                    .copied()
                    .any(|visible| self.same_signature(visible, name, sig))
                {
                    result.push(method);
                }
            }
        };
        match self.types[ty] {
            Type::Class(id) => {
                let mut result = Vec::new();
                add_visible(&mut result, &self.class_methods[&id]);
                add_visible(&mut result, &self.base_chain_methods(id));
                result
            }
            Type::Interface(id, _) => {
                let mut result = Vec::new();
                add_visible(&mut result, &self.interface_methods[&id]);
                result
            }
            Type::Struct(id, _) => {
                let mut result = Vec::new();
                add_visible(&mut result, &self.struct_methods[&id]);
                result
            }
            Type::Enum(id, _) => {
                let mut result = Vec::new();
                add_visible(&mut result, &self.enum_methods[&id]);
                result
            }
            Type::Any => self.any_method(name).into_iter().collect(),
            _ => Vec::new(),
        }
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
