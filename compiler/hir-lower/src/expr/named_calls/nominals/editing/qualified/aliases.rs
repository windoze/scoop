use super::*;
use crate::aliases::AliasTemplate;
use crate::expr::constructors::AliasVariantConstructor;

impl Lowerer {
    pub(super) fn editing_alias_variant_candidates(
        &mut self,
        call: &ast::RecoveredCall,
        alias: &AliasTemplate,
        index: u32,
        name: &ast::Ident,
    ) -> CallCandidates {
        let Some(constructor) = self.alias_variant_constructor(alias, index, name, call.span)
        else {
            return V::Unavailable(U::DeclarationFailure);
        };
        let mut candidates = Candidates::default();
        candidates.add(match constructor {
            AliasVariantConstructor::Source(view) => self.editing_nominal_signature(
                call,
                view.signature,
                view.argument_mode,
                None,
                Some(alias),
            ),
            AliasVariantConstructor::Imported(declaration) => {
                self.editing_imported_constructor(call, &declaration, None, Some(alias))
            }
        });
        candidates.finish()
    }
}
