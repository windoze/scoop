use super::*;

impl Lowerer {
    pub(crate) fn resolved_template_call(
        &self,
        application: hir::ImportedGenericCallableApplicationId,
        kind: hir::ImportedGenericCallKind,
        binding: Option<std::sync::Arc<hir::DirectImportedTargetBinding>>,
        args: Vec<hir::Expr>,
        receiver: hir::SourceCallReceiver<hir::TypeId>,
    ) -> hir::ExprKind {
        if matches!(
            self.imported_generic_applications[application].arguments,
            hir::ImportedCallableArguments::Method { .. }
        ) {
            hir::ExprKind::ImportedGenericCall {
                application,
                kind,
                binding,
                args,
                receiver,
            }
        } else {
            hir::ExprKind::Call {
                callee: hir::CallableTarget::Application(application),
                binding,
                args,
                receiver,
            }
        }
    }

    pub(super) fn imported_template_method_dispatch(
        &mut self,
        declaration: &hir::ImportedCallableDeclaration,
        nominal: &hir::ImportedNominalDeclaration,
    ) -> Result<(hir::MethodModifier, hir::DeclaredMethodDispatch), String> {
        let callable = declaration.interface();
        let modifier = match callable.modality() {
            hir::CallableModalityV1::Final => hir::MethodModifier::Final,
            hir::CallableModalityV1::Open | hir::CallableModalityV1::InterfaceDefault => {
                hir::MethodModifier::Open
            }
            hir::CallableModalityV1::Abstract => hir::MethodModifier::Abstract,
        };
        if matches!(
            nominal.interface.source_shape(),
            hir::NominalSourceShapeV1::Interface
        ) && let Some(slot) = callable.slot_relations().values().first()
        {
            return Ok((modifier, hir::DeclaredMethodDispatch::Interface(*slot)));
        }
        let family = callable
            .slot_relations()
            .values()
            .iter()
            .find_map(|slot| self.imported_virtual_family(*slot));
        let dispatch = match (modifier, family) {
            (hir::MethodModifier::Final, None) => hir::DeclaredMethodDispatch::Direct,
            (hir::MethodModifier::Final, Some(family)) => {
                hir::DeclaredMethodDispatch::FinalOverride(family)
            }
            (_, Some(family)) => hir::DeclaredMethodDispatch::Virtual(family),
            (_, None) => {
                return Err("dependency virtual method has no declared dispatch family".into());
            }
        };
        Ok((modifier, dispatch))
    }
}
