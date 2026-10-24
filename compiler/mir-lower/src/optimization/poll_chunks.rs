//! Strip mine bounded scalar kernels before safepoints and roots acquire identity.

use std::collections::BTreeSet;

use scoop_identity::{StructuralDefinitionSiteRole, SyntheticLocalRole};

use super::{inline, mir};
use crate::local_values::LocalValueRegistry;

mod copy;
mod linear;
mod operations;
mod recognize;

const CHUNK: u32 = 64;

struct Loop {
    header: mir::BlockId,
    body: mir::BlockId,
    index: mir::LocalId,
    bound: mir::Expr,
    kind: mir::IntegerKind,
    header_chain: Vec<mir::BlockId>,
    body_chain: Vec<mir::BlockId>,
    temporaries: BTreeSet<mir::LocalId>,
}

pub(super) fn run(module: &mut mir::Module, registry: &mut LocalValueRegistry) {
    let functions = module
        .functions
        .iter()
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    for id in functions {
        let Some(owner) = inline::owner(module, id) else {
            continue;
        };
        let function = &module.functions[id];
        if function.gc_effect != mir::GcEffect::Managed {
            continue;
        }
        let headers = function.body.loop_header_polls.clone();
        for target in headers {
            let Some(counted) = recognize::counted(module, &module.functions[id], target.header())
            else {
                continue;
            };
            let function = &mut module.functions[id];
            linear::merge(&mut function.body, &counted.header_chain);
            linear::merge(&mut function.body, &counted.body_chain);
            let counter = function.body.locals.alloc(mir::Local {
                name: format!("$poll.remaining.{}", function.body.locals.len()),
                ty: mir::Type::Integer(counted.kind),
                mutable: true,
            });
            registry.record_generated_local(
                id,
                counter,
                owner,
                StructuralDefinitionSiteRole::SyntheticValue,
                SyntheticLocalRole::Temporary,
            );
            let full = copy::body(
                function,
                counted.body,
                &counted.temporaries,
                registry,
                owner,
                id,
            );
            expand(&mut function.body, counted, counter, full);
        }
    }
}

fn expand(body: &mut mir::Body, counted: Loop, counter: mir::LocalId, full: mir::BlockId) {
    let Loop {
        header,
        body: inner,
        index,
        bound,
        kind,
        ..
    } = counted;
    let ty = mir::Type::Integer(kind);
    let span = body.blocks[inner]
        .statements
        .last()
        .expect("counted loop updates its index")
        .span;
    let constant = |value| {
        mir::Expr::integer(if kind == mir::IntegerKind::SIGNED_32 {
            mir::MirIntegerConstant::Signed32(value)
        } else {
            mir::MirIntegerConstant::Signed64(u64::from(value))
        })
    };
    let read = || mir::Expr::local(counter, ty.clone());
    let compare = |operator, rhs| {
        mir::Expr::integer_compare(
            mir::IntegerComparisonOperation::new(kind, operator),
            read(),
            rhs,
        )
    };
    let clamp = body.blocks.alloc(mir::BasicBlock {
        name: format!("poll.clamp.{}", body.blocks.len()),
        statements: vec![mir::Statement {
            kind: mir::StatementKind::Assign {
                local: counter,
                value: constant(CHUNK),
            },
            span,
        }],
        terminator: mir::Terminator::Goto(full),
        unwind: None,
    });
    let chunk = body.blocks.alloc(mir::BasicBlock {
        name: format!("poll.chunk.{}", body.blocks.len()),
        statements: vec![mir::Statement {
            kind: mir::StatementKind::ValDecl {
                local: counter,
                init: mir::Expr::integer_binary(
                    mir::IntegerBinaryOperation::new(kind, mir::IntegerBinaryOperator::Subtract),
                    bound,
                    mir::Expr::local(index, ty.clone()),
                ),
            },
            span,
        }],
        terminator: mir::Terminator::Branch {
            cond: compare(
                mir::IntegerComparisonOperator::GreaterThanOrEqual,
                constant(CHUNK),
            ),
            then_block: clamp,
            else_block: inner,
        },
        unwind: None,
    });
    let mir::Terminator::Branch { then_block, .. } = &mut body.blocks[header].terminator else {
        unreachable!("recognized conditional header")
    };
    *then_block = chunk;
    for inner in [inner, full] {
        body.blocks[inner].statements.push(mir::Statement {
            kind: mir::StatementKind::Assign {
                local: counter,
                value: mir::Expr::integer_binary(
                    mir::IntegerBinaryOperation::new(kind, mir::IntegerBinaryOperator::Subtract),
                    read(),
                    constant(1),
                ),
            },
            span,
        });
        body.blocks[inner].terminator = mir::Terminator::Branch {
            cond: compare(mir::IntegerComparisonOperator::NotEqual, constant(0)),
            then_block: inner,
            else_block: header,
        };
    }
    // The full inner has a constant countdown; the scalar tail consumes the
    // positive remaining count. Only the original header remains a poll target.
}
