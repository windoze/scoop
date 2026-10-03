use super::{
    ImportedCallReceiver, ImportedCallableCandidate, ImportedDependencyCallProbe,
    ImportedMemberReceiver,
};
use crate::Lowerer;
use hir::ImportedCallableSource;
use scoop_hir as hir;

impl Lowerer {
    pub(in crate::expr) fn commit_imported_literal_equality(
        &mut self,
        probe: ImportedDependencyCallProbe,
    ) -> Option<(hir::Expr, hir::LiteralPatternEquality)> {
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
        let integer = candidate.integer_equality_kind();
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
        let equality = if let Some(kind) = integer {
            hir::LiteralPatternEquality::Integer { kind }
        } else {
            let selected = match candidate {
                ImportedCallableCandidate::Binding(candidate) => self
                    .select_imported_dependency_callable_use(*candidate)
                    .map(|(callee, _)| callee),
                ImportedCallableCandidate::Declaration(candidate) => self
                    .select_imported_callable_declaration_use_with_kind(
                        *candidate,
                        crate::expr::MemberCallKind::Ordinary,
                    ),
            };
            let callee = match selected {
                Ok(callee) => callee,
                Err(error) => {
                    self.error(call_span, error.to_string());
                    return None;
                }
            };
            hir::LiteralPatternEquality::Ordinary {
                equals: hir::CallableTarget::Dependency(callee),
            }
        };
        Some((literal.clone(), equality))
    }
}
