//! The private five-parameter callback storage boundary.

use super::*;

pub(super) struct Storage {
    pub(super) params: Vec<mir::Param>,
    pub(super) snapshot: mir::Expr,
    pub(super) result: mir::Expr,
    pub(super) exception: mir::Expr,
    closure: mir::Expr,
    arguments: mir::Expr,
}

impl Storage {
    pub(super) fn new(
        core: mir::ConeIdentity,
        managed: mir::FunctionTypeId,
        signature: &mir::FunctionType,
        throwable: mir::Type,
        locals: &mut Arena<mir::Local>,
    ) -> Self {
        let mut params = Vec::new();
        let mut parameter = |name: &str, ty: mir::Type| {
            let local = locals.alloc(mir::Local {
                name: name.to_string(),
                ty: ty.clone(),
                mutable: false,
            });
            params.push(mir::Param {
                name: name.to_string(),
                ty: ty.clone(),
                local,
            });
            mir::Expr::local(local, ty)
        };
        let closure = parameter("$closure", mir::Type::Function(managed));
        let snapshot = parameter(
            "$snapshot",
            mir::Type::Context(mir::ContextStorageType::new(
                core,
                mir::ContextStorageRole::Node,
            )),
        );
        let result = parameter(
            "$result",
            mir::Type::Ptr(Box::new(signature.return_type.clone())),
        );
        let arguments = parameter(
            "$arguments",
            mir::Type::Ptr(Box::new(mir::Type::Ptr(Box::new(mir::Type::Unit)))),
        );
        let exception = parameter("$exception", mir::Type::Ptr(Box::new(throwable)));
        Self {
            params,
            snapshot,
            result,
            exception,
            closure,
            arguments,
        }
    }

    pub(super) fn call_arguments(&self, signature: &mir::FunctionType) -> Vec<mir::Expr> {
        let mut args = vec![self.closure.clone()];
        let opaque_pointer = mir::Type::Ptr(Box::new(mir::Type::Unit));
        for (index, parameter) in signature.parameter_types.iter().enumerate() {
            let raw = mir::Expr::new(
                opaque_pointer.clone(),
                mir::ExprKind::PtrLoad {
                    pointer: Box::new(self.arguments.clone()),
                    pointee: Box::new(opaque_pointer.clone()),
                    offset: Some(Box::new(mir::Expr::machine_scalar(
                        mir::MachineScalarValue::PointerElementOffset(
                            u64::try_from(index).expect("callback argument index fits u64"),
                        ),
                    ))),
                },
            );
            args.push(mir::Expr::new(
                parameter.clone(),
                mir::ExprKind::PtrLoad {
                    pointer: Box::new(mir::Expr::new(
                        mir::Type::Ptr(Box::new(parameter.clone())),
                        mir::ExprKind::PtrCast {
                            operand: Box::new(raw),
                            pointee: Box::new(parameter.clone()),
                        },
                    )),
                    pointee: Box::new(parameter.clone()),
                    offset: None,
                },
            ));
        }
        args
    }
}
