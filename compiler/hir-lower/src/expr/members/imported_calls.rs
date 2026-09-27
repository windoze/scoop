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
    pub(super) fn imported_member_call_candidates(
        &mut self,
        receiver: TypeId,
        name: &ast::Ident,
        required: RequiredCallableModifiers,
        kind: MemberCallKind,
    ) -> Result<Vec<hir::ImportedCallableDeclaration>, Box<Lowerer>> {
        self.resolve_imported_member_receiver(receiver, name.span)
            .map_err(|()| Box::new(self.clone()))?;
        let lookup = match required.operator {
            Some(operator) => hir::ImportedMemberLookup::Operator(
                hir::CallableOperatorRoleV1::Language(wire_operator(operator)),
            ),
            None => hir::ImportedMemberLookup::Name(&name.text),
        };
        let candidates = self
            .imported_member_candidates_for_receiver(
                receiver,
                (kind == MemberCallKind::Ordinary).then_some(receiver),
                lookup,
            )
            .map_err(|error| {
                let mut failure = self.clone();
                failure.error(name.span, format!("invalid imported member: {error}"));
                Box::new(failure)
            })?;
        let class_super = kind == MemberCallKind::DirectSuper
            && matches!(
                self.types[receiver],
                Type::Class(_) | Type::ImportedClass(_)
            );
        Ok(candidates
            .into_iter()
            .filter(|candidate| {
                if class_super {
                    let hir::PublicDeclarationOwnerV1::Nominal(hir::SourceNominalId::Concrete(
                        owner,
                    )) = candidate.interface().owner()
                    else {
                        return false;
                    };
                    if !self
                        .dependencies
                        .as_ref()
                        .and_then(|dependencies| dependencies.nominal(owner))
                        .is_some_and(|declaration| {
                            matches!(
                                declaration.interface.source_shape(),
                                hir::NominalSourceShapeV1::Class(_)
                                    | hir::NominalSourceShapeV1::Object(_)
                            )
                        })
                    {
                        return false;
                    }
                }
                let effects = candidate.interface().effects();
                (!required.infix || effects.infix() == hir::CallableInfixV1::Infix)
                    && required.property_delegate_operator.is_none_or(|operator| {
                        effects.operator_role()
                            == hir::CallableOperatorRoleV1::PropertyDelegate(
                                super::extension_calls::imported_delegate_operator(operator),
                            )
                    })
            })
            .collect())
    }

    pub(in crate::expr) fn select_imported_member_probe(
        &self,
        receiver: ImportedMemberReceiver,
        name: &ast::Ident,
        call: ImportedProbeCall<'_>,
        expected: Option<TypeId>,
        required: RequiredCallableModifiers,
    ) -> Result<ImportedDependencyCallProbe, ImportedMemberSelectionFailure> {
        let mut context = self.clone();
        let candidates = context
            .imported_member_call_candidates(
                receiver.ty(),
                name,
                required,
                MemberCallKind::Ordinary,
            )
            .map_err(ImportedMemberSelectionFailure::Failed)?;
        let mut probes = Vec::new();
        let mut first_failure = None;
        for candidate in candidates {
            match context.probe_imported_member_callable(
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
        let mut state = context;
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
