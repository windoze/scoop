use super::*;

impl Lowerer {
    pub(crate) fn validate_core_operator_intrinsics(&mut self, files: &[ast::SourceFile]) {
        for kind in hir::PrimitiveUnaryKind::ALL {
            let intrinsic = hir::IntrinsicFunctionKind::PrimitiveUnary(kind);
            if let Some(function) = self.require_intrinsic(intrinsic, files) {
                self.validate_primitive_unary_intrinsic(function, kind);
            }
        }
        for kind in hir::PrimitiveBinaryKind::ALL {
            let intrinsic = hir::IntrinsicFunctionKind::PrimitiveBinary(kind);
            if let Some(function) = self.require_intrinsic(intrinsic, files) {
                self.validate_primitive_binary_intrinsic(function, kind);
            }
        }
        for kind in hir::ArrayAccessKind::ALL {
            let intrinsic = hir::IntrinsicFunctionKind::ArrayAccess(kind);
            if let Some(function) = self.require_intrinsic(intrinsic, files) {
                self.validate_array_access_intrinsic(function, kind);
            }
        }
    }

    fn validate_primitive_unary_intrinsic(
        &mut self,
        function: FunctionId,
        kind: hir::PrimitiveUnaryKind,
    ) {
        let (owner_kind, operator, result) = match kind {
            hir::PrimitiveUnaryKind::IntUnaryPlus => (
                hir::IntrinsicTypeKind::Int,
                hir::OperatorKind::UnaryPlus,
                self.int,
            ),
            hir::PrimitiveUnaryKind::IntUnaryMinus => (
                hir::IntrinsicTypeKind::Int,
                hir::OperatorKind::UnaryMinus,
                self.int,
            ),
            hir::PrimitiveUnaryKind::IntInc => (
                hir::IntrinsicTypeKind::Int,
                hir::OperatorKind::Inc,
                self.int,
            ),
            hir::PrimitiveUnaryKind::IntDec => (
                hir::IntrinsicTypeKind::Int,
                hir::OperatorKind::Dec,
                self.int,
            ),
            hir::PrimitiveUnaryKind::UIntUnaryPlus => (
                hir::IntrinsicTypeKind::UInt,
                hir::OperatorKind::UnaryPlus,
                self.uint,
            ),
            hir::PrimitiveUnaryKind::UIntInc => (
                hir::IntrinsicTypeKind::UInt,
                hir::OperatorKind::Inc,
                self.uint,
            ),
            hir::PrimitiveUnaryKind::UIntDec => (
                hir::IntrinsicTypeKind::UInt,
                hir::OperatorKind::Dec,
                self.uint,
            ),
            hir::PrimitiveUnaryKind::BooleanNot => (
                hir::IntrinsicTypeKind::Boolean,
                hir::OperatorKind::Not,
                self.boolean,
            ),
        };
        let signature = &self.signatures[&function];
        let valid = self.function_has_intrinsic_owner(function, owner_kind)
            && !signature.is_suspend
            && signature.type_params.is_empty()
            && signature.owner_type_param_count == 0
            && signature.params.is_empty()
            && signature.return_ty == result
            && signature.modifiers.operator == Some(operator)
            && !signature.modifiers.is_infix;
        if !valid {
            self.malformed_operator_intrinsic(function, kind.name());
        }
    }

    fn validate_primitive_binary_intrinsic(
        &mut self,
        function: FunctionId,
        kind: hir::PrimitiveBinaryKind,
    ) {
        let (owner_kind, operand, result, operator) = match kind {
            hir::PrimitiveBinaryKind::IntAdd => (
                hir::IntrinsicTypeKind::Int,
                self.int,
                self.int,
                hir::OperatorKind::Plus,
            ),
            hir::PrimitiveBinaryKind::IntSub => (
                hir::IntrinsicTypeKind::Int,
                self.int,
                self.int,
                hir::OperatorKind::Minus,
            ),
            hir::PrimitiveBinaryKind::IntMul => (
                hir::IntrinsicTypeKind::Int,
                self.int,
                self.int,
                hir::OperatorKind::Times,
            ),
            hir::PrimitiveBinaryKind::IntDiv => (
                hir::IntrinsicTypeKind::Int,
                self.int,
                self.int,
                hir::OperatorKind::Div,
            ),
            hir::PrimitiveBinaryKind::IntRem => (
                hir::IntrinsicTypeKind::Int,
                self.int,
                self.int,
                hir::OperatorKind::Rem,
            ),
            hir::PrimitiveBinaryKind::IntCompareTo => (
                hir::IntrinsicTypeKind::Int,
                self.int,
                self.int,
                hir::OperatorKind::CompareTo,
            ),
            hir::PrimitiveBinaryKind::UIntAdd => (
                hir::IntrinsicTypeKind::UInt,
                self.uint,
                self.uint,
                hir::OperatorKind::Plus,
            ),
            hir::PrimitiveBinaryKind::UIntSub => (
                hir::IntrinsicTypeKind::UInt,
                self.uint,
                self.uint,
                hir::OperatorKind::Minus,
            ),
            hir::PrimitiveBinaryKind::UIntMul => (
                hir::IntrinsicTypeKind::UInt,
                self.uint,
                self.uint,
                hir::OperatorKind::Times,
            ),
            hir::PrimitiveBinaryKind::UIntDiv => (
                hir::IntrinsicTypeKind::UInt,
                self.uint,
                self.uint,
                hir::OperatorKind::Div,
            ),
            hir::PrimitiveBinaryKind::UIntRem => (
                hir::IntrinsicTypeKind::UInt,
                self.uint,
                self.uint,
                hir::OperatorKind::Rem,
            ),
            hir::PrimitiveBinaryKind::UIntCompareTo => (
                hir::IntrinsicTypeKind::UInt,
                self.uint,
                self.int,
                hir::OperatorKind::CompareTo,
            ),
            hir::PrimitiveBinaryKind::StringConcat => (
                hir::IntrinsicTypeKind::String,
                self.string,
                self.string,
                hir::OperatorKind::Plus,
            ),
            hir::PrimitiveBinaryKind::StringCompareTo => (
                hir::IntrinsicTypeKind::String,
                self.string,
                self.int,
                hir::OperatorKind::CompareTo,
            ),
        };
        let signature = &self.signatures[&function];
        let valid = self.function_has_intrinsic_owner(function, owner_kind)
            && !signature.is_suspend
            && signature.type_params.is_empty()
            && signature.owner_type_param_count == 0
            && matches!(signature.params.as_slice(), [parameter] if parameter.ty == operand)
            && signature.return_ty == result
            && signature.modifiers.operator == Some(operator)
            && !signature.modifiers.is_infix;
        if !valid {
            self.malformed_operator_intrinsic(function, kind.name());
        }
    }

    fn validate_array_access_intrinsic(
        &mut self,
        function: FunctionId,
        kind: hir::ArrayAccessKind,
    ) {
        let (owner_kind, operator) = match kind {
            hir::ArrayAccessKind::ImmutableGet => {
                (hir::IntrinsicTypeKind::Array, hir::OperatorKind::Get)
            }
            hir::ArrayAccessKind::MutableGet => {
                (hir::IntrinsicTypeKind::MutableArray, hir::OperatorKind::Get)
            }
            hir::ArrayAccessKind::MutableSet => {
                (hir::IntrinsicTypeKind::MutableArray, hir::OperatorKind::Set)
            }
        };
        let signature = &self.signatures[&function];
        let parameters_match = match kind {
            hir::ArrayAccessKind::ImmutableGet | hir::ArrayAccessKind::MutableGet => {
                matches!(signature.params.as_slice(), [index] if index.ty == self.int)
                    && self.is_type_param(signature.return_ty, 0)
            }
            hir::ArrayAccessKind::MutableSet => {
                matches!(signature.params.as_slice(), [index, value]
                    if index.ty == self.int && self.is_type_param(value.ty, 0))
                    && signature.return_ty == self.unit
            }
        };
        let valid = self.function_has_intrinsic_owner(function, owner_kind)
            && !signature.is_suspend
            && signature.owner_type_param_count == 1
            && signature.type_params.len() == 1
            && signature.type_params[0].kind() == hir::TypeParamKind::Any
            && parameters_match
            && signature.modifiers.operator == Some(operator)
            && !signature.modifiers.is_infix;
        if !valid {
            self.malformed_operator_intrinsic(function, kind.name());
        }
    }

    fn function_has_intrinsic_owner(
        &self,
        function: FunctionId,
        kind: hir::IntrinsicTypeKind,
    ) -> bool {
        let Some(&(intrinsic_owner, _)) = self.intrinsic_type_owners.get(&kind) else {
            return false;
        };
        match (self.function_owner.get(&function), intrinsic_owner) {
            (Some(Owner::Struct(actual)), IntrinsicTypeOwner::Struct(expected)) => {
                *actual == expected
            }
            (Some(Owner::Class(actual)), IntrinsicTypeOwner::Class(expected)) => {
                *actual == expected
            }
            _ => false,
        }
    }

    fn malformed_operator_intrinsic(&mut self, function: FunctionId, intrinsic: &str) {
        self.current_file = self.function_files[&function];
        self.error(
            self.functions[function].span,
            format!("malformed core operator intrinsic `{intrinsic}`"),
        );
    }
}
