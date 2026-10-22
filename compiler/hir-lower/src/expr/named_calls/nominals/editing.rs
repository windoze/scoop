use super::*;
use crate::call_resolution::applicability::DeclarationTypeArguments;
use crate::call_resolution::candidates::{ArgumentMode, DeclarationSignature};
use crate::editing::{
    EditingCancelled,
    candidates::{Candidate, Candidates, inference::CandidateInput},
};
use hir::{EditingUnavailable as U, EditingValue as V, ImportedCallableSource};

mod constructors;

impl Lowerer {
    pub(crate) fn editing_named_nominal_candidates(
        &mut self,
        binding: &NamedCallBinding,
        call: &ast::RecoveredCall,
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<V<Vec<hir::EditingCallCandidate>>, EditingCancelled> {
        let header = header(call);
        if let (
            NamedCallTarget::ImportedDependency(hir::ImportedTarget::EnumVariant(_)),
            NamedCallOrigin::Dependency(binding),
        ) = (&binding.target, &binding.origin)
        {
            let declaration = self.dependencies.as_ref().and_then(|dependencies| {
                dependencies
                    .callable_candidate(binding)
                    .ok()
                    .and_then(|candidate| {
                        dependencies
                            .callable_declaration(candidate.interface().declaration())
                            .ok()
                    })
            });
            let mut result = Candidates::default();
            result.add(match declaration {
                Some(declaration) => {
                    self.editing_imported_constructor(call, &declaration, None, None)
                }
                None => Err(U::DeclarationFailure),
            });
            return Ok(result.finish());
        }
        if matches!(
            binding.target,
            NamedCallTarget::Function(_)
                | NamedCallTarget::ExtensionProperty(_)
                | NamedCallTarget::Value(ValueTarget::Property(_))
                | NamedCallTarget::ImportedDependency(
                    hir::ImportedTarget::Property(_)
                        | hir::ImportedTarget::ExtensionProperty(_)
                        | hir::ImportedTarget::ObjectValue(_)
                )
        ) {
            return Ok(V::Known(Vec::new()));
        }
        let prepared = match self.named_nominal_plans(binding, header, None) {
            Ok(prepared) => prepared,
            Err(()) => return Ok(V::Unavailable(U::DeclarationFailure)),
        };
        self.editing_prepared_nominal_candidates(call, prepared, is_cancelled)
    }

    pub(crate) fn editing_lexical_nominal_candidates(
        &mut self,
        nominal: crate::NominalTarget,
        call: &ast::RecoveredCall,
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<V<Vec<hir::EditingCallCandidate>>, EditingCancelled> {
        let prepared = match self.nominal_constructor_plans(nominal, header(call), None, false) {
            Ok(prepared) => prepared,
            Err(()) => return Ok(V::Unavailable(U::DeclarationFailure)),
        };
        self.editing_prepared_nominal_candidates(call, prepared, is_cancelled)
    }

    fn editing_prepared_nominal_candidates(
        &mut self,
        call: &ast::RecoveredCall,
        prepared: PreparedNominalPlans,
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<V<Vec<hir::EditingCallCandidate>>, EditingCancelled> {
        let mut result = Candidates::default();
        match prepared {
            PreparedNominalPlans::Nominal(plans, pointer) => {
                for plan in plans {
                    if is_cancelled() {
                        return Err(EditingCancelled);
                    }
                    result.add(self.editing_nominal_signature(
                        call,
                        plan.view.signature,
                        plan.view.argument_mode,
                        plan.expected,
                        plan.alias.as_ref(),
                    ));
                }
                if let Some((owner, expected, _, alias)) = pointer {
                    result.add(self.editing_pointer_signature(
                        call,
                        crate::call_resolution::named::NamedIntrinsicStructOrigin::Current(owner),
                        expected,
                        alias.as_ref(),
                    ));
                }
            }
            PreparedNominalPlans::Imported {
                owner,
                expected,
                alias,
                ..
            } => {
                let Some(declaration) = self
                    .dependencies
                    .as_ref()
                    .and_then(|dependencies| dependencies.nominal_declaration(owner))
                else {
                    return Ok(V::Unavailable(U::DeclarationFailure));
                };
                if let hir::NominalSourceShapeV1::Intrinsic(representation) =
                    declaration.interface.source_shape()
                    && representation.family() == hir::IntrinsicTypeKind::Ptr
                {
                    result.add(self.editing_pointer_signature(
                        call,
                        crate::call_resolution::named::NamedIntrinsicStructOrigin::Imported(owner),
                        expected,
                        alias.as_ref(),
                    ));
                    return Ok(result.finish());
                }
                if declaration.interface.declaration_details().modality()
                    == hir::NominalInheritanceModalityV1::Abstract
                {
                    return Ok(V::Known(Vec::new()));
                }
                let candidates = self
                    .dependencies
                    .as_ref()
                    .expect("selected dependency")
                    .constructor_candidates(owner)
                    .map_err(|_| U::DeclarationFailure);
                let candidates = match candidates {
                    Ok(candidates) => candidates,
                    Err(reason) => return Ok(V::Unavailable(reason)),
                };
                for candidate in candidates {
                    if is_cancelled() {
                        return Err(EditingCancelled);
                    }
                    let declaration = self
                        .dependencies
                        .as_ref()
                        .expect("selected dependency")
                        .callable_declaration(candidate.interface().declaration())
                        .map_err(|_| U::DeclarationFailure);
                    result.add(match declaration {
                        Ok(declaration) => self.editing_imported_constructor(
                            call,
                            &declaration,
                            expected,
                            alias.as_ref(),
                        ),
                        Err(reason) => Err(reason),
                    });
                }
            }
        }
        Ok(result.finish())
    }
}

fn header(call: &ast::RecoveredCall) -> NamedCallHeader<'_> {
    let callee = match &call.target {
        ast::RecoveredCallTarget::Named(name)
        | ast::RecoveredCallTarget::Member { name, .. }
        | ast::RecoveredCallTarget::Super { name, .. }
        | ast::RecoveredCallTarget::QualifiedSuper { name, .. } => name,
        ast::RecoveredCallTarget::Invoke(_) | ast::RecoveredCallTarget::Unavailable(_) => {
            unreachable!("nominal lookup starts from a source name")
        }
    };
    NamedCallHeader {
        callee,
        type_args: &call.type_args,
        span: call.span,
    }
}
