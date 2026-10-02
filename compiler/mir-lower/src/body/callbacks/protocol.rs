use super::*;

impl BodyLowerer<'_> {
    pub(super) fn callback_protocol(&self) -> hir::ForeignCallbackCore {
        let hir::ConcreteCoreProtocols::Imported(core) = self.core_protocols else {
            return crate::defined_protocols(self.core_protocols).foreign_callbacks;
        };
        let callback = core.foreign_callbacks();
        let enumeration = |owner| {
            self.module
                .enums
                .iter()
                .find_map(|(id, value)| {
                    (value.origin.concrete_type_id() == Some(owner)).then_some(id)
                })
                .expect("callback protocol types were materialized by HIR")
        };
        let variant = |enumeration: hir::EnumId,
                       identity: scoop_identity::PersistentEnumVariantId| {
            let index = self.module.enums[enumeration]
                .variants
                .iter()
                .position(|variant| variant.identity == identity)
                .expect("the imported callback protocol names an actual variant");
            hir::EnumVariantRef::checked(
                &self.module.enums,
                enumeration,
                hir::VariantId::from_raw(index as u32),
            )
            .expect("the variant belongs to its materialized enum")
        };
        let mode = enumeration(callback.mode().persistent());
        let state = enumeration(callback.state().persistent());
        let modes = hir::ForeignCallbackModes::checked(
            &self.module.enums,
            variant(mode, callback.reusable().persistent()),
            variant(mode, callback.one_shot().persistent()),
        )
        .expect("materialized callback modes retain the imported core shape");
        let states = hir::ForeignCallbackStates::checked(
            &self.module.enums,
            variant(state, callback.registered().persistent()),
            variant(state, callback.active().persistent()),
            variant(state, callback.completed().persistent()),
            variant(state, callback.failed().persistent()),
        )
        .expect("materialized callback states retain the imported core shape");
        let failure = self
            .module
            .enums
            .iter()
            .find_map(|(id, value)| {
                (self.module.exact_type_identities[value.canonical_type].id()
                    == callback.failure_result().persistent())
                .then_some(id)
            })
            .expect("the exact callback failure type was materialized by HIR");
        let some = variant(failure, core.option().some().persistent());
        let field = self.module.enums[failure].variants[some.variant().into_raw() as usize]
            .fields
            .iter()
            .position(|field| field.identity == core.option().some_payload().persistent())
            .expect("the imported Option payload retains its field identity");
        let some = hir::EnumVariantFieldRef::checked(&self.module.enums, some, field as u32)
            .expect("the payload belongs to its materialized variant");
        let option = hir::OptionCore::checked(
            &self.module.enums,
            some,
            variant(failure, core.option().none().persistent()),
        )
        .expect("the failure Option shape is preserved");
        let throwable = self
            .module
            .classes
            .iter()
            .find_map(|(id, class)| {
                (class.origin.concrete_type_id()
                    == Some(core.exceptions().throwable().persistent()))
                .then_some(id)
            })
            .expect("the callback Throwable type was materialized by HIR");
        let failure_result = hir::ForeignCallbackFailureResult::checked(
            &self.module.enums,
            &self.module.types,
            option,
            throwable,
        )
        .expect("the callback failure payload is the exact Throwable type");
        hir::ForeignCallbackCore {
            modes,
            states,
            failure_result,
        }
    }
}
