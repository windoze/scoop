use super::*;
use crate::call_resolution::named::NamedFunctionLikeProbe;
use crate::expr::named_calls::imported_dependency::{
    ImportedCallArguments, ImportedMemberReceiver, ImportedProbeCall,
};
use crate::imports::lookup::calls::wire_operator;
use hir::ImportedCallableSource;

pub(in crate::expr) enum ImportedMemberSelectionFailure {
    NoApplicable(Option<Box<Lowerer>>),
    Failed(Box<Lowerer>),
}

impl Lowerer {
    pub(super) fn probe_imported_member_partition(
        &self,
        receiver: hir::Expr,
        name: &ast::Ident,
        call: CallSite<'_>,
        expected: Option<TypeId>,
        required: RequiredCallableModifiers,
    ) -> PropertyExtensionInvokeOutcome {
        let probe = match self.select_imported_member_probe(
            ImportedMemberReceiver::Value(receiver),
            name,
            call.into(),
            expected,
            required,
        ) {
            Ok(probe) => probe,
            Err(ImportedMemberSelectionFailure::NoApplicable(failure)) => {
                return PropertyExtensionInvokeOutcome::NoApplicable(failure);
            }
            Err(ImportedMemberSelectionFailure::Failed(failure)) => {
                return PropertyExtensionInvokeOutcome::Failed(failure);
            }
        };
        let mut state = self.clone();
        let mut sink = Vec::new();
        let Some(expression) = state.commit_imported_dependency_callable(probe, &mut sink) else {
            return PropertyExtensionInvokeOutcome::Failed(Box::new(state));
        };
        PropertyExtensionInvokeOutcome::Resolved(SuccessfulExprLayer {
            state: Box::new(state),
            expression,
            sink,
        })
    }

    pub(in crate::expr) fn select_imported_member_probe(
        &self,
        receiver: ImportedMemberReceiver,
        name: &ast::Ident,
        call: ImportedProbeCall<'_>,
        expected: Option<TypeId>,
        required: RequiredCallableModifiers,
    ) -> Result<ImportedDependencyCallProbe, ImportedMemberSelectionFailure> {
        if self.imported_nominal_declaration(receiver.ty()).is_none() {
            return Err(ImportedMemberSelectionFailure::NoApplicable(None));
        }
        let lookup = match required.operator {
            Some(operator) => hir::ImportedMemberLookup::Operator(
                hir::CallableOperatorRoleV1::Language(wire_operator(operator)),
            ),
            None => hir::ImportedMemberLookup::Name(&name.text),
        };
        let candidates = match self.imported_member_candidates(receiver.ty(), lookup) {
            Ok(candidates) => candidates,
            Err(error) => {
                let mut failure = self.clone();
                failure.error(name.span, format!("invalid imported member: {error}"));
                return Err(ImportedMemberSelectionFailure::Failed(Box::new(failure)));
            }
        };
        let mut probes = Vec::new();
        let mut first_failure = None;
        for candidate in candidates {
            let effects = candidate.interface().effects();
            if required.infix && effects.infix() != hir::CallableInfixV1::Infix {
                continue;
            }
            if let Some(operator) = required.property_delegate_operator
                && effects.operator_role()
                    != hir::CallableOperatorRoleV1::PropertyDelegate(
                        super::extension_calls::imported_delegate_operator(operator),
                    )
            {
                continue;
            }
            match self.probe_imported_member_callable(
                candidate,
                receiver.clone(),
                name,
                call,
                expected,
                required.operator == Some(hir::OperatorKind::Set),
            ) {
                Ok(probe) => {
                    probes.push(NamedFunctionLikeProbe::ImportedDependency(Box::new(probe)))
                }
                Err(failure) => {
                    first_failure.get_or_insert(failure);
                }
            }
        }
        if probes.is_empty() {
            return Err(ImportedMemberSelectionFailure::NoApplicable(first_failure));
        }
        let mut state = self.clone();
        let winner = match call.arguments {
            ImportedCallArguments::Source(arguments) => state
                .select_named_function_like(&name.text, "member", &probes, arguments, call.span),
            ImportedCallArguments::Lowered(_) => {
                state.select_lowered_named_function_like(&name.text, "member", &probes, call.span)
            }
        };
        let Some(winner) = winner else {
            return Err(ImportedMemberSelectionFailure::Failed(Box::new(state)));
        };
        let NamedFunctionLikeProbe::ImportedDependency(probe) = probes.swap_remove(winner) else {
            unreachable!("the imported member partition contains imported call probes")
        };
        Ok(*probe)
    }
}
