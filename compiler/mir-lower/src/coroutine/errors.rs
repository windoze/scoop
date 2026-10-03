use super::*;

pub(super) fn protocol_error_block(
    lowerer: &Lowerer,
    module: &hir::Module,
    locals: &mut Arena<mir::Local>,
    blocks: &mut Arena<mir::BasicBlock>,
    unwind: Option<mir::BlockId>,
) -> mir::BlockId {
    let (mir_class, initializer) = match &module.core_protocols {
        hir::ConcreteCoreProtocols::Defined(protocols) => {
            let exception = protocols.exceptions.illegal_state_exception;
            (
                lowerer.class_map[&exception.class()],
                mir::Callee::User(lowerer.ctors[&exception.callable()]),
            )
        }
        hir::ConcreteCoreProtocols::Imported(protocols) => {
            let declaration = protocols
                .exceptions()
                .illegal_state_exception()
                .persistent();
            let class = module
                .classes
                .iter()
                .find_map(|(id, class)| {
                    (class.origin.concrete_type_id() == Some(declaration)).then_some(id)
                })
                .expect("the coroutine failure type was concretized by HIR");
            let (provider, target) = crate::current::runtime_constructor_target(
                protocols.exceptions().illegal_state_exception_constructor(),
            )
            .expect("the checked coroutine exception has an initializer");
            let callable = lowerer
                .external_callables
                .iter()
                .find_map(|(id, callable)| {
                    (callable.reference().provider() == provider
                        && callable.reference().implementation() == target)
                        .then_some(id)
                })
                .expect("the coroutine failure initializer was selected from its provider");
            (lowerer.class_map[&class], mir::Callee::External(callable))
        }
    };
    let exception = locals.alloc(local("$protocol_error", mir::Type::Class(mir_class)));
    blocks.alloc(mir::BasicBlock {
        name: "coroutine.protocol_error".to_string(),
        statements: vec![
            statement(mir::StatementKind::ValDecl {
                local: exception,
                init: mir::Expr::new(
                    mir::Type::Class(mir_class),
                    mir::ExprKind::ClassAlloc {
                        class_id: mir_class,
                    },
                ),
            }),
            statement(mir::StatementKind::Call(mir::CallEffect::Unit(mir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee: initializer,
                },
                args: vec![mir::Expr::local(exception, mir::Type::Class(mir_class))],
                pending: mir::CoroutinePendingContext::Root,
            }))),
        ],
        terminator: mir::Terminator::Throw {
            exception: mir::Expr::local(exception, mir::Type::Class(mir_class)),
            unwind,
        },
        unwind,
    })
}
