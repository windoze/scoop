//! LLVM codegen：runtime C ABI 符号名（集中管理）。
//!
//! 注意：这里仅负责“符号名字符串”，签名/调用约定见 `runtime_abi.rs`。

pub(super) const SCOOP_ALLOC_TYPED: &str = "scoop_alloc_typed";
// T2999：effect 统一 lowering 尚未重新接回主入口，相关 ABI 符号先以集中、可审计的方式保留。
pub(super) const SCOOP_CALLEE_SUSPEND_STATE_CLEAR: &str = "scoop_callee_suspend_state_clear";
pub(super) const SCOOP_CALLEE_SUSPEND_STATE_GET: &str = "scoop_callee_suspend_state_get";
pub(super) const SCOOP_CALLEE_SUSPEND_STATE_SET: &str = "scoop_callee_suspend_state_set";
pub(super) const SCOOP_ARRAY_BUILDER_BUILD_ARRAY: &str = "scoop_array_builder_build_array";
pub(super) const SCOOP_ARRAY_BUILDER_BUILD_MUTABLE_ARRAY: &str =
    "scoop_array_builder_build_mutable_array";
pub(super) const SCOOP_ARRAY_BUILDER_NEW: &str = "scoop_array_builder_new";
pub(super) const SCOOP_ARRAY_BUILDER_PUSH_REF: &str = "scoop_array_builder_push_ref";
pub(super) const SCOOP_ARRAY_BUILDER_PUSH_U64: &str = "scoop_array_builder_push_u64";
pub(super) const SCOOP_ARRAY_GET_REF: &str = "scoop_array_get_ref";
pub(super) const SCOOP_ARRAY_GET_U64: &str = "scoop_array_get_u64";
pub(super) const SCOOP_ARRAY_LEN: &str = "scoop_array_len";
pub(super) const SCOOP_ARRAY_SET_REF: &str = "scoop_array_set_ref";
pub(super) const SCOOP_ARRAY_SET_U64: &str = "scoop_array_set_u64";
pub(super) const SCOOP_CHANNELS_CHANNEL_CREATE: &str = "scoop_channels_channel_create";
pub(super) const SCOOP_CHANNELS_CLOSE: &str = "scoop_channels_close";
pub(super) const SCOOP_CHANNELS_RECV_U64: &str = "scoop_channels_recv_u64";
pub(super) const SCOOP_CHANNELS_SEND_U64: &str = "scoop_channels_send_u64";
pub(super) const SCOOP_CONTINUATION_ALLOC: &str = "scoop_continuation_alloc";
pub(super) const SCOOP_CONTINUATION_RESUME: &str = "scoop_continuation_resume";
pub(super) const SCOOP_EFFECT_CLEAR: &str = "scoop_effect_clear";
pub(super) const SCOOP_EFFECT_HANDLER_STACK_POP: &str = "scoop_effect_handler_stack_pop";
pub(super) const SCOOP_EFFECT_HANDLER_STACK_PUSH: &str = "scoop_effect_handler_stack_push";
pub(super) const SCOOP_EFFECT_HANDLER_STACK_SET_ACTIVE: &str =
    "scoop_effect_handler_stack_set_active";
pub(super) const SCOOP_EFFECT_HANDLER_STACK_SWAP_TOP: &str = "scoop_effect_handler_stack_swap_top";
pub(super) const SCOOP_EFFECT_HANDLER_STACK_UNWIND_TO_TAG: &str =
    "scoop_effect_handler_stack_unwind_to_tag";
pub(super) const SCOOP_EFFECT_IS_ACTIVE: &str = "scoop_effect_is_active";
pub(super) const SCOOP_EFFECT_PERFORM_SLOT_READ_LEN_WORDS: &str =
    "scoop_effect_perform_slot_read_len_words";
pub(super) const SCOOP_EFFECT_PERFORM_SLOT_READ_OP_TAG: &str =
    "scoop_effect_perform_slot_read_op_tag";
pub(super) const SCOOP_EFFECT_PERFORM_SLOT_READ_GC_REF: &str =
    "scoop_effect_perform_slot_read_gc_ref";
pub(super) const SCOOP_EFFECT_PERFORM_SLOT_READ_U64: &str = "scoop_effect_perform_slot_read_u64";
pub(super) const SCOOP_EFFECT_PERFORM_SLOT_READ_U64_AT: &str =
    "scoop_effect_perform_slot_read_u64_at";
pub(super) const SCOOP_EFFECT_PERFORM_SLOT_WRITE_U64_WITH_GC_REF: &str =
    "scoop_effect_perform_slot_write_u64_with_gc_ref";
pub(super) const SCOOP_EFFECT_PERFORM_SLOT_WRITE_U64: &str = "scoop_effect_perform_slot_write_u64";
pub(super) const SCOOP_EFFECT_PERFORM_SLOT_WRITE_U64_2: &str =
    "scoop_effect_perform_slot_write_u64_2";
pub(super) const SCOOP_EFFECT_SET_ACTIVE: &str = "scoop_effect_set_active";
pub(super) const SCOOP_EFFECT_SET_ACTIVE_WITH_TRACE: &str = "scoop_effect_set_active_with_trace";
pub(super) const SCOOP_ENTER_NATIVE: &str = "scoop_enter_native";
pub(super) const SCOOP_ENV_GET: &str = "scoop_env_get";
pub(super) const SCOOP_EXECUTOR_CREATE: &str = "scoop_executor_create";
pub(super) const SCOOP_EXECUTOR_DEBUG_PENDING_COUNT: &str = "scoop_executor_debug_pending_count";
pub(super) const SCOOP_EXECUTOR_DESTROY: &str = "scoop_executor_destroy";
pub(super) const SCOOP_EXECUTOR_RUN_NEXT: &str = "scoop_executor_run_next";
pub(super) const SCOOP_EXECUTOR_RUN_UNTIL_IDLE: &str = "scoop_executor_run_until_idle";
pub(super) const SCOOP_FS_READ_ALL_TEXT_UTF8: &str = "scoop_fs_read_all_text_utf8";
pub(super) const SCOOP_FS_WRITE_ALL_TEXT_UTF8: &str = "scoop_fs_write_all_text_utf8";
pub(super) const SCOOP_GC_COLLECT_SAFEPOINT: &str = "scoop_gc_collect_safepoint";
pub(super) const SCOOP_GC_DEBUG_ALLOC_GARBAGE: &str = "scoop_gc_debug_alloc_garbage";
pub(super) const SCOOP_GC_DEBUG_HEAP_OBJECT_COUNT: &str = "scoop_gc_debug_heap_object_count";
pub(super) const SCOOP_GC_WRITE_BARRIER: &str = "scoop_gc_write_barrier";
pub(super) const SCOOP_HANDLE_DROP: &str = "scoop_handle_drop";
pub(super) const SCOOP_HANDLE_GET: &str = "scoop_handle_get";
pub(super) const SCOOP_HANDLE_NEW: &str = "scoop_handle_new";
pub(super) const SCOOP_IO_STDIN_READ_LINE_UTF8: &str = "scoop_io_stdin_read_line_utf8";
pub(super) const SCOOP_LEAVE_NATIVE: &str = "scoop_leave_native";
pub(super) const SCOOP_ONCE_BEGIN: &str = "scoop_once_begin";
pub(super) const SCOOP_ONCE_END: &str = "scoop_once_end";
pub(super) const SCOOP_PATH_BASENAME: &str = "scoop_path_basename";
pub(super) const SCOOP_PATH_DIRNAME: &str = "scoop_path_dirname";
pub(super) const SCOOP_PATH_JOIN: &str = "scoop_path_join";
pub(super) const SCOOP_PATH_NORMALIZE: &str = "scoop_path_normalize";
pub(super) const SCOOP_PIN: &str = "scoop_pin";
pub(super) const SCOOP_PROCESS_ARGS_ARRAY: &str = "scoop_process_args_array";
pub(super) const SCOOP_PROCESS_EXIT: &str = "scoop_process_exit";
pub(super) const SCOOP_BOOL_TO_STRING: &str = "scoop_bool_to_string";
pub(super) const SCOOP_CHAR_TO_STRING: &str = "scoop_char_to_string";
pub(super) const SCOOP_FLOAT32_TO_INT: &str = "scoop_float32_to_int";
pub(super) const SCOOP_FLOAT32_TO_STRING: &str = "scoop_float32_to_string";
pub(super) const SCOOP_FLOAT64_TO_INT: &str = "scoop_float64_to_int";
pub(super) const SCOOP_FLOAT64_TO_STRING: &str = "scoop_float64_to_string";
pub(super) const SCOOP_INT_TO_STRING: &str = "scoop_int_to_string";
pub(super) const SCOOP_STRING_CHAR_AT: &str = "scoop_string_char_at";
pub(super) const SCOOP_STRING_COMPARE_TO: &str = "scoop_string_compare_to";
pub(super) const SCOOP_STRING_CONCAT: &str = "scoop_string_concat";
pub(super) const SCOOP_STRING_EQUALS: &str = "scoop_string_equals";
pub(super) const SCOOP_STRING_HASH: &str = "scoop_string_hash";
pub(super) const SCOOP_STRING_IS_EMPTY: &str = "scoop_string_is_empty";
pub(super) const SCOOP_STRING_LENGTH: &str = "scoop_string_length";
pub(super) const SCOOP_STRING_REPEAT: &str = "scoop_string_repeat";
pub(super) const SCOOP_STRING_REPLACE: &str = "scoop_string_replace";
pub(super) const SCOOP_STRING_TO_INT: &str = "scoop_string_to_int";
pub(super) const SCOOP_STRING_TRIM_INDENT: &str = "scoop_string_trim_indent";
pub(super) const SCOOP_STRING_UNSAFE_SLICE_BYTES: &str = "scoop_string_unsafe_slice_bytes";
pub(super) const SCOOP_SYNC_CONDVAR_CREATE: &str = "scoop_sync_condvar_create";
pub(super) const SCOOP_SYNC_CONDVAR_DESTROY: &str = "scoop_sync_condvar_destroy";
pub(super) const SCOOP_SYNC_CONDVAR_NOTIFY_ALL: &str = "scoop_sync_condvar_notify_all";
pub(super) const SCOOP_SYNC_CONDVAR_NOTIFY_ONE: &str = "scoop_sync_condvar_notify_one";
pub(super) const SCOOP_SYNC_CONDVAR_WAIT: &str = "scoop_sync_condvar_wait";
pub(super) const SCOOP_SYNC_MUTEX_CREATE: &str = "scoop_sync_mutex_create";
pub(super) const SCOOP_SYNC_MUTEX_DESTROY: &str = "scoop_sync_mutex_destroy";
pub(super) const SCOOP_SYNC_MUTEX_LOCK: &str = "scoop_sync_mutex_lock";
pub(super) const SCOOP_SYNC_MUTEX_UNLOCK: &str = "scoop_sync_mutex_unlock";
pub(super) const SCOOP_SYNC_ONCE_CREATE: &str = "scoop_sync_once_create";
pub(super) const SCOOP_SYNC_ONCE_IS_DONE: &str = "scoop_sync_once_is_done";
pub(super) const SCOOP_SYNC_ONCE_RUN: &str = "scoop_sync_once_run";
pub(super) const SCOOP_TASK_JOIN_INT: &str = "scoop_task_join_int";
pub(super) const SCOOP_TASK_SPAWN_INT: &str = "scoop_task_spawn_int";
pub(super) const SCOOP_TASK_U64_COMPLETE: &str = "scoop_task_u64_complete";
pub(super) const SCOOP_TASK_U64_CREATE: &str = "scoop_task_u64_create";
pub(super) const SCOOP_TASK_U64_ON_COMPLETE_RESUME_U64: &str =
    "scoop_task_u64_on_complete_resume_u64";
pub(super) const SCOOP_TASK_U64_RESULT: &str = "scoop_task_u64_result";
pub(super) const SCOOP_TASK_U64_STATE: &str = "scoop_task_u64_state";
pub(super) const SCOOP_TASK_U64_TRY_START: &str = "scoop_task_u64_try_start";
pub(super) const SCOOP_THREAD_CURRENT_ID: &str = "scoop_thread_current_id";
pub(super) const SCOOP_THREAD_JOIN: &str = "scoop_thread_join";
pub(super) const SCOOP_THREAD_SLEEP_MILLIS: &str = "scoop_thread_sleep_millis";
pub(super) const SCOOP_THREAD_SPAWN: &str = "scoop_thread_spawn";
pub(super) const SCOOP_THREAD_SPAWN_JOIN_RESUME_U64: &str = "scoop_thread_spawn_join_resume_u64";
pub(super) const SCOOP_THREAD_YIELD: &str = "scoop_thread_yield";
pub(super) const SCOOP_TIME_NOW_UNIX_MILLIS: &str = "scoop_time_now_unix_millis";
pub(super) const SCOOP_UNPIN: &str = "scoop_unpin";
