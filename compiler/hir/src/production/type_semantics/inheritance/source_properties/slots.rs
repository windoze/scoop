use super::*;

pub(super) fn project(
    export: &ExportHir,
    getter: PropertyAccessorImplementation,
    setter: Option<PropertyAccessorImplementation>,
) -> Result<CanonicalProtectedSlotRefsV1, Error> {
    let mut slots = Vec::new();
    for implementation in std::iter::once(getter).chain(setter) {
        let function = match implementation {
            PropertyAccessorImplementation::Body(function)
            | PropertyAccessorImplementation::AbstractSlot(function) => function,
            PropertyAccessorImplementation::Storage => continue,
            PropertyAccessorImplementation::Constant => {
                return Err(invalid(
                    "const accessor cannot contribute an inheritance slot",
                ));
            }
        };
        let method = export.functions[function]
            .method
            .ok_or_else(|| invalid("inheritance property accessor body has no nominal owner"))?;
        let slot = match method.dispatch {
            MethodDispatch::Direct => continue,
            MethodDispatch::Virtual(family) | MethodDispatch::FinalOverride(family) => export
                .dispatch_slot_identities
                .get_virtual(family)
                .ok_or_else(|| invalid("property virtual family has no sealed slot"))?,
            MethodDispatch::Interface(member) => export
                .dispatch_slot_identities
                .get_interface(member)
                .ok_or_else(|| invalid("property interface member has no sealed slot"))?,
        };
        scoop_wire::allocation::try_reserve(&mut slots, 1, &WirePath::root()).map_err(resource)?;
        slots.push(slot.id());
    }

    CanonicalProtectedSlotRefsV1::try_new(slots).map_err(invalid)
}
