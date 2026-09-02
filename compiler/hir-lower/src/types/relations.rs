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
            (Type::Param(parameter), _) => self
                .type_params_in_scope
                .iter()
                .find(|candidate| candidate.id == parameter)
                .map(|parameter| parameter.interface_bounds().to_vec())
                .unwrap_or_default()
                .into_iter()
                .any(|bound| {
                    let bound = self.interface_applications[bound.application].canonical_type;
                    self.is_subtype(bound, b)
                }),
            (Type::Class(application), Type::Class(..)) => {
                let application = self.class_applications[application].clone();
                let Some((base, _)) = self.classes[application.template].base_class.clone() else {
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
                        let parent = self.interface_applications[parent].canonical_type;
                        let parent = self.instantiate_ty(parent, &a.arguments);
                        self.is_subtype(parent, b.canonical_type)
                    });
                }
                let variances: Vec<hir::Variance> = self.interfaces[a.template]
                    .type_params
                    .iter()
                    .map(|param| param.variance)
                    .collect();
                variances
                    .into_iter()
                    .zip(a.arguments)
                    .zip(b.arguments)
                    .all(|((variance, a), b)| match variance {
                        hir::Variance::Invariant => self.types_equal(a, b),
                        hir::Variance::Out => self.is_subtype(a, b),
                        hir::Variance::In => self.is_subtype(b, a),
                    })
            }
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
            (Type::Int, Type::Interface(..)) => self
                .intrinsic_type_interfaces(hir::IntrinsicTypeKind::Int)
                .into_iter()
                .any(|implemented| self.is_subtype(implemented, b)),
            (Type::UInt, Type::Interface(..)) => self
                .intrinsic_type_interfaces(hir::IntrinsicTypeKind::UInt)
                .into_iter()
                .any(|implemented| self.is_subtype(implemented, b)),
            (Type::Boolean, Type::Interface(..)) => self
                .intrinsic_type_interfaces(hir::IntrinsicTypeKind::Boolean)
                .into_iter()
                .any(|implemented| self.is_subtype(implemented, b)),
            (Type::String, Type::Interface(..)) => self
                .intrinsic_type_interfaces(hir::IntrinsicTypeKind::String)
                .into_iter()
                .any(|implemented| self.is_subtype(implemented, b)),
            (Type::Class(application), Type::Interface(..)) => self
                .class_interfaces_for_application(application)
                .into_iter()
                .any(|implemented| self.is_subtype(implemented, b)),
            (Type::Struct(application), Type::Interface(..)) => {
                let application = self.struct_applications[application].clone();
                let interfaces = self.structs[application.template].interfaces.clone();
                interfaces.into_iter().any(|implemented| {
                    let implemented = self.instantiate_ty(implemented, &application.arguments);
                    self.is_subtype(implemented, b)
                })
            }
            (Type::Enum(application), Type::Interface(..)) => {
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
    /// compiler-represented intrinsic type.  Primitive `Type` variants do not
    /// erase their source declaration: `intrinsic_type_owners` is the typed
    /// declaration relation established in pass 1, and all capability checks
    /// consume that relation instead of maintaining a second built-in list.
    fn intrinsic_type_interfaces(&mut self, kind: hir::IntrinsicTypeKind) -> Vec<TypeId> {
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

    /// View a concrete argument type through one exact implemented
    /// interface. Generic-call inference uses this before binding the
    /// interface's type arguments, so `C : I<Int>` can constrain a
    /// parameter declared as `I<T>` without an explicit upcast first.
    pub(crate) fn implemented_interface_application(
        &mut self,
        ty: TypeId,
        target: hir::InterfaceId,
    ) -> Option<Vec<TypeId>> {
        let candidates = match self.types[ty].clone() {
            Type::Interface(application) => {
                let application = self.interface_applications[application].clone();
                return (application.template == target).then_some(application.arguments);
            }
            Type::Int => self.intrinsic_type_interfaces(hir::IntrinsicTypeKind::Int),
            Type::UInt => self.intrinsic_type_interfaces(hir::IntrinsicTypeKind::UInt),
            Type::Boolean => self.intrinsic_type_interfaces(hir::IntrinsicTypeKind::Boolean),
            Type::String => self.intrinsic_type_interfaces(hir::IntrinsicTypeKind::String),
            Type::Class(application) => self.class_interfaces_for_application(application),
            Type::Struct(application) => {
                let application = self.struct_applications[application].clone();
                self.structs[application.template]
                    .interfaces
                    .clone()
                    .into_iter()
                    .map(|implemented| self.instantiate_ty(implemented, &application.arguments))
                    .collect()
            }
            Type::Enum(application) => {
                let application = self.enum_applications[application].clone();
                self.enums[application.template]
                    .interfaces
                    .clone()
                    .into_iter()
                    .map(|implemented| self.instantiate_ty(implemented, &application.arguments))
                    .collect()
            }
            _ => Vec::new(),
        };
        candidates.into_iter().find_map(|candidate| {
            let Type::Interface(application) = self.types[candidate] else {
                return None;
            };
            let application = &self.interface_applications[application];
            (application.template == target).then(|| application.arguments.clone())
        })
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
        let mut pending = vec![application];
        let mut seen = Vec::<hir::ClassApplicationId>::new();
        while let Some(application) = pending.pop() {
            let application_value = self.class_applications[application].clone();
            for interface in self.classes[application_value.template].interfaces.clone() {
                let interface = self.instantiate_ty(interface, &application_value.arguments);
                self.append_interface_closure(interface, &mut result);
            }
            if let Some((base, _)) = self.classes[application_value.template].base_class.clone() {
                let base = self.instantiate_ty(base, &application_value.arguments);
                let Type::Class(base_application) = self.types[base] else {
                    unreachable!("class bases are resolved class applications")
                };
                if seen.contains(&base_application) {
                    continue;
                }
                seen.push(base_application);
                pending.push(base_application);
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
        let Type::Interface(application) = self.types[interface] else {
            unreachable!("interface closure starts from an interface application")
        };
        let application = self.interface_applications[application].clone();
        for parent in self.interfaces[application.template].parents.clone() {
            let parent = self.interface_applications[parent].canonical_type;
            let parent = self.instantiate_ty(parent, &application.arguments);
            self.append_interface_closure(parent, result);
        }
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
            Type::Any | Type::Interface(..) => true,
            &Type::Class(application) => {
                let template = self.class_applications[application].template;
                self.classes[template].modifier != hir::ClassModifier::Final
                    && matches!(self.types[b], Type::Interface(..))
            }
            _ => false,
        }
    }

    /// Value types (spec 3): everything that is not a reference. They
    /// cross into reference types (`Any` / interfaces) only by boxing
    /// (spec 4.4.4).
    pub(crate) fn is_value_ty(&self, ty: TypeId) -> bool {
        match self.types[ty] {
            Type::Unit
            | Type::Int
            | Type::UInt
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
