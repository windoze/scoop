//! Intrinsics without indirect-call semantics retain a distinct diagnostic.

use super::*;
use hir::ImportedCallableSource;
use std::fmt;

pub(super) enum ReferenceFailureReason {
    Candidate(String),
    Intrinsic {
        target: hir::SemanticCallable,
        message: String,
    },
}

impl From<String> for ReferenceFailureReason {
    fn from(message: String) -> Self {
        Self::Candidate(message)
    }
}

impl fmt::Display for ReferenceFailureReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Candidate(message) | Self::Intrinsic { message, .. } => message,
        };
        formatter.write_str(message)
    }
}

fn restriction(kind: hir::IntrinsicFunctionKind) -> Option<&'static str> {
    match kind {
        hir::IntrinsicFunctionKind::IncludeStr | hir::IntrinsicFunctionKind::IncludeBytes => Some(
            "this intrinsic is evaluated at compile time and has no runtime callable entry\nhelp: import and call it directly with a constant path",
        ),
        hir::IntrinsicFunctionKind::CompileEnv => Some(
            "this intrinsic is evaluated at compile time and has no runtime callable entry\nhelp: import and call it directly with a constant name",
        ),
        hir::IntrinsicFunctionKind::CurrentSourceLocation => Some(
            "this intrinsic requires the source location of a direct call; indirect calls do not provide it\nhelp: call it directly where the location is needed, or pass a SourceLocation value explicitly",
        ),
        _ => None,
    }
}

fn failure(target: hir::SemanticCallable, name: &str, reason: &str) -> ReferenceFailureReason {
    ReferenceFailureReason::Intrinsic {
        target,
        message: format!(
            "compiler intrinsic `{name}` does not support callable references\nnote: {reason}"
        ),
    }
}

impl Lowerer {
    pub(super) fn local_intrinsic_reference_failure(
        &self,
        function: hir::FunctionId,
    ) -> Option<ReferenceFailureReason> {
        let hir::FunctionKind::Intrinsic(intrinsic) = self.functions[function].kind else {
            return None;
        };
        Some(failure(
            hir::SemanticCallable::Current(function),
            &self.functions[function].name,
            restriction(intrinsic.kind)?,
        ))
    }

    pub(super) fn imported_intrinsic_reference_failure(
        &self,
        source: &impl ImportedCallableSource,
    ) -> Option<ReferenceFailureReason> {
        let interface = source.interface();
        let hir::CallableImplementationV1::Intrinsic(kind) = interface.effects().implementation()
        else {
            return None;
        };
        let reason = restriction(kind)?;
        let target =
            super::super::named_calls::imported_dependency::semantic_function_target(source)
                .expect("a restricted function intrinsic retains its function declaration");
        let declaration = self
            .dependencies
            .as_ref()
            .expect("an imported reference has a dependency catalog")
            .callable_declaration(interface.declaration())
            .expect("an imported candidate retains its declaration");
        Some(failure(target, declaration.name(), reason))
    }

    pub(super) fn report_intrinsic_reference_failure(
        &mut self,
        span: Span,
        reason: &ReferenceFailureReason,
    ) -> bool {
        let ReferenceFailureReason::Intrinsic { target, message } = reason else {
            return false;
        };
        self.observe_rejected_callable_reference(span, *target);
        self.error(span, message.clone());
        true
    }
}
