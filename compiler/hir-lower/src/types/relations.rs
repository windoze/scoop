use super::*;

impl Lowerer {
    /// The subtyping relation (milestone6 DESIGN.md 2.2): equal types,
    /// a class below its base classes, a class below the interfaces it
    /// (or a base class) implements, a value type below the interfaces
    /// it implements (spec 4.4.3), and everything below `Any`. Value
    /// types count as subtypes of `Any` (they cross via boxing, spec
    /// 4.4.4); arrays are invariant in the element type (spec 10.4), so
    /// no array is a subtype of another array.
    pub(crate) fn is_subtype(&mut self, a: TypeId, b: TypeId) -> bool {
        if self.types_equal(a, b) {
            return true;
        }
        let a_ty = self.types[a].clone();
        let b_ty = self.types[b].clone();
        match (a_ty, b_ty) {
            (_, Type::Any) => true,
            (Type::ImportedInterface(interface), _) => interface
                .parents
                .iter()
                .any(|parent| self.is_subtype(*parent, b)),
            (Type::ImportedClass(class), _) => class
                .base_class
                .into_iter()
                .chain(class.interfaces.iter().copied())
                .any(|parent| self.is_subtype(parent, b)),
            (Type::ImportedStruct(structure), _) => structure
                .interfaces
                .iter()
                .any(|parent| self.is_subtype(*parent, b)),
            (Type::ImportedEnum(enumeration), _) => enumeration
                .interfaces
                .iter()
                .any(|parent| self.is_subtype(*parent, b)),
            (Type::Param(parameter), _) => {
                let Some(parameter) = self
                    .type_params_in_scope
                    .iter()
                    .find(|candidate| candidate.id == parameter)
                    .cloned()
                else {
                    return false;
                };
                parameter
                    .nominal_bounds_in_source_order()
                    .into_iter()
                    .any(|bound| {
                        let ty = match bound {
                            hir::NominalBoundRef::Class(bound) => {
                                self.class_applications[bound.application].canonical_type
                            }
                            hir::NominalBoundRef::Interface(bound) => {
                                self.interface_applications[bound.application].canonical_type
                            }
                            hir::NominalBoundRef::ImportedClass(bound)
                            | hir::NominalBoundRef::ImportedInterface(bound) => bound.ty,
                        };
                        self.is_subtype(ty, b)
                    })
            }
            (Type::Class(application), Type::Class(..) | Type::ImportedClass(_)) => {
                let application = self.class_applications[application].clone();
                let Some(base) = self.classes[application.template].base_class else {
                    return false;
                };
                let base = self.instantiate_ty(base, &application.arguments);
                self.is_subtype(base, b)
            }
            (Type::Interface(a), Type::Interface(b)) => {
                let a = self.interface_applications[a].clone();
                let b = self.interface_applications[b].clone();
                if a.template != b.template {
                    let parents = self.interfaces[a.template].parents.clone();
                    return parents.into_iter().any(|parent| {
                        let parent = self.instantiate_ty(parent, &a.arguments);
                        self.is_subtype(parent, b.canonical_type)
                    });
                }
                a.arguments
                    .into_iter()
                    .zip(b.arguments)
                    .all(|(a, b)| self.types_equal(a, b))
            }
            (Type::Interface(_), Type::ImportedInterface(_)) => self
                .interface_parent_types(a)
                .into_iter()
                .any(|parent| self.is_subtype(parent, b)),
            (Type::Function(source), Type::Function(target)) => {
                let source = self.function_types[source].clone();
                let target = self.function_types[target].clone();
                source.is_suspend == target.is_suspend
                    && source.parameter_types.len() == target.parameter_types.len()
                    && target
                        .parameter_types
                        .into_iter()
                        .zip(source.parameter_types)
                        .all(|(target, source)| self.is_subtype(target, source))
                    && self.is_subtype(source.return_type, target.return_type)
            }
            (Type::Integer(kind), Type::Interface(..) | Type::ImportedInterface(_)) => self
                .intrinsic_type_interfaces(hir::IntrinsicTypeKind::Integer(kind))
                .into_iter()
                .any(|implemented| self.is_subtype(implemented, b)),
            (Type::Boolean, Type::Interface(..) | Type::ImportedInterface(_)) => self
                .intrinsic_type_interfaces(hir::IntrinsicTypeKind::Boolean)
                .into_iter()
                .any(|implemented| self.is_subtype(implemented, b)),
            (Type::String, Type::Interface(..) | Type::ImportedInterface(_)) => self
                .intrinsic_type_interfaces(hir::IntrinsicTypeKind::String)
                .into_iter()
                .any(|implemented| self.is_subtype(implemented, b)),
            (Type::Class(application), Type::Interface(..) | Type::ImportedInterface(_)) => self
                .class_interfaces_for_application(application)
                .into_iter()
                .any(|implemented| self.is_subtype(implemented, b)),
            (Type::Struct(application), Type::Interface(..) | Type::ImportedInterface(_)) => {
                let application = self.struct_applications[application].clone();
                let interfaces = self.structs[application.template].interfaces.clone();
                interfaces.into_iter().any(|implemented| {
                    let implemented = self.instantiate_ty(implemented, &application.arguments);
                    self.is_subtype(implemented, b)
                })
            }
            (Type::Enum(application), Type::Interface(..) | Type::ImportedInterface(_)) => {
                let application = self.enum_applications[application].clone();
                let interfaces = self.enums[application.template].interfaces.clone();
                interfaces.into_iter().any(|implemented| {
                    let implemented = self.instantiate_ty(implemented, &application.arguments);
                    self.is_subtype(implemented, b)
                })
            }
            _ => false,
        }
    }

    /// Interfaces explicitly declared by the source definition of one
    /// compiler-represented intrinsic type. Primitive `Type` variants retain
    /// their local owner or the declaration resolved from ordinary dependency
    /// metadata; the interface set is not built into the compiler.
    pub(crate) fn intrinsic_type_interfaces(
        &mut self,
        kind: hir::IntrinsicTypeKind,
    ) -> Vec<TypeId> {
        if let Err(error) = self.resolve_imported_intrinsic_type(kind) {
            self.diagnostics.push(ast::Diagnostic::without_span(
                ast::DiagnosticSeverity::Error,
                self.current_file,
                error.diagnostic(&format!(
                    "interfaces of intrinsic type `{}`",
                    kind.source_name()
                )),
            ));
            return Vec::new();
        }
        if let Some(source) = self.imported_intrinsic_types.get(&kind) {
            return source.interfaces.clone();
        }
        let Some(&(owner, _provider)) = self.intrinsic_type_owners.get(&kind) else {
            // A missing intrinsic owner is diagnosed by the core-contract
            // validator and prevents HIR output.  During recovery there is no
            // source declaration whose interfaces could be consumed.
            return Vec::new();
        };
        match owner {
            crate::IntrinsicTypeOwner::Struct(owner) => self.structs[owner].interfaces.clone(),
            crate::IntrinsicTypeOwner::Class(owner) => {
                self.class_interfaces_for_application(self.classes[owner].self_application)
            }
        }
    }

    /// Fully substitute every interface reached from one class application,
    /// including interfaces inherited through its concrete generic base
    /// application. Applications, rather than declaration ids, are the
    /// deduplication identity.
    pub(crate) fn class_interfaces_for_application(
        &mut self,
        application: hir::ClassApplicationId,
    ) -> Vec<TypeId> {
        let mut result = Vec::new();
        let mut pending = vec![(
            self.class_applications[application].canonical_type,
            Type::Class(application),
        )];
        let mut seen = Vec::new();
        while let Some((ty, class_type)) = pending.pop() {
            if seen.contains(&ty) {
                continue;
            }
            seen.push(ty);
            match class_type {
                Type::Class(application) => {
                    let application = self.class_applications[application].clone();
                    let class = self.classes[application.template].clone();
                    for interface in class.interfaces {
                        let interface = self.instantiate_ty(interface, &application.arguments);
                        self.append_interface_closure(interface, &mut result);
                    }
                    if let Some(base) = class.base_class {
                        let base = self.instantiate_ty(base, &application.arguments);
                        pending.push((base, self.types[base].clone()));
                    }
                }
                Type::ImportedClass(class) => {
                    for interface in &class.interfaces {
                        self.append_interface_closure(*interface, &mut result);
                    }
                    pending.extend(
                        class
                            .base_class
                            .map(|base| (base, self.types[base].clone())),
                    );
                }
                _ => unreachable!("resolved class bases have class types"),
            }
        }
        result
    }

    pub(crate) fn append_interface_closure(&mut self, interface: TypeId, result: &mut Vec<TypeId>) {
        if result
            .iter()
            .any(|&other| self.types_equal(other, interface))
        {
            return;
        }
        result.push(interface);
        let parents = self.interface_parent_types(interface);
        for parent in parents {
            self.append_interface_closure(parent, result);
        }
    }

    pub(crate) fn interface_parent_types(&mut self, interface: TypeId) -> Vec<TypeId> {
        match self.types[interface].clone() {
            Type::Interface(application) => {
                let application = self.interface_applications[application].clone();
                self.interfaces[application.template]
                    .parents
                    .clone()
                    .into_iter()
                    .map(|parent| self.instantiate_ty(parent, &application.arguments))
                    .collect()
            }
            Type::ImportedInterface(interface) => interface.parents.clone(),
            _ => unreachable!("interface closure contains interface types"),
        }
    }

    /// Collect every distinct exact application of `target` reachable from a
    /// type's complete class/interface/bound closure. Unlike ordinary
    /// subtyping queries this deliberately retains multiple applications with
    /// different arguments so source iteration can diagnose an ambiguous
    /// element type.
    pub(crate) fn exact_interface_applications(
        &mut self,
        ty: TypeId,
        target: hir::InterfaceId,
    ) -> Vec<hir::InterfaceApplicationId> {
        let mut roots = Vec::new();
        match self.types[ty].clone() {
            Type::Interface(application) => {
                roots.push(self.interface_applications[application].canonical_type)
            }
            Type::Class(application) => {
                roots.extend(self.class_interfaces_for_application(application));
            }
            Type::Struct(application) => {
                let application = self.struct_applications[application].clone();
                for interface in self.structs[application.template].interfaces.clone() {
                    roots.push(self.instantiate_ty(interface, &application.arguments));
                }
            }
            Type::Enum(application) => {
                let application = self.enum_applications[application].clone();
                for interface in self.enums[application.template].interfaces.clone() {
                    roots.push(self.instantiate_ty(interface, &application.arguments));
                }
            }
            Type::Integer(kind) => {
                roots.extend(self.intrinsic_type_interfaces(hir::IntrinsicTypeKind::Integer(kind)));
            }
            Type::Boolean => {
                roots.extend(self.intrinsic_type_interfaces(hir::IntrinsicTypeKind::Boolean));
            }
            Type::String => {
                roots.extend(self.intrinsic_type_interfaces(hir::IntrinsicTypeKind::String));
            }
            Type::Param(parameter) => {
                let declaration = self
                    .type_params_in_scope
                    .iter()
                    .find(|candidate| candidate.id == parameter)
                    .expect("the result parameter is in the active declaration scope")
                    .clone();
                for bound in declaration.nominal_bounds_in_source_order() {
                    match bound {
                        // This query returns applications of a current-Cone interface;
                        // a dependency bound cannot mention that later declaration.
                        hir::NominalBoundRef::ImportedClass(_)
                        | hir::NominalBoundRef::ImportedInterface(_) => continue,
                        hir::NominalBoundRef::Class(bound) => {
                            roots.extend(self.class_interfaces_for_application(bound.application));
                        }
                        hir::NominalBoundRef::Interface(bound) => roots
                            .push(self.interface_applications[bound.application].canonical_type),
                    }
                }
            }
            Type::ImportedStruct(_)
            | Type::ImportedEnum(_)
            | Type::ImportedClass(_)
            | Type::ImportedInterface(_)
            | Type::Unit
            | Type::Any
            | Type::Tuple(_)
            | Type::Function(_)
            | Type::Ptr(_)
            | Type::FunPtr(_) => {}
        }

        let mut closure = Vec::new();
        for root in roots {
            self.append_interface_closure(root, &mut closure);
        }
        let mut applications = Vec::new();
        for interface in closure {
            let Type::Interface(application) = self.types[interface] else {
                unreachable!("interface closure contains only interface types")
            };
            if self.interface_applications[application].template == target
                && !applications.contains(&application)
            {
                applications.push(application);
            }
        }
        applications
    }

    /// Whether a value of static type `a` could ever hold a `b` at run
    /// time — the static premise of `is` / `as` / `as?` (a check
    /// between unrelated types is diagnosed as useless). Beyond the
    /// subtyping relation in either direction, `Any` and interfaces
    /// can hold anything below them, and an open/abstract class may
    /// gain an interface implementation in a subclass.
    pub(crate) fn could_hold(&mut self, a: TypeId, b: TypeId) -> bool {
        if self.is_subtype(a, b) || self.is_subtype(b, a) {
            return true;
        }
        match &self.types[a] {
            Type::Any | Type::Interface(..) | Type::ImportedInterface(_) => true,
            Type::ImportedClass(class) => {
                class.declaration.interface.declaration_details().modality()
                    != hir::NominalInheritanceModalityV1::Final
                    && matches!(
                        self.types[b],
                        Type::Interface(_) | Type::ImportedInterface(_)
                    )
            }
            &Type::Class(application) => {
                let template = self.class_applications[application].template;
                self.classes[template].modifier != hir::ClassModifier::Final
                    && matches!(
                        self.types[b],
                        Type::Interface(..) | Type::ImportedInterface(_)
                    )
            }
            _ => false,
        }
    }

    /// Value types (spec 3): everything that is not a reference. They
    /// cross into reference types (`Any` / interfaces) only by boxing
    /// (spec 4.4.4).
    pub(crate) fn is_value_ty(&self, ty: TypeId) -> bool {
        match self.types[ty] {
            Type::ImportedStruct(_)
            | Type::ImportedEnum(_)
            | Type::Unit
            | Type::Integer(_)
            | Type::Boolean
            | Type::Struct(..)
            | Type::Enum(..)
            | Type::Tuple(_)
            | Type::Ptr(_)
            | Type::FunPtr(_) => true,
            Type::Param(index) => self
                .type_params_in_scope
                .iter()
                .find(|parameter| parameter.id == index)
                .is_none_or(|param| param.kind() != hir::TypeParamKind::Ref),
            _ => false,
        }
    }

    /// Reference types (spec 3): classes, interfaces, `Any`, strings
    /// and the built-in array types. `===` / `!==` only apply to these
    /// (spec 4.4.2).
    pub(crate) fn is_ref_ty(&self, ty: TypeId) -> bool {
        !self.is_value_ty(ty)
    }
}
