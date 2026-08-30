//! Class / interface resolution and inheritance checks (M6,
//! milestone6 DESIGN.md 2.2).
//!
//! Pass 2 (`resolve_class`): constructor properties (duplicate names
//! diagnosed), the base-class clause (the base must be an `open` or
//! `abstract` class) and the interface list.
//!
//! Pass 2.5 (`resolve_method_signature`): member signatures. Methods
//! have no type parameters of their own (generic member functions
//! cannot participate in virtual dispatch, spec 3.2, and remain
//! unsupported in M7); enum methods resolve in the enum's type-parameter
//! scope. Bodyless declarations — interface methods and `abstract`
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

use hir::{ClassId, FunctionId, InterfaceId, Type, TypeId};

use crate::{FnParam, FnSig, Lowerer, Owner};

impl Lowerer {
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
    pub(crate) fn resolve_interface_list(&mut self, names: &[ast::Ident]) -> Vec<InterfaceId> {
        let mut interfaces = Vec::new();
        for name in names {
            match self.interfaces_by_name.get(&name.text) {
                Some(&(iface, _)) => {
                    if !interfaces.contains(&iface) {
                        interfaces.push(iface);
                    }
                }
                None => {
                    let what = if self.type_namespace_conflict(&name.text).is_some() {
                        format!("`{}` is not an interface", name.text)
                    } else {
                        format!("unknown type `{}`", name.text)
                    };
                    self.error(name.span, what);
                }
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
            self.error(
                decl.name.span,
                format!("generic member function `{short}` is not supported in M6"),
            );
        }
        self.check_method_body_shape(id, decl, owner);

        // Enum methods resolve in the enum's type-parameter scope (a
        // method of `E<T>` may mention `T`); every other owner has no
        // type parameters in scope.
        self.type_params_in_scope = match owner {
            Owner::Enum(enum_id) => {
                let type_params = self.enums[enum_id].type_params.clone();
                if !type_params.is_empty() {
                    self.register_generic(id);
                }
                type_params
            }
            _ => Vec::new(),
        };
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
                type_params: Vec::new(),
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
        let this = self.locals.alloc(hir::Local {
            name: "this".to_string(),
            ty: host_ty,
            mutable: false,
        });
        let mut params = vec![hir::Param {
            name: "this".to_string(),
            ty: host_ty,
            local: this,
        }];
        let mut declared = Vec::with_capacity(sig.params.len());
        for param in &sig.params {
            let local = self.locals.alloc(hir::Local {
                name: param.name.text.clone(),
                ty: param.ty,
                mutable: false,
            });
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
        let candidates = match owner {
            Owner::Class(class_id) => {
                let mut candidates = self.base_chain_methods(class_id);
                for iface in self.class_interfaces_all(class_id) {
                    candidates.extend(self.interface_methods[&iface].iter().copied());
                }
                candidates
            }
            Owner::Struct(struct_id) => self.structs[struct_id]
                .interfaces
                .iter()
                .flat_map(|&iface| self.interface_methods[&iface].iter().copied())
                .collect(),
            Owner::Enum(enum_id) => self.enums[enum_id]
                .interfaces
                .iter()
                .flat_map(|&iface| self.interface_methods[&iface].iter().copied())
                .collect(),
            Owner::Interface(_) => return,
        };
        let sig = self.signatures[&id].clone();
        let short = decl.name.text.clone();
        let overrides = candidates
            .iter()
            .copied()
            .find(|&candidate| self.same_signature(candidate, &short, &sig));
        if let Some(candidate) = overrides
            && matches!(self.function_owner.get(&candidate), Some(Owner::Class(_)))
            && self.functions[candidate]
                .method
                .is_some_and(|method| method.modifier == hir::MethodModifier::Final)
        {
            let owner = self.functions[candidate].name.clone();
            self.error(
                decl.name.span,
                format!("`{short}` cannot override final method `{owner}`"),
            );
            return;
        }
        match (overrides, decl.is_override) {
            (Some(candidate), false) => {
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
        for iface in self.class_interfaces_all(id) {
            let methods: Vec<(String, FunctionId)> = self.interface_methods[&iface]
                .iter()
                .map(|&m| (self.functions[m].name.clone(), m))
                .collect();
            for (qualified, method) in methods {
                let sig = self.signatures[&method].clone();
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
        let (interfaces, own_methods): (Vec<InterfaceId>, Vec<FunctionId>) = match owner {
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
        for iface in interfaces {
            let methods: Vec<(String, FunctionId)> = self.interface_methods[&iface]
                .iter()
                .map(|&m| (self.functions[m].name.clone(), m))
                .collect();
            for (qualified, method) in methods {
                let sig = self.signatures[&method].clone();
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
        let function = &self.functions[candidate];
        if function.name.rsplit('.').next() != Some(name) {
            return false;
        }
        let Some(candidate_sig) = self.signatures.get(&candidate) else {
            return false;
        };
        candidate_sig.params.len() == sig.params.len()
            && candidate_sig
                .params
                .iter()
                .zip(&sig.params)
                .all(|(a, b)| self.types_equal(a.ty, b.ty))
            && self.types_equal(candidate_sig.return_ty, sig.return_ty)
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

    /// Whether class `c` or one of its base classes implements
    /// interface `i`.
    pub(crate) fn class_implements(&self, c: ClassId, i: InterfaceId) -> bool {
        self.class_interfaces_all(c).contains(&i)
    }

    /// Every interface implemented by class `c` or its base classes,
    /// deduplicated, own list first (cycle-safe).
    pub(crate) fn class_interfaces_all(&self, c: ClassId) -> Vec<InterfaceId> {
        let mut result = Vec::new();
        let mut seen = vec![c];
        let mut current = Some(c);
        while let Some(id) = current {
            for &iface in &self.classes[id].interfaces {
                if !result.contains(&iface) {
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
            Type::Interface(id) => {
                let mut result = Vec::new();
                add_visible(&mut result, &self.interface_methods[&id]);
                result
            }
            Type::Struct(id) => {
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
