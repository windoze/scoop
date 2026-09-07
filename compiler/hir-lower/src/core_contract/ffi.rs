use super::*;

impl Lowerer {
    pub(crate) fn validate_ffi_core(&mut self, files: &[ast::SourceFile]) -> Option<hir::FfiCore> {
        let ptr = self.ffi_ptr;
        let fun_ptr = self.ffi_fun_ptr;
        let pinned_ptr = self.ffi_pinned_ptr;
        let gc_handle = self.ffi_gc_handle;
        if let Some(id) = ptr {
            self.validate_ptr_struct(id);
        }
        if let Some(id) = fun_ptr {
            self.validate_fun_ptr_struct(id);
        }
        if let Some(id) = pinned_ptr {
            self.validate_ffi_handle_struct(id, "PinnedPtr");
        }
        if let Some(id) = gc_handle {
            self.validate_ffi_handle_struct(id, "GcHandle");
        }

        let ptr_to_ulong = self.require_intrinsic(
            hir::IntrinsicFunctionKind::Pointer(hir::PointerIntrinsic::ToULong),
            files,
        );
        let ptr_cast = self.require_intrinsic(
            hir::IntrinsicFunctionKind::Pointer(hir::PointerIntrinsic::Cast),
            files,
        );
        let ptr_load = self.require_intrinsic(
            hir::IntrinsicFunctionKind::Pointer(hir::PointerIntrinsic::Load),
            files,
        );
        let ptr_load_offset = self.require_intrinsic(
            hir::IntrinsicFunctionKind::Pointer(hir::PointerIntrinsic::LoadOffset),
            files,
        );
        let ptr_store = self.require_intrinsic(
            hir::IntrinsicFunctionKind::Pointer(hir::PointerIntrinsic::Store),
            files,
        );
        let ptr_store_offset = self.require_intrinsic(
            hir::IntrinsicFunctionKind::Pointer(hir::PointerIntrinsic::StoreOffset),
            files,
        );
        let ptr_plus = self.require_intrinsic(
            hir::IntrinsicFunctionKind::Pointer(hir::PointerIntrinsic::Plus),
            files,
        );
        let ptr_minus = self.require_intrinsic(
            hir::IntrinsicFunctionKind::Pointer(hir::PointerIntrinsic::Minus),
            files,
        );
        let address_of = self.require_intrinsic(
            hir::IntrinsicFunctionKind::Pointer(hir::PointerIntrinsic::AddressOf),
            files,
        );
        let size_of = self.require_intrinsic(
            hir::IntrinsicFunctionKind::Pointer(hir::PointerIntrinsic::SizeOf),
            files,
        );
        let align_of = self.require_intrinsic(
            hir::IntrinsicFunctionKind::Pointer(hir::PointerIntrinsic::AlignOf),
            files,
        );
        let gc_pin_raw = self.require_intrinsic(hir::IntrinsicFunctionKind::GcPinRaw, files);
        let gc_unpin_raw = self.require_intrinsic(hir::IntrinsicFunctionKind::GcUnpinRaw, files);
        let gc_get_handle_raw =
            self.require_intrinsic(hir::IntrinsicFunctionKind::GcGetHandleRaw, files);
        let gc_release_handle_raw =
            self.require_intrinsic(hir::IntrinsicFunctionKind::GcReleaseHandleRaw, files);

        for (id, kind) in [
            (gc_pin_raw, GcIntrinsic::Pin),
            (gc_unpin_raw, GcIntrinsic::Unpin),
            (gc_get_handle_raw, GcIntrinsic::GetHandle),
            (gc_release_handle_raw, GcIntrinsic::ReleaseHandle),
        ] {
            if let Some(id) = id {
                self.validate_gc_intrinsic(id, kind);
            }
        }

        if let Some(ptr) = ptr {
            for (id, kind) in [
                (ptr_to_ulong, hir::PointerIntrinsic::ToULong),
                (ptr_cast, hir::PointerIntrinsic::Cast),
                (ptr_load, hir::PointerIntrinsic::Load),
                (ptr_load_offset, hir::PointerIntrinsic::LoadOffset),
                (ptr_store, hir::PointerIntrinsic::Store),
                (ptr_store_offset, hir::PointerIntrinsic::StoreOffset),
                (ptr_plus, hir::PointerIntrinsic::Plus),
                (ptr_minus, hir::PointerIntrinsic::Minus),
            ] {
                if let Some(id) = id {
                    self.validate_ptr_method_intrinsic(id, ptr, kind);
                }
            }
        }
        if let Some(id) = address_of {
            self.validate_pointer_top_level_intrinsic(id, hir::PointerIntrinsic::AddressOf);
        }
        if let Some(id) = size_of {
            self.validate_pointer_top_level_intrinsic(id, hir::PointerIntrinsic::SizeOf);
        }
        if let Some(id) = align_of {
            self.validate_pointer_top_level_intrinsic(id, hir::PointerIntrinsic::AlignOf);
        }

        Some(hir::FfiCore {
            ptr: ptr?,
            fun_ptr: fun_ptr?,
            pinned_ptr: pinned_ptr?,
            gc_handle: gc_handle?,
            ptr_to_ulong: ptr_to_ulong?,
            ptr_cast: ptr_cast?,
            ptr_load: ptr_load?,
            ptr_load_offset: ptr_load_offset?,
            ptr_store: ptr_store?,
            ptr_store_offset: ptr_store_offset?,
            ptr_plus: ptr_plus?,
            ptr_minus: ptr_minus?,
            address_of: address_of?,
            size_of: size_of?,
            align_of: align_of?,
            gc_pin_raw: gc_pin_raw?,
            gc_unpin_raw: gc_unpin_raw?,
            gc_get_handle_raw: gc_get_handle_raw?,
            gc_release_handle_raw: gc_release_handle_raw?,
        })
    }
}
