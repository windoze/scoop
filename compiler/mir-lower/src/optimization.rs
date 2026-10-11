//! Local optimizations over complete MIR, before output validation and LIR.

use scoop_mir as mir;

mod constants;
mod dispatch;
mod flow;
mod graph;
mod inline;
mod simplify;

/// Producer choices; they do not change entity or ABI identity.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MirOptimizationOptions {
    pub devirtualize: bool,
    pub inline: bool,
}

impl MirOptimizationOptions {
    pub const RELEASE: Self = Self {
        devirtualize: true,
        inline: true,
    };
}

pub(super) fn run(
    module: &mut mir::Module,
    selected: &mir::SelectedExternalMirSet,
    options: MirOptimizationOptions,
) {
    if options.devirtualize {
        devirtualize(module, selected);
    }
    if options.inline {
        inline::run(module);
    }
    if options.devirtualize && options.inline {
        devirtualize(module, selected);
    }
}

struct Rewrite {
    function: mir::FunctionId,
    block: mir::BlockId,
    statement: usize,
    call: mir::Call,
}

fn devirtualize(module: &mut mir::Module, selected: &mir::SelectedExternalMirSet) {
    let mut rewrites = Vec::new();
    for (function_id, function) in module.functions.iter() {
        if !function.body.blocks.iter().any(|(_, block)| {
            block.statements.iter().any(|statement| {
                matches!(&statement.kind, mir::StatementKind::Call(effect)
                    if !matches!(call(effect).target, mir::CallTarget::Direct(_)))
            })
        }) {
            continue;
        }
        let analysis = flow::Analysis::new(module, function);
        let incoming = analysis.solve();
        for (block_id, block) in function.body.blocks.iter() {
            let Some(mut state) = incoming[block_id.into_raw().into_u32() as usize].clone() else {
                continue;
            };
            for (statement_index, statement) in block.statements.iter().enumerate() {
                if let mir::StatementKind::Call(effect) = &statement.kind {
                    let call = call(effect);
                    let result = match effect {
                        mir::CallEffect::Unit(_) => &mir::Type::Unit,
                        mir::CallEffect::Value { destination, .. } => {
                            &function.body.locals[*destination].ty
                        }
                    };
                    if let Some(receiver) = call.target.receiver()
                        && let Some((target, receiver_ty)) = dispatch::resolve(
                            module,
                            selected,
                            call,
                            result,
                            analysis.expression(receiver, &state).actual,
                        )
                    {
                        rewrites.push(Rewrite {
                            function: function_id,
                            block: block_id,
                            statement: statement_index,
                            call: {
                                let receiver = if receiver.ty == receiver_ty {
                                    receiver.clone()
                                } else {
                                    mir::Expr::new(
                                        receiver_ty.clone(),
                                        mir::ExprKind::Retype {
                                            operand: Box::new(receiver.clone()),
                                            ty: Box::new(receiver_ty),
                                        },
                                    )
                                };
                                mir::Call {
                                    target: mir::CallTarget::Direct(target.into()),
                                    args: std::iter::once(receiver)
                                        .chain(call.args.iter().cloned())
                                        .collect(),
                                }
                            },
                        });
                    }
                }
                analysis.statement(statement, &mut state);
            }
        }
    }
    for rewrite in rewrites {
        let statement = &mut module.functions[rewrite.function].body.blocks[rewrite.block]
            .statements[rewrite.statement];
        let mir::StatementKind::Call(effect) = &mut statement.kind else {
            unreachable!("only existing calls are rewritten")
        };
        let call = match effect {
            mir::CallEffect::Unit(call) | mir::CallEffect::Value { call, .. } => call,
        };
        *call = rewrite.call;
    }
}

fn call(effect: &mir::CallEffect) -> &mir::Call {
    match effect {
        mir::CallEffect::Unit(call) | mir::CallEffect::Value { call, .. } => call,
    }
}
