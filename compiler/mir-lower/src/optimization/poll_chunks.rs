//! Strip mine scalar counted loops before safepoints and roots acquire identity.

use scoop_identity::{StructuralDefinitionSiteRole, SyntheticLocalRole};

use super::{inline, mir};
use crate::local_values::LocalValueRegistry;

mod recognize;

const CHUNK: u32 = 64;

struct Loop {
    header: mir::BlockId,
    body: mir::BlockId,
    index: mir::LocalId,
    bound: mir::Expr,
    kind: mir::IntegerKind,
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
        let function = &mut module.functions[id];
        if function.gc_effect != mir::GcEffect::Managed {
            continue;
        }
        let headers = function.body.loop_header_polls.clone();
        for target in headers {
            let Some(counted) = recognize::counted(function, target.header()) else {
                continue;
            };
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
            expand(&mut function.body, counted, counter);
        }
    }
}

fn expand(body: &mut mir::Body, counted: Loop, counter: mir::LocalId) {
    let Loop {
        header,
        body: inner,
        index,
        bound,
        kind,
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
        terminator: mir::Terminator::Goto(inner),
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
            cond: compare(mir::IntegerComparisonOperator::GreaterThan, constant(CHUNK)),
            then_block: clamp,
            else_block: inner,
        },
        unwind: None,
    });
    let mir::Terminator::Branch { then_block, .. } = &mut body.blocks[header].terminator else {
        unreachable!("recognized conditional header")
    };
    *then_block = chunk;
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
    // Only the original header remains a poll target. The new self-loop has a
    // positive countdown initialized to min(CHUNK, bound - index).
}
