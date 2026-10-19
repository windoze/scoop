//! Negative facts concern complete static interface applications, never the
//! absence of a currently known subtype edge for an unresolved parameter.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InterfaceExclusion {
    Satisfied,
    Implemented,
    Unknown,
}

impl Lowerer {
    pub(crate) fn excludes_interface(
        &mut self,
        subject: TypeId,
        interface: TypeId,
    ) -> InterfaceExclusion {
        if self.is_subtype(subject, interface) {
            return InterfaceExclusion::Implemented;
        }
        if let Type::Param(id) | Type::Companion(id) = self.types[subject] {
            let excluded = self
                .type_params_in_scope
                .iter()
                .find(|parameter| parameter.id == id)
                .map(|parameter| {
                    if matches!(self.types[subject], Type::Companion(_)) {
                        parameter
                            .companion
                            .as_ref()
                            .map(|bounds| bounds.excluded_interfaces.clone())
                            .unwrap_or_default()
                    } else {
                        parameter.excluded_interfaces.clone()
                    }
                })
                .unwrap_or_default();
            return if excluded
                .iter()
                .any(|bound| self.is_subtype(interface, bound.ty))
            {
                InterfaceExclusion::Satisfied
            } else {
                InterfaceExclusion::Unknown
            };
        }
        for actual in self.type_interfaces(subject) {
            if self.open_types_may_equal(actual, interface) {
                return InterfaceExclusion::Unknown;
            }
        }
        InterfaceExclusion::Satisfied
    }

    /// Only distinguish definite structural mismatches. No substitution is
    /// inferred from a negative condition, including inside invariant types.
    fn open_types_may_equal(&self, left: TypeId, right: TypeId) -> bool {
        if self.types_equal(left, right) {
            return true;
        }
        if !self.type_contains_param(left) && !self.type_contains_param(right) {
            return false;
        }
        if matches!(self.types[left], Type::Param(_) | Type::Companion(_))
            || matches!(self.types[right], Type::Param(_) | Type::Companion(_))
        {
            return true;
        }
        if let (Some((a, aa)), Some((b, ba))) = (
            self.dependency_nominal_application(left),
            self.dependency_nominal_application(right),
        ) {
            return a == b && self.open_type_lists_may_equal(aa, ba);
        }
        match (&self.types[left], &self.types[right]) {
            (Type::Struct(a), Type::Struct(b)) => {
                let (a, b) = (&self.struct_applications[*a], &self.struct_applications[*b]);
                a.template == b.template
                    && self.open_type_lists_may_equal(&a.arguments, &b.arguments)
            }
            (Type::Enum(a), Type::Enum(b)) => {
                let (a, b) = (&self.enum_applications[*a], &self.enum_applications[*b]);
                a.template == b.template
                    && self.open_type_lists_may_equal(&a.arguments, &b.arguments)
            }
            (Type::Class(a), Type::Class(b)) => {
                let (a, b) = (&self.class_applications[*a], &self.class_applications[*b]);
                a.template == b.template
                    && self.open_type_lists_may_equal(&a.arguments, &b.arguments)
            }
            (Type::Interface(a), Type::Interface(b)) => {
                let (a, b) = (
                    &self.interface_applications[*a],
                    &self.interface_applications[*b],
                );
                a.template == b.template
                    && self.open_type_lists_may_equal(&a.arguments, &b.arguments)
            }
            (Type::Tuple(a), Type::Tuple(b)) => self.open_type_lists_may_equal(a, b),
            (Type::Ptr(a), Type::Ptr(b)) => self.open_types_may_equal(*a, *b),
            (Type::Function(a), Type::Function(b)) | (Type::FunPtr(a), Type::FunPtr(b)) => {
                let (a, b) = (&self.function_types[*a], &self.function_types[*b]);
                a.is_suspend == b.is_suspend
                    && a.arity() == b.arity()
                    && a.parameters()
                        .zip(b.parameters())
                        .all(|(&a, &b)| self.open_types_may_equal(a, b))
                    && self.open_types_may_equal(a.return_type, b.return_type)
            }
            _ => false,
        }
    }

    fn open_type_lists_may_equal(&self, left: &[TypeId], right: &[TypeId]) -> bool {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(&a, &b)| self.open_types_may_equal(a, b))
    }
}
