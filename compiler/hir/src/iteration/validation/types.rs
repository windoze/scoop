use super::*;

mod callable;
mod receiver;
mod relations;

#[derive(Clone)]
struct CallableInfo {
    function: FunctionId,
    is_suspend: bool,
    parameter_types: Vec<TypeId>,
    return_ty: TypeId,
    receiver: Option<ReceiverType>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReceiverType {
    Exact(TypeId),
    Parameter(TypeParamId),
}
