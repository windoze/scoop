use super::*;

pub(super) fn project(
    local: &hir::concrete::Module,
    function: &hir::concrete::Function,
    declaration: Declaration,
    modality: hir::CallableModalityV1,
) -> Result<mir::MirCallableLoweringRoleV1, Error> {
    if modality == hir::CallableModalityV1::Abstract {
        let method = function
            .receiver
            .method()
            .ok_or(Error::InvalidSourceRole(declaration))?;
        if method.modifier != hir::MethodModifier::Abstract {
            return Err(Error::InvalidSourceRole(declaration));
        }
        let slot = match method.dispatch {
            hir::concrete::MethodDispatch::Virtual(family)
            | hir::concrete::MethodDispatch::FinalOverride(family) => {
                local.dispatch_slot_identities.virtual_slot(family).id()
            }
            hir::concrete::MethodDispatch::Interface { interface, slot } => local
                .dispatch_slot_identities
                .interface_slot(interface, slot)
                .id(),
            _ => return Err(Error::InvalidSourceRole(declaration)),
        };
        return Ok(mir::MirCallableLoweringRoleV1::PureVirtualTrap { slot });
    }
    if crate::lowering_support::is_abstract_bodiless(function) {
        return Err(Error::InvalidSourceRole(declaration));
    }
    Ok(match declaration {
        Declaration::Function(_) => mir::MirCallableLoweringRoleV1::Ordinary,
        Declaration::PropertyAccessor(_) => mir::MirCallableLoweringRoleV1::Accessor,
    })
}
