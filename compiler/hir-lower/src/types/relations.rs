use super::*;

mod parents;

impl Lowerer {
    /// Subtyping follows the complete parent applications shared by source
    /// checking and inference. Function types retain their structural variance.
    pub(crate) fn is_subtype(&mut self, a: TypeId, b: TypeId) -> bool {
        if self.types_equal(a, b) || self.is_nothing_ty(a) || matches!(self.types[b], Type::Any) {
            return true;
        }
        if let (Type::Function(source), Type::Function(target)) =
            (self.types[a].clone(), self.types[b].clone())
        {
            let source = self.function_types[source].clone();
            let target = self.function_types[target].clone();
            return source.is_suspend == target.is_suspend
                && source.parameter_types.len() == target.parameter_types.len()
                && target
                    .parameter_types
                    .into_iter()
                    .zip(source.parameter_types)
                    .all(|(target, source)| self.is_subtype(target, source))
                && self.is_subtype(source.return_type, target.return_type);
        }
        if !matches!(self.types[b], Type::Class(_) | Type::Interface(_)) {
            return false;
        }
        let mut pending = vec![a];
        let mut seen = std::collections::HashSet::new();
        while let Some(ty) = pending.pop() {
            if !seen.insert(ty) {
                continue;
            }
            if self.types_equal(ty, b) {
                return true;
            }
            pending.extend(self.direct_nominal_supertypes(ty));
        }
        false
    }

    pub(crate) fn is_nothing_ty(&self, ty: TypeId) -> bool {
        matches!(self.types[ty], Type::Class(application)
            if matches!(self.class_applications[application].representation,
                hir::ClassApplicationRepresentation::Intrinsic(hir::IntrinsicTypeRepresentation::Nothing)))
    }

    pub(crate) fn nothing_type(&mut self) -> TypeId {
        match &self.core {
            crate::CoreLoweringAuthority::Defined => {
                let (IntrinsicTypeOwner::Class(owner), _) =
                    self.intrinsic_type_owners[&hir::IntrinsicTypeKind::Nothing]
                else {
                    unreachable!("the core contract requires an intrinsic Nothing class")
                };
                self.class_application(owner, Vec::new())
            }
            crate::CoreLoweringAuthority::Imported(core) => {
                let owner = core.fundamental_types().nothing().persistent();
                self.imported_nominal_application(hir::SourceNominalId::Concrete(owner), Vec::new())
                    .expect("the imported core contract provides its Nothing declaration")
            }
        }
    }

    /// Every reachable interface application, including the subject itself
    /// when it is an interface. Distinct arguments remain distinct entries.
    pub(crate) fn type_interfaces(&mut self, ty: TypeId) -> Vec<TypeId> {
        self.interfaces_from_roots(vec![ty])
    }

    pub(crate) fn class_interfaces_for_application(
        &mut self,
        application: hir::ClassApplicationId,
    ) -> Vec<TypeId> {
        // Intrinsic classes can have a normalized canonical Type (String).
        // Read their declaration's edges without resolving that Type again.
        let roots = self.direct_nominal_supertypes_for(Type::Class(application));
        self.interfaces_from_roots(roots)
    }

    fn interfaces_from_roots(&mut self, roots: Vec<TypeId>) -> Vec<TypeId> {
        let mut result = Vec::new();
        let mut pending = Vec::new();
        for root in roots {
            if matches!(self.types[root], Type::Interface(_)) {
                self.append_interface_closure(root, &mut result);
            } else {
                pending.push(root);
            }
        }
        pending.reverse();
        let mut seen = std::collections::HashSet::new();
        while let Some(ty) = pending.pop() {
            if !seen.insert(ty) {
                continue;
            }
            if matches!(self.types[ty], Type::Interface(_)) {
                self.append_interface_closure(ty, &mut result);
                continue;
            }
            let parents = self.direct_nominal_supertypes(ty);
            let mut bases = Vec::new();
            for parent in parents {
                if matches!(self.types[parent], Type::Interface(_)) {
                    self.append_interface_closure(parent, &mut result);
                } else {
                    bases.push(parent);
                }
            }
            pending.extend(bases.into_iter().rev());
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
        for parent in self.direct_nominal_supertypes(interface) {
            self.append_interface_closure(parent, result);
        }
    }

    /// Actual Iterator applications use the same inheritance/bound closure as
    /// all other interfaces, preserving distinct element types for diagnostics.
    pub(crate) fn iteration_interface_applications(&mut self, ty: TypeId) -> Vec<TypeId> {
        self.type_interfaces(ty)
            .into_iter()
            .filter(|&interface| match (&self.core, &self.types[interface]) {
                (crate::CoreLoweringAuthority::Defined, Type::Interface(application)) => {
                    self.interface_applications[*application].template
                        == self
                            .nominal_identity(crate::Owner::Interface(
                                self.iteration_core
                                    .expect("the defining core was checked")
                                    .iterator(),
                            ))
                            .declaration_id()
                }
                (crate::CoreLoweringAuthority::Imported(core), Type::Interface(application)) => {
                    self.interface_applications[*application].template
                        == hir::SourceNominalId::GenericTemplate(
                            core.iteration().iterator().persistent(),
                        )
                }
                _ => false,
            })
            .collect()
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
                self.class_definition(template).modifier != hir::ClassModifier::Final
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
