use super::*;

mod native;

impl Writer<'_, '_> {
    pub(super) fn descriptor(&mut self, descriptor: TypeDescriptorRef) -> Result {
        // A local materialization and an external declaration name the same
        // exact descriptor; placement and producer-table membership are not
        // part of a callable's semantic reference.
        let exact = match descriptor {
            TypeDescriptorRef::Local(id) => {
                self.module.meta.type_descriptors[id].identity.exact_type()
            }
            TypeDescriptorRef::External(id) => {
                self.module.meta.external_type_descriptors[id].target()
            }
        };
        self.id(&exact)
    }

    pub(super) fn local_function(&mut self, id: LocalFunctionId) -> Result {
        self.id(&self.module.functions[id.into_u32() as usize]
            .callable_body
            .id())
    }

    pub(super) fn function_address(&mut self, target: FunctionAddressTarget) -> Result {
        match target {
            FunctionAddressTarget::Local(reference) => {
                record!(self, 1; self.local_function(reference.declaration()))
            }
            FunctionAddressTarget::CallbackTrampoline(id) => {
                record!(self, 2; self.id(&self.module.callback_bridges[id].trampoline.entry().unit()))
            }
        }
    }

    pub(super) fn global(&mut self, id: GlobalId) -> Result {
        match &self.module.globals[id].init {
            GlobalInit::StringConst { identity, value } => {
                record!(self, 1; self.id(&identity.identity_record().id()), self.text(value))
            }
            GlobalInit::CString { identity, value } => {
                record!(self, 2; self.id(&identity.atom_record().id()), self.text(value))
            }
            GlobalInit::Storage { identity, ty, .. } => {
                record!(self, 3; self.id(&identity.symbol_request()), self.ty(ty))
            }
            GlobalInit::ImportedStorage { definition, ty } => {
                record!(self, 3; self.id(&definition.expected_symbol()), self.ty(ty))
            }
        }
    }

    pub(super) fn array_type(&mut self, id: ArrayTypeId) -> Result {
        let array = &self.module.meta.arrays[id];
        let kind = match array.kind {
            ArrayKind::Immutable => 1,
            ArrayKind::Mutable => 2,
        };
        let shape = array.layout.instance();
        record!(self, kind;
            self.id(&array.identity.layout_record().id()), self.id(&array.element_exact), self.ty(&array.element),
            self.descriptor(array.type_descriptor), self.u(shape.minimum_size()), self.u(shape.instance_alignment()),
            self.u(shape.inline_offset()), self.scan(shape.object_scan()), self.array_storage(array.layout.storage()), self.u(array.layout.maximum_count()))
    }

    fn array_storage(&mut self, storage: &ArrayElementStorageV1) -> Result {
        match storage.kind() {
            ArrayElementStorageKindV1::ZeroSized { alignment } => {
                record!(self, 1; self.u(alignment.get()))
            }
            ArrayElementStorageKindV1::Inline {
                stride,
                alignment,
                scan,
            } => {
                record!(self, 2; self.u(stride.get()), self.u(alignment.get()), self.scan(scan.as_ref_scan()))
            }
        }
    }

    pub(super) fn callback_family(&mut self, id: ForeignCallbackFamilyId) -> Result {
        let family = self.module.foreign_callback_families[id];
        record!(self, 1; self.id(&self.module.structs[family.callback].exact_type),
            self.variant(family.modes.reusable()), self.variant(family.modes.one_shot()),
            self.variant(family.states.registered()), self.variant(family.states.active()),
            self.variant(family.states.completed()), self.variant(family.states.failed()),
            self.field(family.failure_result.some_payload()), self.variant(family.failure_result.none()))
    }

    pub(super) fn callback_bridge(&mut self, id: ForeignCallbackBridgeId) -> Result {
        let bridge = &self.module.foreign_callback_bridges[id];
        record!(self, 1; self.id(&bridge.application), self.callback_family(bridge.family),
            self.local_function(bridge.adapter.declaration()), self.id(&bridge.trampoline.entry().unit()),
            self.id(&bridge.trampoline.signature()), self.u(u64::from(bridge.context_index)), self.variant(bridge.mode))
    }
}
