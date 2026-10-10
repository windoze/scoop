use super::*;

impl Lowerer {
    pub(crate) fn validate_core_operator_intrinsics(&mut self, files: &[ast::SourceFile]) {
        self.validate_char_intrinsics(files);
        self.validate_float_intrinsics(files);
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
        for kind in hir::IntegerKind::ALL {
            for operation in hir::NoGcIntegerOperation::ALL {
                if !operation.supports(kind) {
                    continue;
                }
                let intrinsic =
                    hir::IntrinsicFunctionKind::Integer(hir::IntegerIntrinsicKind::NoGcOperation {
                        kind,
                        operation,
                    });
                if let Some(function) = self.require_intrinsic(intrinsic, files) {
                    self.validate_no_gc_integer_intrinsic(function, kind, operation);
                }
            }
            for operation in hir::IntegerDivRem::ALL {
                let intrinsic = hir::IntrinsicFunctionKind::Integer(
                    hir::IntegerIntrinsicKind::ManagedOperation { kind, operation },
                );
                if let Some(function) = self.require_intrinsic(intrinsic, files) {
                    self.validate_managed_integer_intrinsic(function, kind, operation);
                }
            }
            for target_kind in hir::IntegerKind::ALL {
                let intrinsic =
                    hir::IntrinsicFunctionKind::Integer(hir::IntegerIntrinsicKind::Conversion {
                        source: kind,
                        target_kind,
                    });
                if let Some(function) = self.require_intrinsic(intrinsic, files) {
                    self.validate_integer_conversion_intrinsic(function, kind, target_kind);
                }
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
            hir::PrimitiveUnaryKind::BooleanNot => (
                hir::IntrinsicTypeKind::Boolean,
                hir::OperatorKind::Not,
                self.boolean,
            ),
        };
        let signature = &self.signatures[&function];
        let valid = self.function_has_intrinsic_owner(function, owner_kind)
            && self.plain_intrinsic_signature(signature)
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
        let long = self.integer_type(hir::IntegerKind::SIGNED_64);
        let (owner_kind, operand, result, operator) = match kind {
            hir::PrimitiveBinaryKind::StringConcat => (
                hir::IntrinsicTypeKind::String,
                self.string,
                self.string,
                hir::OperatorKind::Plus,
            ),
            hir::PrimitiveBinaryKind::StringCompareTo => (
                hir::IntrinsicTypeKind::String,
                self.string,
                long,
                hir::OperatorKind::CompareTo,
            ),
        };
        let signature = &self.signatures[&function];
        let valid = self.function_has_intrinsic_owner(function, owner_kind)
            && self.plain_intrinsic_signature(signature)
            && matches!(signature.params.as_slice(), [parameter] if parameter.ty == operand)
            && signature.return_ty == result
            && signature.modifiers.operator == Some(operator)
            && !signature.modifiers.is_infix;
        if !valid {
            self.malformed_operator_intrinsic(function, kind.name());
        }
    }

    fn validate_no_gc_integer_intrinsic(
        &mut self,
        function: FunctionId,
        kind: hir::IntegerKind,
        operation: hir::NoGcIntegerOperation,
    ) {
        let owner = self.integer_type(kind);
        let long = self.integer_type(hir::IntegerKind::SIGNED_64);
        let (parameters_match, result, operator, infix, source_name) = match operation {
            hir::NoGcIntegerOperation::UnaryPlus => (
                self.no_parameters(function),
                owner,
                Some(hir::OperatorKind::UnaryPlus),
                false,
                "unaryPlus",
            ),
            hir::NoGcIntegerOperation::UnaryMinus => (
                self.no_parameters(function),
                owner,
                Some(hir::OperatorKind::UnaryMinus),
                false,
                "unaryMinus",
            ),
            hir::NoGcIntegerOperation::Inc => (
                self.no_parameters(function),
                owner,
                Some(hir::OperatorKind::Inc),
                false,
                "inc",
            ),
            hir::NoGcIntegerOperation::Dec => (
                self.no_parameters(function),
                owner,
                Some(hir::OperatorKind::Dec),
                false,
                "dec",
            ),
            hir::NoGcIntegerOperation::Add => (
                self.one_parameter(function, owner),
                owner,
                Some(hir::OperatorKind::Plus),
                false,
                "plus",
            ),
            hir::NoGcIntegerOperation::Sub => (
                self.one_parameter(function, owner),
                owner,
                Some(hir::OperatorKind::Minus),
                false,
                "minus",
            ),
            hir::NoGcIntegerOperation::Mul => (
                self.one_parameter(function, owner),
                owner,
                Some(hir::OperatorKind::Times),
                false,
                "times",
            ),
            hir::NoGcIntegerOperation::CompareTo => (
                self.one_parameter(function, owner),
                long,
                Some(hir::OperatorKind::CompareTo),
                false,
                "compareTo",
            ),
            hir::NoGcIntegerOperation::Equals => (
                self.one_parameter(function, owner),
                self.boolean,
                Some(hir::OperatorKind::Equals),
                false,
                "equals",
            ),
            hir::NoGcIntegerOperation::And => (
                self.one_parameter(function, owner),
                owner,
                None,
                true,
                "and",
            ),
            hir::NoGcIntegerOperation::Or => {
                (self.one_parameter(function, owner), owner, None, true, "or")
            }
            hir::NoGcIntegerOperation::Xor => (
                self.one_parameter(function, owner),
                owner,
                None,
                true,
                "xor",
            ),
            hir::NoGcIntegerOperation::Inv => {
                (self.no_parameters(function), owner, None, false, "inv")
            }
            hir::NoGcIntegerOperation::Shl => {
                (self.one_parameter(function, long), owner, None, true, "shl")
            }
            hir::NoGcIntegerOperation::Shr => {
                (self.one_parameter(function, long), owner, None, true, "shr")
            }
            hir::NoGcIntegerOperation::Ushr => (
                self.one_parameter(function, long),
                owner,
                None,
                true,
                "ushr",
            ),
        };
        let signature = &self.signatures[&function];
        let valid = self
            .function_has_intrinsic_owner(function, hir::IntrinsicTypeKind::Integer(kind))
            && self.plain_intrinsic_signature(signature)
            && parameters_match
            && signature.return_ty == result
            && signature.modifiers.operator == operator
            && signature.modifiers.is_infix == infix
            && self.source_function_name(function) == source_name
            && hir::NoGcCallableRef::try_from_function(function, self.functions.as_arena())
                .is_some();
        if !valid {
            self.malformed_operator_intrinsic(function, operation.registry_key());
        }
    }

    fn validate_managed_integer_intrinsic(
        &mut self,
        function: FunctionId,
        kind: hir::IntegerKind,
        operation: hir::IntegerDivRem,
    ) {
        let owner = self.integer_type(kind);
        let (operator, source_name) = match operation {
            hir::IntegerDivRem::Div => (hir::OperatorKind::Div, "div"),
            hir::IntegerDivRem::Rem => (hir::OperatorKind::Rem, "rem"),
        };
        let signature = &self.signatures[&function];
        let valid = self
            .function_has_intrinsic_owner(function, hir::IntrinsicTypeKind::Integer(kind))
            && self.plain_intrinsic_signature(signature)
            && self.one_parameter(function, owner)
            && signature.return_ty == owner
            && signature.modifiers.operator == Some(operator)
            && !signature.modifiers.is_infix
            && self.source_function_name(function) == source_name
            && hir::ManagedCallableRef::try_from_function(function, self.functions.as_arena())
                .is_some();
        if !valid {
            self.malformed_operator_intrinsic(function, operation.registry_key());
        }
    }

    fn validate_integer_conversion_intrinsic(
        &mut self,
        function: FunctionId,
        source: hir::IntegerKind,
        target: hir::IntegerKind,
    ) {
        let signature = &self.signatures[&function];
        let valid = self
            .function_has_intrinsic_owner(function, hir::IntrinsicTypeKind::Integer(source))
            && self.plain_intrinsic_signature(signature)
            && signature.params.is_empty()
            && signature.return_ty == self.integer_type(target)
            && signature.modifiers == hir::CallableModifiers::default()
            && self.source_function_name(function) == conversion_source_name(target)
            && hir::NoGcCallableRef::try_from_function(function, self.functions.as_arena())
                .is_some();
        if !valid {
            self.malformed_operator_intrinsic(
                function,
                &format!("{}_to_{}", source.registry_key(), target.registry_key()),
            );
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
        let long = self.integer_type(hir::IntegerKind::SIGNED_64);
        let signature = &self.signatures[&function];
        let parameters_match = match kind {
            hir::ArrayAccessKind::ImmutableGet | hir::ArrayAccessKind::MutableGet => {
                matches!(signature.params.as_slice(), [index] if index.ty == long)
                    && self.is_type_param(signature.return_ty, 0)
            }
            hir::ArrayAccessKind::MutableSet => {
                matches!(signature.params.as_slice(), [index, value]
                    if index.ty == long && self.is_type_param(value.ty, 0))
                    && signature.return_ty == self.unit
            }
        };
        let valid = self.function_has_intrinsic_owner(function, owner_kind)
            && !signature.is_suspend
            && signature.owner_type_param_count == 1
            && signature.type_params.len() == 1
            && signature.type_params[0].kind() == hir::TypeParamKind::Any
            && parameters_match
            && (signature.modifiers.operator == Some(operator)
                || (operator == hir::OperatorKind::Get && signature.modifiers.operator.is_none()))
            && !signature.modifiers.is_infix;
        if !valid {
            self.malformed_operator_intrinsic(function, kind.name());
        }
    }

    fn plain_intrinsic_signature(&self, signature: &FnSig) -> bool {
        !signature.is_suspend
            && signature.type_params.is_empty()
            && signature.owner_type_param_count == 0
    }

    fn no_parameters(&self, function: FunctionId) -> bool {
        self.signatures[&function].params.is_empty()
    }

    fn one_parameter(&self, function: FunctionId, expected: TypeId) -> bool {
        matches!(self.signatures[&function].params.as_slice(), [parameter] if parameter.ty == expected)
    }

    fn source_function_name(&self, function: FunctionId) -> &str {
        self.functions[function]
            .name
            .rsplit('.')
            .next()
            .expect("function names are non-empty")
    }

    pub(super) fn function_has_intrinsic_owner(
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

    pub(super) fn malformed_operator_intrinsic(&mut self, function: FunctionId, intrinsic: &str) {
        self.current_file = self.function_files[&function];
        self.error(
            self.functions[function].span,
            format!("malformed core operator intrinsic `{intrinsic}`"),
        );
    }
}

fn conversion_source_name(kind: hir::IntegerKind) -> &'static str {
    match kind {
        hir::IntegerKind::SIGNED_8 => "toInt8",
        hir::IntegerKind::SIGNED_16 => "toInt16",
        hir::IntegerKind::SIGNED_32 => "toInt32",
        hir::IntegerKind::SIGNED_64 => "toInt64",
        hir::IntegerKind::UNSIGNED_8 => "toUInt8",
        hir::IntegerKind::UNSIGNED_16 => "toUInt16",
        hir::IntegerKind::UNSIGNED_32 => "toUInt32",
        hir::IntegerKind::UNSIGNED_64 => "toUInt64",
    }
}
