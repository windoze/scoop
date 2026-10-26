use super::{Constant, mir};

/// Fold only actual typed operations over immutable, already known operands.
pub(in crate::optimization) fn call(
    module: &mir::Module,
    call: &mir::Call,
    value: impl Fn(&mir::Expr) -> Option<Constant>,
) -> Option<Constant> {
    let mir::CallTarget::Direct(mir::Callee::Runtime(operation)) = call.target else {
        return None;
    };
    let string = |expression: &mir::Expr| {
        let Constant::String(id) = value(expression)? else {
            return None;
        };
        Some(module.strings[id].value.as_bytes())
    };
    use mir::MirIntegerConstant as I;
    let result = match (operation, call.args.as_slice()) {
        (mir::RuntimeFn::StringByteLength, [source]) => I::Signed64(string(source)?.len() as u64),
        (mir::RuntimeFn::StringByteAt, [source, index]) => {
            let Constant::Integer(I::Signed64(index)) = value(index)? else {
                return None;
            };
            // Invalid indices retain the original operation and fatal diagnostic.
            I::Signed8(*string(source)?.get(usize::try_from(index).ok()?)?)
        }
        (mir::RuntimeFn::StringCompare, [left, right]) => {
            let order = match string(left)?.cmp(string(right)?) {
                std::cmp::Ordering::Less => -1_i64,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            };
            I::Signed64(order as u64)
        }
        _ => return None,
    };
    Some(Constant::Integer(result))
}
