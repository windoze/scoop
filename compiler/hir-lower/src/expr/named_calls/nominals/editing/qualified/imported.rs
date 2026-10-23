use super::*;
use crate::expr::imported_namespaces::{ImportedNominalQualifier, ImportedQualifiedCall};

impl Lowerer {
    pub(super) fn editing_imported_qualified_candidates(
        &mut self,
        call: &ast::RecoveredCall,
        owner: ImportedNominalQualifier,
        name: &ast::Ident,
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<CallCandidates, EditingCancelled> {
        let target = match self.resolve_imported_qualified_call(owner, name) {
            Ok(target) => target,
            Err(()) => return Ok(V::Unavailable(U::DeclarationFailure)),
        };
        match target {
            ImportedQualifiedCall::Variant { owner, index } => {
                let mut candidates = Candidates::default();
                let candidate = self.imported_variant_constructor(owner, index as u32, name.span);
                candidates.add(match candidate {
                    Some(declaration) => {
                        self.editing_imported_constructor(call, &declaration, Some(owner), None)
                    }
                    None => Err(U::DeclarationFailure),
                });
                Ok(candidates.finish())
            }
            ImportedQualifiedCall::Constructor(owner) => {
                let prepared = self.imported_nominal_plans(
                    hir::SourceNominalId::Concrete(owner),
                    header(call),
                    None,
                    false,
                );
                self.editing_prepared_nominal_candidates(call, prepared, is_cancelled)
            }
            ImportedQualifiedCall::Singleton(receiver) => self.editing_member_candidates(
                call,
                receiver,
                &name.text,
                false,
                false,
                is_cancelled,
            ),
            ImportedQualifiedCall::Bindings(bindings) => self
                .editing_named_function_candidates(call, &bindings, is_cancelled)
                .map(Candidates::finish),
        }
    }
}
