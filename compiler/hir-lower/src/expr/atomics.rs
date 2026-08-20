use super::*;

pub(super) const ATOMIC_REFERENCE_ERROR: &str = "atomic methods with memory-order parameters cannot be referenced; use a lambda with fixed orders";

impl Lowerer {
    pub(in crate::expr) fn normalize_atomic_method(
        &mut self,
        intrinsic: hir::AtomicIntrinsic,
        receiver: hir::Expr,
        arguments: Vec<hir::Expr>,
        ty: TypeId,
        span: Span,
        sink: &[hir::Statement],
    ) -> Option<hir::Expr> {
        use hir::{AtomicMethod as Method, AtomicOperation as Operation};
        let method = intrinsic.method();
        let value_count = method.value_parameter_count();
        let orders = arguments[value_count..]
            .iter()
            .map(|value| self.constant_atomic_order(value, sink))
            .collect::<Option<Vec<_>>>()?;
        let mut values = arguments.into_iter().take(value_count);
        let operation = match method {
            Method::Load => {
                let Some(order) = hir::AtomicLoadOrder::from_memory_order(orders[0]) else {
                    self.error(span, format!("memory order {:?} is not valid for atomic load; expected Relaxed, Acquire, or SeqCst", orders[0]));
                    return None;
                };
                Operation::Load { order }
            }
            Method::Store => {
                let Some(order) = hir::AtomicStoreOrder::from_memory_order(orders[0]) else {
                    self.error(span, format!("memory order {:?} is not valid for atomic store; expected Relaxed, Release, or SeqCst", orders[0]));
                    return None;
                };
                Operation::Store {
                    value: values.next().expect("store has a value"),
                    order,
                }
            }
            Method::CompareAndSet | Method::CompareAndExchange => {
                let Some(order) =
                    hir::AtomicCompareExchangeOrder::from_memory_orders(orders[0], orders[1])
                else {
                    self.error(
                        span,
                        format!(
                            "atomic CAS failure order {:?} is not allowed with success order {:?}",
                            orders[1], orders[0]
                        ),
                    );
                    return None;
                };
                Operation::CompareExchange {
                    expected: values.next().expect("CAS has an expected value"),
                    value: values.next().expect("CAS has a replacement value"),
                    order,
                    result: if method == Method::CompareAndSet {
                        hir::AtomicCompareExchangeResult::Success
                    } else {
                        hir::AtomicCompareExchangeResult::ObservedValue
                    },
                }
            }
            Method::Exchange
            | Method::FetchAdd
            | Method::FetchSub
            | Method::FetchAnd
            | Method::FetchOr
            | Method::FetchXor => Operation::Rmw {
                value: values.next().expect("RMW has a value"),
                operation: match method {
                    Method::Exchange => hir::AtomicRmwOperation::Exchange,
                    Method::FetchAdd => hir::AtomicRmwOperation::Add,
                    Method::FetchSub => hir::AtomicRmwOperation::Subtract,
                    Method::FetchAnd => hir::AtomicRmwOperation::And,
                    Method::FetchOr => hir::AtomicRmwOperation::Or,
                    Method::FetchXor => hir::AtomicRmwOperation::Xor,
                    _ => unreachable!("this branch contains only RMW methods"),
                },
                order: orders[0],
            },
        };
        Some(hir::Expr {
            kind: ExprKind::Atomic(Box::new(hir::AtomicExpression {
                object: receiver,
                kind: intrinsic.family(),
                operation,
            })),
            ty,
            span,
            origin: self.expression_origin(span),
        })
    }

    fn constant_atomic_order(
        &mut self,
        argument: &hir::Expr,
        sink: &[hir::Statement],
    ) -> Option<hir::AtomicMemoryOrder> {
        let mut source = argument;
        let mut preceding = sink;
        while let ExprKind::Local(local) = source.kind {
            if self.locals[local].definition != hir::LocalValueDefinitionSite::Synthetic {
                break;
            }
            let Some((index, init)) =
                preceding
                    .iter()
                    .enumerate()
                    .rev()
                    .find_map(|(index, statement)| {
                        let hir::StatementKind::ValDecl { pattern, init } = &statement.kind else {
                            return None;
                        };
                        matches!(pattern, hir::Pattern::Binding { local: bound } if *bound == local)
                            .then_some((index, init))
                    })
            else {
                break;
            };
            source = init;
            preceding = &preceding[..index];
        }
        if let ExprKind::VariantConstruct { variant, args } = &source.kind
            && args.is_empty()
            && let Some(order) = self.atomic_memory_order(*variant)
        {
            return Some(order);
        }
        self.error(
            argument.span,
            "atomic memory order must be a constant MemoryOrder variant".into(),
        );
        None
    }
}
