//! LLVM codegen：runtime ABI glue（符号声明 / 调用约定）。
//!
//! 目标：把 runtime 的 C ABI 声明集中管理，避免在 expr/stmt codegen 中散落 `declare_*`。

use inkwell::AddressSpace;
use inkwell::types::BasicMetadataTypeEnum;
use inkwell::types::StructType;
use inkwell::values::FunctionValue;

use super::MainCodegen;
use super::runtime_symbols;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(super) fn llvm_effect_handler_frame_type(&self) -> StructType<'ctx> {
        // 说明：
        // - 该类型对应 `runtime/c/scoop_runtime.c` 的 `ScoopEffectHandlerFrame`（TODO T0913）；
        // - v0 只要求 `{ prev: i8*, op_tag: i32, active: i32 }` 的稳定布局；
        // - codegen 不直接访问字段，只负责在栈上分配并把指针传给 runtime push/pop/active API。
        const TY_NAME: &str = "scoop.runtime.ScoopEffectHandlerFrame";

        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        let i8_ptr_ty = self.context.ptr_type(AddressSpace::default());
        let i32_ty = self.context.i32_type();
        ty.set_body(&[i8_ptr_ty.into(), i32_ty.into(), i32_ty.into()], false);
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

    pub(super) fn declare_runtime_env_get(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_ENV_GET;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `const ScoopString* scoop_env_get(const ScoopString* key)`
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [str_ptr_ty.into()];
        let fn_ty = str_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_time_now_unix_millis(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_TIME_NOW_UNIX_MILLIS;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `int64_t scoop_time_now_unix_millis(void)`
        let i64_ty = self.context.i64_type();
        let fn_ty = i64_ty.fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_fs_read_all_text_utf8(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_FS_READ_ALL_TEXT_UTF8;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `const ScoopString* scoop_fs_read_all_text_utf8(const ScoopString* path)`
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [str_ptr_ty.into()];
        let fn_ty = str_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_fs_write_all_text_utf8(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_FS_WRITE_ALL_TEXT_UTF8;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `int64_t scoop_fs_write_all_text_utf8(const ScoopString* path, const ScoopString* content)`
        let i64_ty = self.context.i64_type();
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [str_ptr_ty.into(), str_ptr_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_process_exit(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_PROCESS_EXIT;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_process_exit(int64_t code)`
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_process_args_array(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_PROCESS_ARGS_ARRAY;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_process_args_array(void)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let fn_ty = gc_i8_ptr_ty.fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_io_stdin_read_line_utf8(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_IO_STDIN_READ_LINE_UTF8;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `const ScoopString* scoop_io_stdin_read_line_utf8(void)`
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let fn_ty = str_ptr_ty.fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_path_normalize(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_PATH_NORMALIZE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `const ScoopString* scoop_path_normalize(const ScoopString* path)`
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [str_ptr_ty.into()];
        let fn_ty = str_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_path_join(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_PATH_JOIN;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `const ScoopString* scoop_path_join(const ScoopString* base, const ScoopString* child)`
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [str_ptr_ty.into(), str_ptr_ty.into()];
        let fn_ty = str_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_path_basename(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_PATH_BASENAME;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `const ScoopString* scoop_path_basename(const ScoopString* path)`
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [str_ptr_ty.into()];
        let fn_ty = str_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_path_dirname(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_PATH_DIRNAME;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `const ScoopString* scoop_path_dirname(const ScoopString* path)`
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [str_ptr_ty.into()];
        let fn_ty = str_ptr_ty.fn_type(&param_tys, false);
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

    // --- std v3：channels（T1319d） ---

    pub(super) fn declare_runtime_channels_channel_create(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_CHANNELS_CHANNEL_CREATE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_channels_channel_create(void)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let fn_ty = gc_i8_ptr_ty.fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_channels_send_u64(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_CHANNELS_SEND_U64;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_channels_send_u64(void* channel, uint64_t value)`
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let i32_ty = self.context.i32_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i8_ptr_ty.into(), i64_ty.into()];
        let fn_ty = i32_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_channels_recv_u64(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_CHANNELS_RECV_U64;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_channels_recv_u64(void* channel, uint64_t* out_value)`
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ptr_ty = self.context.ptr_type(AddressSpace::default());
        let i32_ty = self.context.i32_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i8_ptr_ty.into(), i64_ptr_ty.into()];
        let fn_ty = i32_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_channels_close(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_CHANNELS_CLOSE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_channels_close(void* channel)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [gc_i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    // --- std v3：task/executor（T1319e） ---

    pub(super) fn declare_runtime_executor_create(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_EXECUTOR_CREATE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_executor_create(void)`
        let fn_ty = self.context.i64_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_executor_destroy(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_EXECUTOR_DESTROY;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_executor_destroy(uint64_t executor_handle)`
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_executor_debug_pending_count(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_EXECUTOR_DEBUG_PENDING_COUNT;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_executor_debug_pending_count(uint64_t executor_handle)`
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_executor_run_next(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_EXECUTOR_RUN_NEXT;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_executor_run_next(uint64_t executor_handle)`
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_executor_run_until_idle(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_EXECUTOR_RUN_UNTIL_IDLE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_executor_run_until_idle(uint64_t executor_handle, uint64_t max_steps)`
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i64_ty.into(), i64_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_task_u64_create(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_TASK_U64_CREATE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_task_u64_create(uint64_t (*body_fn)(void*), void* body_ctx)`
        let i8_ptr_ty = self.context.ptr_type(AddressSpace::default());
        let i64_ty = self.context.i64_type();
        let body_fn_ptr_ty = self.context.ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [body_fn_ptr_ty.into(), i8_ptr_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_task_u64_state(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_TASK_U64_STATE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_task_u64_state(uint64_t task_handle)`
        let i64_ty = self.context.i64_type();
        let i32_ty = self.context.i32_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = i32_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_task_u64_result(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_TASK_U64_RESULT;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_task_u64_result(uint64_t task_handle)`
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_task_u64_try_start(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_TASK_U64_TRY_START;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_task_u64_try_start(uint64_t task_handle, uint64_t executor_handle)`
        let i64_ty = self.context.i64_type();
        let i32_ty = self.context.i32_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i64_ty.into(), i64_ty.into()];
        let fn_ty = i32_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_task_u64_complete(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_TASK_U64_COMPLETE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_task_u64_complete(uint64_t task_handle, uint64_t value)`
        let i64_ty = self.context.i64_type();
        let i32_ty = self.context.i32_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i64_ty.into(), i64_ty.into()];
        let fn_ty = i32_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_task_u64_on_complete_resume_u64(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_TASK_U64_ON_COMPLETE_RESUME_U64;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_task_u64_on_complete_resume_u64(uint64_t task_handle, uint64_t executor_handle, void* continuation)`
        let i64_ty = self.context.i64_type();
        let i32_ty = self.context.i32_type();
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 3] =
            [i64_ty.into(), i64_ty.into(), i8_ptr_ty.into()];
        let fn_ty = i32_ty.fn_type(&param_tys, false);
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
            return existing;
        }

        // `void scoop_enter_native(void*** root_slots, uint32_t root_slots_len)`
        let slots_ptr_ty = self.context.ptr_type(AddressSpace::default());
        let i32_ty = self.context.i32_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [slots_ptr_ty.into(), i32_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_leave_native(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_LEAVE_NATIVE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_leave_native(void)`
        let fn_ty = self.context.void_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
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

    pub(super) fn declare_runtime_task_spawn_int(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_TASK_SPAWN_INT;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_task_spawn_int(int64_t value)`
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_task_join_int(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_TASK_JOIN_INT;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `int64_t scoop_task_join_int(uint64_t handle)`
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }
}

// Effect runtime ABI 声明：被 `codegen_sysroot_effect_intrinsics`、`codegen_perform_expr`、
// `emit_raise_runtime_error_variant` 与 state machine emitter 等生产路径消费。
impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(super) fn declare_runtime_effect_is_active(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_EFFECT_IS_ACTIVE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_effect_is_active(void)`
        let fn_ty = self.context.i32_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_effect_set_active(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_EFFECT_SET_ACTIVE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_effect_set_active(void)`
        let fn_ty = self.context.void_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_effect_clear(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_EFFECT_CLEAR;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_effect_clear(void)`
        let fn_ty = self.context.void_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_effect_clear_active(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_EFFECT_CLEAR_ACTIVE;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_effect_clear_active(void)`
        let fn_ty = self.context.void_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_effect_handler_stack_push(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_EFFECT_HANDLER_STACK_PUSH;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_effect_handler_stack_push(ScoopEffectHandlerFrame* frame, uint32_t op_tag)`
        let i8_ptr_ty = self.context.ptr_type(AddressSpace::default());
        let i32_ty = self.context.i32_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i8_ptr_ty.into(), i32_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_effect_handler_stack_pop(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_EFFECT_HANDLER_STACK_POP;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_effect_handler_stack_pop(ScoopEffectHandlerFrame* frame)`
        let i8_ptr_ty = self.context.ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    /// 发布当前 ordinary indirect-callee 的 suspend state 到 runtime TLS。
    ///
    /// 语义上该 state 始终是 GC-managed 对象；编译器 fresh path 保存完 locals/captures
    /// 后调用 publish，outer state-machine 的 `Suspend` terminator 再用 get+clear 把它
    /// 提升为 continuation 的正式字段。
    pub(super) fn declare_runtime_callee_suspend_state_publish(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_CALLEE_SUSPEND_STATE_PUBLISH;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [gc_i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    /// 读取当前线程暂存的 indirect callee suspend state。
    ///
    /// 该值在 runtime 中以 `void*` 暴露，但语义上始终指向 GC-managed 的
    /// callee suspend object，因此 LLVM 侧将其视为 `ptr addrspace(1)`。
    pub(super) fn declare_runtime_callee_suspend_state_get(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_CALLEE_SUSPEND_STATE_GET;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_callee_suspend_state_get(void)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let fn_ty = gc_i8_ptr_ty.fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    /// 清空当前线程暂存的 indirect callee suspend state。
    ///
    /// continuation 在 suspend 时会把该 state 提升为 continuation 的正式字段；
    /// clear 之后，TLS 只承担运行期临时寄存职责，不再承担持久化语义。
    pub(super) fn declare_runtime_callee_suspend_state_clear(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_CALLEE_SUSPEND_STATE_CLEAR;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_callee_suspend_state_clear(void)`
        let fn_ty = self.context.void_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_continuation_alloc(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_CONTINUATION_ALLOC;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // T1607：step_fn 签名扩展为 3 参数——(state, resume_word, resume_gc_ref)。
        // `void* scoop_continuation_alloc(void* state, void (*step_fn)(void*, uint64_t, void*))`
        // LLVM 侧将 `state` / `resume_gc_ref` 都视为 addrspace(1) GC ref，
        // 这样 continuation trace 与 statepoint rewrite 都使用同一套 GC 指针约定。
        let state_ptr_ty = self.llvm_gc_i8_ptr_type();
        let step_fn_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] =
            [state_ptr_ty.into(), step_fn_ptr_ty.into()];
        let fn_ty = state_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    /// T1607：新 ABI——调用方已将 payload 写入 continuation 的 resume_word / resume_gc_ref 槽位。
    pub(super) fn declare_runtime_continuation_resume(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_CONTINUATION_RESUME;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_continuation_resume(void* k)`
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    /// T1607：返回 `ScoopContinuation` 的 LLVM 结构类型（用于 GEP 到
    /// `resume_word` / `resume_gc_ref` / captured callee suspend state）。
    ///
    /// 布局与 `runtime/c/scoop_runtime.c` 的 `ScoopContinuation` 一致：
    ///   { ScoopGcObjectHeader, i32 resumed, i32 resume_state_tag, ptr captured_handler_stack_top,
    ///     ptr addrspace(1) state, ptr step_fn, i64 resume_word, ptr addrspace(1) resume_gc_ref,
    ///     ptr addrspace(1) captured_callee_suspend_state }
    pub(super) fn llvm_continuation_struct_type(&self) -> inkwell::types::StructType<'ctx> {
        const TY_NAME: &str = "scoop.runtime.ScoopContinuation";
        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }
        let ty = self.context.opaque_struct_type(TY_NAME);
        let header_ty = self.llvm_gc_object_header_type();
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        ty.set_body(
            &[
                header_ty.into(),    // 0: hdr
                i32_ty.into(),       // 1: resumed (_Atomic uint32_t)
                i32_ty.into(),       // 2: resume_state_tag
                i8_ptr_ty.into(),    // 3: captured_handler_stack_top
                gc_i8_ptr_ty.into(), // 4: state
                i8_ptr_ty.into(),    // 5: step_fn
                i64_ty.into(),       // 6: resume_word
                gc_i8_ptr_ty.into(), // 7: resume_gc_ref
                gc_i8_ptr_ty.into(), // 8: captured_callee_suspend_state
            ],
            false,
        );
        ty
    }

    pub(super) fn declare_runtime_thread_spawn_join_resume_u64(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_THREAD_SPAWN_JOIN_RESUME_U64;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_thread_spawn_join_resume_u64(void* k, uint64_t resume_value)`
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i8_ptr_ty.into(), i64_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_effect_perform_slot_write_u64(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_EFFECT_PERFORM_SLOT_WRITE_U64;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_effect_perform_slot_write_u64(uint32_t op_tag, uint32_t effect_instance_key, uint64_t value)`
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 3] =
            [i32_ty.into(), i32_ty.into(), i64_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_effect_perform_slot_write_u64_with_gc_ref(
        &self,
    ) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_EFFECT_PERFORM_SLOT_WRITE_U64_WITH_GC_REF;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_effect_perform_slot_write_u64_with_gc_ref(uint32_t op_tag, uint32_t effect_instance_key, uint64_t word0, void* gc_ref)`
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 4] = [
            i32_ty.into(),
            i32_ty.into(),
            i64_ty.into(),
            gc_i8_ptr_ty.into(),
        ];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_effect_perform_slot_write_u64_2(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_EFFECT_PERFORM_SLOT_WRITE_U64_2;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_effect_perform_slot_write_u64_2(uint32_t op_tag, uint32_t effect_instance_key, uint64_t word0, uint64_t word1)`
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 4] =
            [i32_ty.into(), i32_ty.into(), i64_ty.into(), i64_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_effect_perform_slot_read_op_tag(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_EFFECT_PERFORM_SLOT_READ_OP_TAG;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_effect_perform_slot_read_op_tag(void)`
        let fn_ty = self.context.i32_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_effect_perform_slot_read_effect_instance_key(
        &self,
    ) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_EFFECT_PERFORM_SLOT_READ_EFFECT_INSTANCE_KEY;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_effect_perform_slot_read_effect_instance_key(void)`
        let fn_ty = self.context.i32_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_effect_perform_slot_read_len_words(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_EFFECT_PERFORM_SLOT_READ_LEN_WORDS;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_effect_perform_slot_read_len_words(void)`
        let fn_ty = self.context.i32_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_effect_perform_slot_read_gc_ref(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_EFFECT_PERFORM_SLOT_READ_GC_REF;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_effect_perform_slot_read_gc_ref(void)`
        let fn_ty = self.llvm_gc_i8_ptr_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_effect_perform_slot_read_u64(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_EFFECT_PERFORM_SLOT_READ_U64;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_effect_perform_slot_read_u64(void)`
        let fn_ty = self.context.i64_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    pub(super) fn declare_runtime_effect_perform_slot_read_u64_at(&self) -> FunctionValue<'ctx> {
        const NAME: &str = runtime_symbols::SCOOP_EFFECT_PERFORM_SLOT_READ_U64_AT;
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_effect_perform_slot_read_u64_at(uint32_t index)`
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i32_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }
}
