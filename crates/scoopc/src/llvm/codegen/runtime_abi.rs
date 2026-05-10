//! LLVM codegen：runtime ABI glue（符号声明 / 调用约定）。
//!
//! 目标：把 runtime 的 C ABI 声明集中管理，避免在 expr/stmt codegen 中散落 `declare_*`。

use inkwell::AddressSpace;
use inkwell::types::BasicMetadataTypeEnum;
use inkwell::types::StructType;
use inkwell::values::FunctionValue;
use inkwell::values::GlobalValue;

use super::MainCodegen;
use super::runtime_symbols;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(super) fn llvm_explicit_root_frame_desc_type(&self) -> StructType<'ctx> {
        const TY_NAME: &str = "scoop.runtime.ScoopRootFrameDesc";
        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        ty.set_body(
            &[
                self.context.i32_type().into(),
                self.llvm_ptr_type(AddressSpace::default()).into(),
            ],
            false,
        );
        ty
    }

    pub(super) fn llvm_explicit_root_frame_header_type(&self) -> StructType<'ctx> {
        const TY_NAME: &str = "scoop.runtime.ScoopRootFrameHeader";
        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        let ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        ty.set_body(&[ptr_ty.into(), ptr_ty.into()], false);
        ty
    }

    pub(super) fn declare_runtime_explicit_root_frame_top_tls(&self) -> GlobalValue<'ctx> {
        const NAME: &str = "__scoop_explicit_root_frame_top";
        if let Some(existing) = self.module.get_global(NAME) {
            return existing;
        }

        let gv = self
            .module
            .add_global(self.llvm_ptr_type(AddressSpace::default()), None, NAME);
        gv.set_linkage(inkwell::module::Linkage::External);
        gv.set_thread_local(true);
        gv
    }

    pub(super) fn llvm_value_transport_struct_type(&self) -> StructType<'ctx> {
        const TY_NAME: &str = "scoop.runtime.ScoopValueTransport";
        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        ty.set_body(
            &[
                self.context.i64_type().into(),
                self.llvm_gc_i8_ptr_type().into(),
            ],
            false,
        );
        ty
    }

    pub(super) fn llvm_effect_signal_struct_type(&self) -> StructType<'ctx> {
        const TY_NAME: &str = "scoop.runtime.ScoopEffectSignal";
        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        ty.set_body(
            &[
                self.context.i32_type().into(),
                self.context.i32_type().into(),
                self.llvm_value_transport_struct_type().into(),
                self.llvm_gc_i8_ptr_type().into(),
            ],
            false,
        );
        ty
    }

    pub(super) fn llvm_composite_transport_descriptor_type(&self) -> StructType<'ctx> {
        const TY_NAME: &str = "scoop.runtime.ScoopCompositeTransportDescriptor";
        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let default_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        ty.set_body(
            &[
                i32_ty.into(),         // abi_version
                i32_ty.into(),         // storage_kind
                i64_ty.into(),         // size_bytes
                i64_ty.into(),         // align_bytes
                default_ptr_ty.into(), // gc_slot_offsets
                i32_ty.into(),         // gc_slot_count
                i32_ty.into(),         // _reserved_u32
                default_ptr_ty.into(), // trace_fn
                default_ptr_ty.into(), // copy_fn
                default_ptr_ty.into(), // drop_fn
                default_ptr_ty.into(), // type_desc
            ],
            false,
        );
        ty
    }

    pub(super) fn llvm_effect_outcome_struct_type(&self) -> StructType<'ctx> {
        const TY_NAME: &str = "scoop.runtime.ScoopEffectOutcome";
        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        ty.set_body(
            &[
                self.context.i32_type().into(),
                self.context.i32_type().into(),
                self.llvm_value_transport_struct_type().into(),
                self.llvm_effect_signal_struct_type().into(),
            ],
            false,
        );
        ty
    }

    pub(super) fn declare_runtime_print_like(&self, name: &str) -> FunctionValue<'ctx> {
        if let Some(existing) = self.module.get_function(name) {
            return existing;
        }

        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] =
            [self.llvm_scoop_string_ptr_type().into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(name, fn_ty, None)
    }

    pub(super) fn declare_runtime_format_int(&self, name: &str) -> FunctionValue<'ctx> {
        if let Some(existing) = self.module.get_function(name) {
            return existing;
        }

        // `uint64_t scoop_format_{i64,u64}(int64_t value, uint8_t* out, uint64_t cap)`
        //
        // 说明：
        // - 该函数用于 f-string 插值 `{Int}` 的最小 formatting（TODO T0823）；
        // - 由 runtime 实现，避免在 LLVM IR 中直接引入 varargs `snprintf` 调用。
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.context.ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 3] =
            [i64_ty.into(), i8_ptr_ty.into(), i64_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(name, fn_ty, None)
    }

    pub(super) fn declare_runtime_trim_indent(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_STRING_TRIM_INDENT;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `const ScoopString* scoop_string_trim_indent(const ScoopString* value)`
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [str_ptr_ty.into()];
        let fn_ty = str_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    // T1811: String P0 methods.

    /// `int64_t scoop_string_length(const ScoopString* s)`
    pub(super) fn declare_runtime_string_length(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_STRING_LENGTH;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }
        let i64_ty = self.context.i64_type();
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let fn_ty = i64_ty.fn_type(&[str_ptr_ty.into()], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    /// T0107: `int64_t scoop_string_equals(const ScoopString* a, const ScoopString* b)`
    pub(super) fn declare_runtime_string_equals(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_STRING_EQUALS;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }
        let i64_ty = self.context.i64_type();
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let fn_ty = i64_ty.fn_type(&[str_ptr_ty.into(), str_ptr_ty.into()], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    /// T1817: `int64_t scoop_string_hash(const ScoopString* s)`
    pub(super) fn declare_runtime_string_hash(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_STRING_HASH;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }
        let i64_ty = self.context.i64_type();
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let fn_ty = i64_ty.fn_type(&[str_ptr_ty.into()], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_composite_trace(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_COMPOSITE_TRACE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }
        let i64_ty = self.context.i64_type();
        let ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let fn_ty = i64_ty.fn_type(
            &[ptr_ty.into(), ptr_ty.into(), ptr_ty.into(), ptr_ty.into()],
            false,
        );
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_composite_copy(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_COMPOSITE_COPY;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }
        let ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let fn_ty = self
            .context
            .void_type()
            .fn_type(&[ptr_ty.into(), ptr_ty.into(), ptr_ty.into()], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_composite_drop(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_COMPOSITE_DROP;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }
        let ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let fn_ty = self
            .context
            .void_type()
            .fn_type(&[ptr_ty.into(), ptr_ty.into()], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    // `const ScoopString* scoop_string_substring(const ScoopString* s, int64_t start, int64_t end)`
    // T0122/T0143: declare_runtime_string_substring 已移除（迁移到 sysroot/string.scoop）

    /// `const ScoopString* scoop_string_unsafe_slice_bytes(const ScoopString* source, int64_t offset, int64_t len)`
    pub(super) fn declare_runtime_string_unsafe_slice_bytes(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_STRING_UNSAFE_SLICE_BYTES;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }
        let i64_ty = self.context.i64_type();
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let fn_ty = str_ptr_ty.fn_type(&[str_ptr_ty.into(), i64_ty.into(), i64_ty.into()], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    /// `const ScoopString* scoop_string_concat(const ScoopString* a, const ScoopString* b)`
    pub(super) fn declare_runtime_string_concat(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_STRING_CONCAT;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let fn_ty = str_ptr_ty.fn_type(&[str_ptr_ty.into(), str_ptr_ty.into()], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    // T0122: starts_with/ends_with/index_of/contains/split/trim/trim_start/trim_end
    // 已移除（迁移到 sysroot/string.scoop）

    // T0115: String 補齐 — isEmpty/replace/charAt/repeat/compareTo

    /// `int64_t scoop_string_is_empty(const ScoopString* s)`
    pub(super) fn declare_runtime_string_is_empty(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_STRING_IS_EMPTY;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }
        let i64_ty = self.context.i64_type();
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let fn_ty = i64_ty.fn_type(&[str_ptr_ty.into()], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    /// `const ScoopString* scoop_string_replace(const ScoopString* s, const ScoopString* old, const ScoopString* new_str)`
    pub(super) fn declare_runtime_string_replace(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_STRING_REPLACE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let fn_ty = str_ptr_ty.fn_type(
            &[str_ptr_ty.into(), str_ptr_ty.into(), str_ptr_ty.into()],
            false,
        );
        self.module.add_function(NAME, fn_ty, None)
    }

    /// `int64_t scoop_string_char_at(const ScoopString* s, int64_t index)`
    pub(super) fn declare_runtime_string_char_at(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_STRING_CHAR_AT;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }
        let i64_ty = self.context.i64_type();
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let fn_ty = i64_ty.fn_type(&[str_ptr_ty.into(), i64_ty.into()], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    /// `const ScoopString* scoop_string_repeat(const ScoopString* s, int64_t n)`
    pub(super) fn declare_runtime_string_repeat(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_STRING_REPEAT;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }
        let i64_ty = self.context.i64_type();
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let fn_ty = str_ptr_ty.fn_type(&[str_ptr_ty.into(), i64_ty.into()], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    /// `int64_t scoop_string_compare_to(const ScoopString* a, const ScoopString* b)`
    pub(super) fn declare_runtime_string_compare_to(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_STRING_COMPARE_TO;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }
        let i64_ty = self.context.i64_type();
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let fn_ty = i64_ty.fn_type(&[str_ptr_ty.into(), str_ptr_ty.into()], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    // T0114: Bool→String conversion.

    /// `const ScoopString* scoop_bool_to_string(int64_t value)`
    pub(super) fn declare_runtime_bool_to_string(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_BOOL_TO_STRING;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }
        let i64_ty = self.context.i64_type();
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let fn_ty = str_ptr_ty.fn_type(&[i64_ty.into()], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    /// `const ScoopString* scoop_char_to_string(int32_t codepoint)`
    pub(super) fn declare_runtime_char_to_string(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_CHAR_TO_STRING;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }
        let i32_ty = self.context.i32_type();
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let fn_ty = str_ptr_ty.fn_type(&[i32_ty.into()], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    /// `const ScoopString* scoop_float64_to_string(double value)`
    pub(super) fn declare_runtime_float64_to_string(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_FLOAT64_TO_STRING;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let fn_ty = str_ptr_ty.fn_type(&[self.context.f64_type().into()], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    /// `const ScoopString* scoop_float32_to_string(float value)`
    pub(super) fn declare_runtime_float32_to_string(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_FLOAT32_TO_STRING;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let fn_ty = str_ptr_ty.fn_type(&[self.context.f32_type().into()], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    // T1812: Int↔String conversion methods.

    /// `const ScoopString* scoop_int_to_string(int64_t value)`
    pub(super) fn declare_runtime_int_to_string(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_INT_TO_STRING;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }
        let i64_ty = self.context.i64_type();
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let fn_ty = str_ptr_ty.fn_type(&[i64_ty.into()], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    /// `int64_t scoop_string_to_int(const ScoopString* s)`
    pub(super) fn declare_runtime_string_to_int(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_STRING_TO_INT;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }
        let i64_ty = self.context.i64_type();
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let fn_ty = i64_ty.fn_type(&[str_ptr_ty.into()], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    /// `int64_t scoop_float64_to_int(double value)`
    pub(super) fn declare_runtime_float64_to_int(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_FLOAT64_TO_INT;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }
        let fn_ty = self
            .context
            .i64_type()
            .fn_type(&[self.context.f64_type().into()], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    /// `int64_t scoop_float32_to_int(float value)`
    pub(super) fn declare_runtime_float32_to_int(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_FLOAT32_TO_INT;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }
        let fn_ty = self
            .context
            .i64_type()
            .fn_type(&[self.context.f32_type().into()], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_panic(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_PANIC;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_panic(const ScoopString* message)`
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [str_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_error_fatal(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_RUNTIME_ERROR_FATAL;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_runtime_error_fatal(void* runtime_error)`
        let payload_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [payload_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    // --- std v3：sync（T1319b） ---

    pub(super) fn declare_runtime_sync_mutex_create(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_SYNC_MUTEX_CREATE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_sync_mutex_create(void)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let fn_ty = gc_i8_ptr_ty.fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_sync_mutex_lock(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_SYNC_MUTEX_LOCK;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_sync_mutex_lock(void* mutex_obj)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [gc_i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_sync_mutex_unlock(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_SYNC_MUTEX_UNLOCK;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_sync_mutex_unlock(void* mutex_obj)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [gc_i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_sync_mutex_destroy(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_SYNC_MUTEX_DESTROY;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_sync_mutex_destroy(void* mutex_obj)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [gc_i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_sync_condvar_create(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_SYNC_CONDVAR_CREATE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_sync_condvar_create(void)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let fn_ty = gc_i8_ptr_ty.fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_sync_condvar_wait(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_SYNC_CONDVAR_WAIT;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_sync_condvar_wait(void* condvar_obj, void* mutex_obj)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] =
            [gc_i8_ptr_ty.into(), gc_i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_sync_condvar_notify_one(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_SYNC_CONDVAR_NOTIFY_ONE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_sync_condvar_notify_one(void* condvar_obj)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [gc_i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_sync_condvar_notify_all(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_SYNC_CONDVAR_NOTIFY_ALL;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_sync_condvar_notify_all(void* condvar_obj)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [gc_i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_sync_condvar_destroy(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_SYNC_CONDVAR_DESTROY;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_sync_condvar_destroy(void* condvar_obj)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [gc_i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_sync_once_create(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_SYNC_ONCE_CREATE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_sync_once_create(void)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let fn_ty = gc_i8_ptr_ty.fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_sync_once_is_done(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_SYNC_ONCE_IS_DONE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `bool scoop_sync_once_is_done(void* once_obj)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i1_ty = self.context.bool_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [gc_i8_ptr_ty.into()];
        let fn_ty = i1_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_sync_once_run(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_SYNC_ONCE_RUN;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_sync_once_run(void* once_obj, void* env_ptr, void (*fn)(void* env_ptr))`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let env_ptr_ty = self.llvm_i8_ptr_type();
        let init_fn_ptr_ty = self.context.ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 3] = [
            gc_i8_ptr_ty.into(),
            env_ptr_ty.into(),
            init_fn_ptr_ty.into(),
        ];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    // --- std v3：thread（T1319c） ---

    pub(super) fn declare_runtime_thread_spawn(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_THREAD_SPAWN;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_thread_spawn(void* env_ptr, void (*fn)(void* env_ptr))`
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let start_fn_ptr_ty = self.context.ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] =
            [i8_ptr_ty.into(), start_fn_ptr_ty.into()];
        let fn_ty = gc_i8_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_thread_spawn_join_compat_resume_u64(
        &self,
    ) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_THREAD_SPAWN_JOIN_COMPAT_RESUME_U64;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [gc_i8_ptr_ty.into(), i64_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_thread_spawn_join_resume_u64(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_THREAD_SPAWN_JOIN_RESUME_U64;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let thunk_ptr_ty = self.context.ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 3] =
            [gc_i8_ptr_ty.into(), i64_ty.into(), thunk_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_thread_spawn_join_resume_transport(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_THREAD_SPAWN_JOIN_RESUME_TRANSPORT;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let default_ptr_ty = self.context.ptr_type(AddressSpace::default());
        let thunk_ptr_ty = self.context.ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 6] = [
            gc_i8_ptr_ty.into(),
            i64_ty.into(),
            gc_i8_ptr_ty.into(),
            default_ptr_ty.into(),
            default_ptr_ty.into(),
            thunk_ptr_ty.into(),
        ];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_thread_join(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_THREAD_JOIN;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_thread_join(void* thread_obj)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [gc_i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_thread_yield(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_THREAD_YIELD;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_thread_yield(void)`
        let fn_ty = self.context.void_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_thread_sleep_millis(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_THREAD_SLEEP_MILLIS;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_thread_sleep_millis(int64_t ms)`
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_thread_current_id(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_THREAD_CURRENT_ID;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `int64_t scoop_thread_current_id(void)`
        let i64_ty = self.context.i64_type();
        let fn_ty = i64_ty.fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_array_builder_new(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_ARRAY_BUILDER_NEW;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_array_builder_new(void)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let fn_ty = gc_i8_ptr_ty.fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_array_builder_push_u64(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_ARRAY_BUILDER_PUSH_U64;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_array_builder_push_u64(void* builder, uint64_t value)`
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i8_ptr_ty.into(), i64_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_array_builder_push_ref(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_ARRAY_BUILDER_PUSH_REF;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_array_builder_push_ref(void* builder, void* value)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] =
            [gc_i8_ptr_ty.into(), gc_i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_array_builder_push_composite(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_ARRAY_BUILDER_PUSH_COMPOSITE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_array_builder_push_composite(void* builder, const desc*, const void* value)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 3] =
            [gc_i8_ptr_ty.into(), ptr_ty.into(), ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_array_builder_build_array(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_ARRAY_BUILDER_BUILD_ARRAY;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_array_builder_build_array(void* builder)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [gc_i8_ptr_ty.into()];
        let fn_ty = gc_i8_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_array_builder_build_array_composite(
        &self,
    ) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_ARRAY_BUILDER_BUILD_ARRAY_COMPOSITE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_array_builder_build_array_composite(void* builder, const desc*)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [gc_i8_ptr_ty.into(), ptr_ty.into()];
        let fn_ty = gc_i8_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_array_builder_build_mutable_array(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_ARRAY_BUILDER_BUILD_MUTABLE_ARRAY;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_array_builder_build_mutable_array(void* builder)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [gc_i8_ptr_ty.into()];
        let fn_ty = gc_i8_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_array_builder_build_mutable_array_composite(
        &self,
    ) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_ARRAY_BUILDER_BUILD_MUTABLE_ARRAY_COMPOSITE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_array_builder_build_mutable_array_composite(void* builder, const desc*)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [gc_i8_ptr_ty.into(), ptr_ty.into()];
        let fn_ty = gc_i8_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_array_len(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_ARRAY_LEN;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_array_len(void* array_obj)`
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i8_ptr_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_array_get_u64(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_ARRAY_GET_U64;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_array_get_u64(void* array_obj, int64_t index)`
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i8_ptr_ty.into(), i64_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_array_get_ref(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_ARRAY_GET_REF;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_array_get_ref(void* array_obj, int64_t index)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [gc_i8_ptr_ty.into(), i64_ty.into()];
        let fn_ty = gc_i8_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_array_get_composite(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_ARRAY_GET_COMPOSITE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_array_get_composite(void* array_obj, int64_t index, const desc*, void* out)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 4] = [
            gc_i8_ptr_ty.into(),
            i64_ty.into(),
            ptr_ty.into(),
            ptr_ty.into(),
        ];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_array_set_u64(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_ARRAY_SET_U64;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_array_set_u64(void* array_obj, int64_t index, uint64_t value)`
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 3] =
            [i8_ptr_ty.into(), i64_ty.into(), i64_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_array_set_ref(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_ARRAY_SET_REF;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_array_set_ref(void* array_obj, int64_t index, void* value)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 3] =
            [gc_i8_ptr_ty.into(), i64_ty.into(), gc_i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_array_set_composite(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_ARRAY_SET_COMPOSITE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_array_set_composite(void* array_obj, int64_t index, const desc*, const void* value)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 4] = [
            gc_i8_ptr_ty.into(),
            i64_ty.into(),
            ptr_ty.into(),
            ptr_ty.into(),
        ];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_alloc_typed(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_ALLOC_TYPED;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void *scoop_alloc_typed(void* type_desc, uint64_t size_bytes)`
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let ret_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i8_ptr_ty.into(), i64_ty.into()];
        let fn_ty = ret_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_once_begin(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_ONCE_BEGIN;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_once_begin(uint64_t* guard_word)`（TODO T0918）
        let i32_ty = self.context.i32_type();
        let i64_ptr_ty = self.context.ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ptr_ty.into()];
        let fn_ty = i32_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_once_end(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_ONCE_END;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_once_end(uint64_t* guard_word)`（TODO T0918）
        let i64_ptr_ty = self.context.ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_gc_write_barrier(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_GC_WRITE_BARRIER;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_gc_write_barrier(void* slot_addr, void* value)`
        //
        // 说明：
        // - `slot_addr` 是“slot 的地址”，而非 `void**` 的二级指针；
        // - v0 采用 promote-on-store，避免 old→nursery 指针；
        // - 返回值保留扩展点（未来可在需要时返回“写入后的新地址”）。
        //
        // GC/stackmap 重要性：
        // - `slot_addr` 只是一个 native 地址（指向某个 heap field/array slot 的地址），并不是 GC-managed 指针；
        // - 若把它声明为 `addrspace(1)`，LLVM statepoint 会把它当作 GC root 纳入 stackmap roots，
        //   从而在 `SCOOP_GC_VERIFY_ROOTS=1` 下产生“root 不是对象头地址”的误报，并可能导致 roots 更新协议混乱。
        // - 因此这里必须使用 `addrspace(0)` 指针类型承载 slot_addr（与 C ABI 的 `void*` 对齐）。
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i8_ptr_ty.into(), gc_i8_ptr_ty.into()];
        let fn_ty = gc_i8_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_gc_register_global_root(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_GC_REGISTER_GLOBAL_ROOT;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_gc_register_global_root(void* base, const ScoopTypeDescriptor* type_desc)`
        //
        // 说明：
        // - `base` 指向一个 module-local global backing slot 的起始地址；
        // - `type_desc` 描述该 backing slot 内部哪些 word 是 GC-managed pointers；
        // - runtime 会把该 backing slot 当作永久 roots，并在 moving GC 后按描述更新内部引用。
        let default_ptr_ty = self.context.ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] =
            [default_ptr_ty.into(), default_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_gc_collect_safepoint(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_GC_COLLECT_SAFEPOINT;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_gc_collect_safepoint(void)`
        //
        // 说明：
        // - 该函数在 C runtime 内部调用 `scoop_gc_collect()` 并返回 NULL；
        // - LLVM statepoint pipeline 依赖其“返回 GC ref”的形状，在调用点产出 stackmap record；
        // - codegen 会丢弃返回值，仅把它作为 safepoint 边界（供 roots 枚举/更新）。
        let ret_ptr_ty = self.llvm_gc_i8_ptr_type();
        let fn_ty = ret_ptr_ty.fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_enter_native(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_ENTER_NATIVE;
        if let Some(existing) = self.module.get_function(NAME) {
            self.mark_gc_leaf_function(existing);
            return existing;
        }

        // `void scoop_enter_native(void*** root_slots, uint32_t root_slots_len)`
        let slots_ptr_ty = self.context.ptr_type(AddressSpace::default());
        let i32_ty = self.context.i32_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [slots_ptr_ty.into(), i32_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        let function = self.module.add_function(NAME, fn_ty, None);
        self.mark_gc_leaf_function(function);
        function
    }

    pub(super) fn declare_runtime_leave_native(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_LEAVE_NATIVE;
        if let Some(existing) = self.module.get_function(NAME) {
            self.mark_gc_leaf_function(existing);
            return existing;
        }

        // `void scoop_leave_native(void)`
        let fn_ty = self.context.void_type().fn_type(&[], false);
        let function = self.module.add_function(NAME, fn_ty, None);
        self.mark_gc_leaf_function(function);
        function
    }

    pub(super) fn declare_runtime_gc_pin(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_PIN;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_pin(void* obj)`
        let i32_ty = self.context.i32_type();
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i8_ptr_ty.into()];
        let fn_ty = i32_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_gc_unpin(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_UNPIN;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_unpin(void* obj)`
        let i32_ty = self.context.i32_type();
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i8_ptr_ty.into()];
        let fn_ty = i32_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_gc_handle_new(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_HANDLE_NEW;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_handle_new(void* obj)`
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i8_ptr_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_gc_handle_get(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_HANDLE_GET;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_handle_get(uint64_t handle)`
        let i64_ty = self.context.i64_type();
        let ret_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = ret_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_gc_handle_drop(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_HANDLE_DROP;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_handle_drop(uint64_t handle)`
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = i32_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_gc_debug_heap_object_count(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_GC_DEBUG_HEAP_OBJECT_COUNT;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_gc_debug_heap_object_count(void)`
        let fn_ty = self.context.i64_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_gc_debug_alloc_garbage(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_GC_DEBUG_ALLOC_GARBAGE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_gc_debug_alloc_garbage(int64_t count)`
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_stackmap_statepoint_smoke(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_TEST_STACKMAP_STATEPOINT_SMOKE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `intptr_t scoop_test_stackmap_statepoint_smoke(void)`
        //
        // 说明：
        // - 该 helper 只服务“显式 opt-in 的 stackmap smoke”；
        // - lowering 侧会在包含该调用点的函数上单独恢复 `gc "statepoint-example"`，从而让
        //   调用点重新产出真实 statepoint/stackmap record；
        // - 同时它仍必须以 ordinary managed runtime 调用进入 LLVM IR；若改走 `@Extern` +
        //   enter_native/leave_native leaf lowering，则调用点本身不会留下 stackmap record，
        //   helper 内部的 `__builtin_return_address(0)` 也无法命中 registry。
        let fn_ty = self.context.i64_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {}
