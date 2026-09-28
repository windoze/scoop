use super::*;

impl Lowerer {
    pub(super) fn imported_template_method_dispatch(
        &mut self,
        declaration: &hir::ImportedCallableDeclaration,
        nominal: &hir::ImportedNominalDeclaration,
    ) -> Result<(hir::MethodModifier, hir::ImportedMethodDispatch), String> {
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
            return Ok((modifier, hir::ImportedMethodDispatch::Interface(*slot)));
        }
        let family = callable
            .slot_relations()
            .values()
            .iter()
            .find_map(|slot| self.imported_virtual_family(*slot));
        let dispatch = match (modifier, family) {
            (hir::MethodModifier::Final, None) => hir::ImportedMethodDispatch::Direct,
            (hir::MethodModifier::Final, Some(family)) => {
                hir::ImportedMethodDispatch::FinalOverride(family)
            }
            (_, Some(family)) => hir::ImportedMethodDispatch::Virtual(family),
            (_, None) => {
                return Err("dependency virtual method has no declared dispatch family".into());
            }
        };
        Ok((modifier, dispatch))
    }
}
