use super::*;

impl Lowerer {
    fn overload_types_equal(
        &self,
        left: TypeId,
        right: TypeId,
        parameter_pairs: &[(hir::TypeParamId, hir::TypeParamId)],
    ) -> bool {
        let (Type::Function(left), Type::Function(right)) = (&self.types[left], &self.types[right])
        else {
            return self.signature_types_equal(left, right, parameter_pairs);
        };
        let left = &self.function_types[*left];
        let right = &self.function_types[*right];
        left.is_suspend == right.is_suspend
            && left.arity() == right.arity()
            && left
                .parameters()
                .zip(right.parameters())
                .all(|(&left, &right)| self.overload_types_equal(left, right, parameter_pairs))
            && self.overload_types_equal(left.return_type, right.return_type, parameter_pairs)
    }

    /// Compare parameter signatures under the exact alpha-renaming relation
    /// between their globally unique type-parameter identities.
    pub(crate) fn same_parameter_signature(&self, a: FunctionId, b: FunctionId) -> bool {
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
            (Some(&a), Some(&b)) => self.overload_types_equal(a, b, &parameter_pairs),
            (None, None) => true,
            _ => false,
        };
        receivers_match
            && a_sig.params.len() == b_sig.params.len()
            && a_sig
                .params
                .iter()
                .zip(&b_sig.params)
                .all(|(x, y)| self.overload_types_equal(x.ty, y.ty, &parameter_pairs))
    }

    fn signature_types_equal(
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
                    && left.receiver.is_some() == right.receiver.is_some()
                    && self.signature_type_lists_equal(
                        &left.parameters().copied().collect::<Vec<_>>(),
                        &right.parameters().copied().collect::<Vec<_>>(),
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

    fn signature_type_lists_equal(
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
