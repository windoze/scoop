//! Integer literal preferences apply after the declaration MSC pool.

use scoop_ast as ast;
use scoop_hir::IntegerKind;

pub(crate) fn prefer_literal_defaults(
    candidates: &[usize],
    arguments: &[ast::CallArgument],
    kind: impl Fn(usize, usize) -> Option<IntegerKind>,
) -> Vec<usize> {
    let dominates = |preferred, other| {
        let mut better = false;
        for (index, argument) in arguments.iter().enumerate() {
            let Some(default) = crate::expr::integer_literal_default_kind(&argument.expression)
            else {
                continue;
            };
            let (Some(preferred), Some(other)) = (kind(preferred, index), kind(other, index))
            else {
                continue;
            };
            match (preferred == default, other == default) {
                (true, false) => better = true,
                (false, true) => return false,
                _ => {}
            }
        }
        better
    };
    candidates
        .iter()
        .copied()
        .filter(|&candidate| {
            !candidates
                .iter()
                .copied()
                .any(|other| other != candidate && dominates(other, candidate))
        })
        .collect()
}
