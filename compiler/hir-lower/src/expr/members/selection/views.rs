use super::*;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(in crate::expr) fn add_smart_cast_member_views(
        &mut self,
        local: &mut Vec<(crate::CallableCandidate, hir::Expr)>,
        imported: &mut Vec<(hir::ImportedCallableDeclaration, hir::Expr)>,
        views: &[hir::Expr],
        name: &ast::Ident,
        required: RequiredCallableModifiers,
    ) -> Result<(), Box<Lowerer>> {
        for receiver in views {
            let candidates = match required.operator {
                Some(operator) => self.methods_by_operator(receiver.ty, operator),
                None => self.methods_by_name(receiver.ty, &name.text),
            };
            for candidate in candidates {
                let signature = &self.signatures[&candidate.function];
                if !Self::matches_required_modifiers(signature.modifiers, required)
                    || (matches!(self.types[receiver.ty], Type::Interface(_))
                        && signature.type_params.len() != signature.owner_type_param_count)
                    || local.iter().any(|(selected, _)| {
                        selected.function == candidate.function
                            && selected.owner == candidate.owner
                            && selected.source == candidate.source
                    })
                {
                    continue;
                }
                local.push((candidate, receiver.clone()));
            }
            for candidate in self.imported_member_call_candidates(
                receiver.ty,
                name,
                required,
                MemberCallKind::Ordinary,
            )? {
                let owner = match candidate.interface().owner() {
                    hir::PublicDeclarationOwnerV1::Nominal(owner) => owner,
                    _ => unreachable!("member declarations have nominal owners"),
                };
                let application = self.imported_member_owner_type(receiver.ty, owner);
                let duplicate = imported.iter().any(|(selected, view)| {
                    selected.interface().declaration() == candidate.interface().declaration()
                        && self.imported_member_owner_type(view.ty, owner) == application
                });
                if !duplicate {
                    imported.push((candidate, receiver.clone()));
                }
            }
        }
        Ok(())
    }
}
