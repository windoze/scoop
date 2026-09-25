use super::{ImportedCallReceiver, ImportedDependencyCallProbe, ImportedMemberReceiver};
use crate::Lowerer;
use hir::ImportedCallableSource;
use scoop_hir as hir;

impl Lowerer {
    pub(in crate::expr) fn commit_imported_literal_equality(
        &mut self,
        probe: ImportedDependencyCallProbe,
    ) -> (hir::Expr, hir::LiteralPatternEquality) {
        let ImportedDependencyCallProbe {
            state,
            candidate,
            receiver,
            source_args,
            argument_sinks,
            call_span,
            ..
        } = probe;
        let ImportedCallReceiver::Member {
            value: ImportedMemberReceiver::LiteralSubject(_),
            ..
        } = receiver
        else {
            unreachable!("literal equality probes retain their explicit subject type")
        };
        let kind = candidate
            .integer_equality_kind()
            .expect("literal equality probing establishes a complete normalization plan");
        let [literal] = source_args.as_slice() else {
            unreachable!("an applicable literal equals member has one explicit argument")
        };
        assert!(
            argument_sinks.iter().all(Vec::is_empty),
            "lowered literals have no source evaluation sinks"
        );
        *self = *state;
        if candidate.interface().effects().safety() == hir::CallableSafetyV1::Unsafe {
            self.require_unsafe_operation(call_span, "calling an unsafe dependency function");
        }
        (
            literal.clone(),
            hir::LiteralPatternEquality::Integer { kind },
        )
    }
}
