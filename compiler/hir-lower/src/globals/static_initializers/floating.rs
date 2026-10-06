use super::super::consts::{ResolvedConstFloatIntrinsic, evaluate_float_call};
use super::*;

impl Lowerer {
    pub(super) fn evaluate_static_float_call(
        &mut self,
        resolved: ResolvedConstFloatIntrinsic,
        receiver: StaticValue,
        arguments: &[ast::CallArgument],
    ) -> Option<StaticValue> {
        if !resolved.arguments_match(arguments) {
            return None;
        }
        let right = if let [argument] = arguments {
            let right = self.evaluate_static_value(&argument.expression, Some(receiver.ty))?;
            if !self.types_equal(receiver.ty, right.ty) {
                return None;
            }
            Some(right.value)
        } else {
            None
        };
        let value = evaluate_float_call(resolved.kind, receiver.value, right);
        let ty = self.float_const_result_type(&value)?;
        Some(StaticValue { value, ty })
    }
}
