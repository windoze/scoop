//! FFI, callback, and source-location wire roles.

use super::*;

impl CoreFfiProtocolV1 {
    pub(in crate::production::core_protocol_surface) fn wire_entries(
        &self,
    ) -> [CoreProtocolEntryV1; FFI_PROTOCOL_COUNT] {
        [
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(self.ptr)),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(self.fun_ptr)),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(self.pinned_ptr)),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(self.gc_handle)),
            CoreProtocolEntryV1::Callable(self.ptr_to_ulong.clone()),
            CoreProtocolEntryV1::Callable(self.ptr_cast.clone()),
            CoreProtocolEntryV1::Callable(self.ptr_load.clone()),
            CoreProtocolEntryV1::Callable(self.ptr_load_offset.clone()),
            CoreProtocolEntryV1::Callable(self.ptr_store.clone()),
            CoreProtocolEntryV1::Callable(self.ptr_store_offset.clone()),
            CoreProtocolEntryV1::Callable(self.ptr_plus.clone()),
            CoreProtocolEntryV1::Callable(self.ptr_minus.clone()),
            CoreProtocolEntryV1::Callable(self.address_of.clone()),
            CoreProtocolEntryV1::Callable(self.size_of.clone()),
            CoreProtocolEntryV1::Callable(self.align_of.clone()),
            CoreProtocolEntryV1::Callable(self.gc_pin_raw.clone()),
            CoreProtocolEntryV1::Callable(self.gc_unpin_raw.clone()),
            CoreProtocolEntryV1::Callable(self.gc_get_handle_raw.clone()),
            CoreProtocolEntryV1::Callable(self.gc_release_handle_raw.clone()),
        ]
    }
}

impl DecodedCoreFfiProtocolV1 {
    pub(in crate::production::core_protocol_surface::wire) fn validate_against(
        self,
        foundation: &CanonicalHirFoundation,
    ) -> Result<CoreFfiProtocolV1, CoreCompilerProtocolSurfaceValidationError> {
        let [
            ptr,
            fun_ptr,
            pinned_ptr,
            gc_handle,
            ptr_to_ulong,
            ptr_cast,
            ptr_load,
            ptr_load_offset,
            ptr_store,
            ptr_store_offset,
            ptr_plus,
            ptr_minus,
            address_of,
            size_of,
            align_of,
            gc_pin_raw,
            gc_unpin_raw,
            gc_get_handle_raw,
            gc_release_handle_raw,
        ] = self.0.entries;
        Ok(CoreFfiProtocolV1 {
            ptr: read_generic_nominal(ptr, 0, foundation)?,
            fun_ptr: read_generic_nominal(fun_ptr, 1, foundation)?,
            pinned_ptr: read_generic_nominal(pinned_ptr, 2, foundation)?,
            gc_handle: read_generic_nominal(gc_handle, 3, foundation)?,
            ptr_to_ulong: read_callable(ptr_to_ulong, 4, foundation)?,
            ptr_cast: read_callable(ptr_cast, 5, foundation)?,
            ptr_load: read_callable(ptr_load, 6, foundation)?,
            ptr_load_offset: read_callable(ptr_load_offset, 7, foundation)?,
            ptr_store: read_callable(ptr_store, 8, foundation)?,
            ptr_store_offset: read_callable(ptr_store_offset, 9, foundation)?,
            ptr_plus: read_callable(ptr_plus, 10, foundation)?,
            ptr_minus: read_callable(ptr_minus, 11, foundation)?,
            address_of: read_callable(address_of, 12, foundation)?,
            size_of: read_callable(size_of, 13, foundation)?,
            align_of: read_callable(align_of, 14, foundation)?,
            gc_pin_raw: read_callable(gc_pin_raw, 15, foundation)?,
            gc_unpin_raw: read_callable(gc_unpin_raw, 16, foundation)?,
            gc_get_handle_raw: read_callable(gc_get_handle_raw, 17, foundation)?,
            gc_release_handle_raw: read_callable(gc_release_handle_raw, 18, foundation)?,
        })
    }
}

impl CoreForeignCallbackProtocolV1 {
    pub(in crate::production::core_protocol_surface) fn wire_entries(
        &self,
    ) -> [CoreProtocolEntryV1; FOREIGN_CALLBACK_PROTOCOL_COUNT] {
        [
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(self.callback)),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.mode)),
            CoreProtocolEntryV1::EnumVariant(self.reusable),
            CoreProtocolEntryV1::EnumVariant(self.one_shot),
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.state)),
            CoreProtocolEntryV1::EnumVariant(self.registered),
            CoreProtocolEntryV1::EnumVariant(self.active),
            CoreProtocolEntryV1::EnumVariant(self.completed),
            CoreProtocolEntryV1::EnumVariant(self.failed),
            CoreProtocolEntryV1::ExactType(self.failure_result),
            CoreProtocolEntryV1::Callable(self.register.clone()),
            CoreProtocolEntryV1::Callable(self.retain.clone()),
            CoreProtocolEntryV1::Callable(self.release.clone()),
            CoreProtocolEntryV1::Callable(self.query_state.clone()),
            CoreProtocolEntryV1::Callable(self.failure.clone()),
        ]
    }
}

impl DecodedCoreForeignCallbackProtocolV1 {
    pub(in crate::production::core_protocol_surface::wire) fn validate_against(
        self,
        foundation: &CanonicalHirFoundation,
    ) -> Result<CoreForeignCallbackProtocolV1, CoreCompilerProtocolSurfaceValidationError> {
        let [
            callback,
            mode,
            reusable,
            one_shot,
            state,
            registered,
            active,
            completed,
            failed,
            failure_result,
            register,
            retain,
            release,
            query_state,
            failure,
        ] = self.0.entries;
        Ok(CoreForeignCallbackProtocolV1 {
            callback: read_generic_nominal(callback, 0, foundation)?,
            mode: read_concrete_nominal(mode, 1, foundation)?,
            reusable: read_enum_variant(reusable, 2, foundation)?,
            one_shot: read_enum_variant(one_shot, 3, foundation)?,
            state: read_concrete_nominal(state, 4, foundation)?,
            registered: read_enum_variant(registered, 5, foundation)?,
            active: read_enum_variant(active, 6, foundation)?,
            completed: read_enum_variant(completed, 7, foundation)?,
            failed: read_enum_variant(failed, 8, foundation)?,
            failure_result: read_exact_type(failure_result, 9, foundation)?,
            register: read_callable(register, 10, foundation)?,
            retain: read_callable(retain, 11, foundation)?,
            release: read_callable(release, 12, foundation)?,
            query_state: read_callable(query_state, 13, foundation)?,
            failure: read_callable(failure, 14, foundation)?,
        })
    }
}

impl CoreSourceLocationProtocolV1 {
    pub(in crate::production::core_protocol_surface) fn wire_entries(
        &self,
    ) -> [CoreProtocolEntryV1; SOURCE_LOCATION_PROTOCOL_COUNT] {
        [
            CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(self.location)),
            CoreProtocolEntryV1::Callable(self.current.clone()),
        ]
    }
}

impl DecodedCoreSourceLocationProtocolV1 {
    pub(in crate::production::core_protocol_surface::wire) fn validate_against(
        self,
        foundation: &CanonicalHirFoundation,
    ) -> Result<CoreSourceLocationProtocolV1, CoreCompilerProtocolSurfaceValidationError> {
        let [location, current] = self.0.entries;
        Ok(CoreSourceLocationProtocolV1 {
            location: read_concrete_nominal(location, 0, foundation)?,
            current: read_callable(current, 1, foundation)?,
        })
    }
}
