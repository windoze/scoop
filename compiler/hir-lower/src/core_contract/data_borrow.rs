use super::*;
use crate::types::ArrayKind;

impl Lowerer {
    pub(crate) fn validate_data_borrow_intrinsics(&mut self) {
        for kind in hir::DataBorrowIntrinsic::ALL {
            let Some(&(function, _)) = self
                .intrinsic_functions
                .get(&hir::IntrinsicFunctionKind::DataBorrow(kind))
            else {
                continue;
            };
            if !self.valid_data_borrow_signature(function, kind) {
                self.current_file = self.function_files[&function];
                self.error(
                    self.functions[function].span,
                    format!("malformed core data borrow intrinsic `{}`", kind.name()),
                );
            }
        }
    }

    fn valid_data_borrow_signature(
        &self,
        function: hir::FunctionId,
        kind: hir::DataBorrowIntrinsic,
    ) -> bool {
        let signature = &self.signatures[&function];
        let result_index = kind.type_parameter_count() - 1;
        if signature.is_suspend
            || signature.owner_type_param_count != 0
            || signature.type_params.len() != kind.type_parameter_count() as usize
            || self.function_owner.contains_key(&function)
            || self.extension_receivers.contains_key(&function)
            || !self.is_type_param(signature.return_ty, result_index)
            || signature.type_params[result_index as usize].kind() != hir::TypeParamKind::Any
        {
            return false;
        }
        let [object, block] = signature.params.as_slice() else {
            return false;
        };
        let Type::Function(block) = self.types[block.ty] else {
            return false;
        };
        let block = &self.function_types[block];
        let [pointer, length] = block.parameter_types.as_slice() else {
            return false;
        };
        let Type::Ptr(element) = self.types[*pointer] else {
            return false;
        };
        if block.is_suspend
            || block.return_type != signature.return_ty
            || !matches!(
                self.types[*length],
                Type::Integer(hir::IntegerKind::SIGNED_64)
            )
        {
            return false;
        }
        match kind {
            hir::DataBorrowIntrinsic::String => {
                matches!(self.types[object.ty], Type::String)
                    && matches!(
                        self.types[element],
                        Type::Integer(hir::IntegerKind::UNSIGNED_8)
                    )
            }
            hir::DataBorrowIntrinsic::Array | hir::DataBorrowIntrinsic::MutableArray => {
                let expected = if kind == hir::DataBorrowIntrinsic::Array {
                    ArrayKind::Immutable
                } else {
                    ArrayKind::Mutable
                };
                signature.type_params[0].kind() == hir::TypeParamKind::Value
                    && self.is_type_param(element, 0)
                    && self
                        .array_type_info(object.ty)
                        .is_some_and(|array| array.kind == expected && array.element == element)
            }
        }
    }
}
