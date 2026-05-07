// Scoop runtime ABI allowlist (early stage).
//
// 说明：
// - 该文件用于“集中声明 & 审计”runtime/c 对外导出的 C 符号（ABI 边界）。
// - CI/单测会对比 `nm`/`objdump` 导出符号与本清单：若出现未登记导出则失败。
// - 当你在 `runtime/c/*.c` 中新增 **非 static** 的全局函数/变量时：
//   1) 请确认它确实属于“编译器 ↔ runtime”的必要 ABI；
//   2) 将其加入本文件的 allowlist；
//   3) 如是平台相关能力，优先考虑后续放入 `runtime/c/platform/*`（见 PLAN §15）。
//
// 注意：本文件不是传统意义的“可 include 头文件”（当前阶段不提供类型/函数声明）。
// 它采用 X-macro 列表形式，便于工具/测试做一致性检查与版本化演进。

#ifndef SCOOP_RUNTIME_API_H
#define SCOOP_RUNTIME_API_H

// X-macro：对外导出的 runtime C 符号清单（按字典序）。
//
// 用法示例：
//   #define X(sym) do_something_with(#sym)
//   SCOOP_RUNTIME_API_SYMBOLS(X)
//   #undef X
#define SCOOP_RUNTIME_API_SYMBOLS(X) \
  X(__start_llvm_stackmaps) \
  X(__stop_llvm_stackmaps) \
  X(__scoop_effect_active) \
  X(__scoop_effect_handler_stack_top) \
  X(__scoop_effect_perform_slot) \
  X(__scoop_explicit_root_frame_top) \
  X(scoop_alloc) \
  X(scoop_alloc_typed) \
  X(scoop_callee_suspend_state_clear) \
  X(scoop_callee_suspend_state_get) \
  X(scoop_callee_suspend_state_publish) \
  X(scoop_array_builder_build_array) \
  X(scoop_array_builder_build_array_composite) \
  X(scoop_array_builder_build_mutable_array) \
  X(scoop_array_builder_build_mutable_array_composite) \
  X(scoop_array_builder_new) \
  X(scoop_array_builder_push_composite) \
  X(scoop_array_builder_push_ref) \
  X(scoop_array_builder_push_u64) \
  X(scoop_array_get_composite) \
  X(scoop_array_get_ref) \
  X(scoop_array_get_u64) \
  X(scoop_array_len) \
  X(scoop_array_set_composite) \
  X(scoop_array_set_ref) \
  X(scoop_array_set_u64) \
  X(scoop_continuation_alloc) \
  X(scoop_continuation_discard) \
  X(scoop_continuation_resume) \
  X(scoop_continuation_resume_into) \
  X(scoop_continuation_resume_publish_pending_continuation) \
  X(scoop_continuation_set_captured_callee_suspend_state) \
  X(scoop_continuation_resume_u64) \
  X(scoop_continuation_resume_with) \
  X(scoop_continuation_try_resume) \
  X(scoop_effect_clear) \
  X(scoop_effect_clear_active) \
  X(scoop_effect_handler_stack_find_nearest) \
  X(scoop_effect_handler_stack_pop) \
  X(scoop_effect_handler_stack_push) \
  X(scoop_effect_handler_stack_set_active) \
  X(scoop_effect_handler_stack_swap_top) \
  X(scoop_effect_handler_stack_unwind_to_tag) \
  X(scoop_effect_handler_stack_top) \
  X(scoop_effect_is_active) \
  X(scoop_effect_outcome_consume_current) \
  X(scoop_effect_outcome_publish) \
  X(scoop_effect_perform_slot_read_effect_instance_key) \
  X(scoop_effect_perform_slot_read_len_words) \
  X(scoop_effect_perform_slot_read_op_tag) \
  X(scoop_effect_perform_slot_read_gc_ref) \
  X(scoop_effect_perform_slot_read_u64) \
  X(scoop_effect_perform_slot_read_u64_at) \
  X(scoop_effect_perform_slot_write_u64_with_gc_ref) \
  X(scoop_effect_perform_slot_write_u64) \
  X(scoop_effect_perform_slot_write_u64_2) \
  X(scoop_effect_set_active) \
  X(scoop_effect_set_active_with_trace) \
  X(scoop_effect_trace_src_col) \
  X(scoop_effect_trace_src_line) \
  X(scoop_effect_trace_unwind_len) \
  X(scoop_enter_native) \
  X(scoop_float32_to_int) \
  X(scoop_float32_to_string) \
  X(scoop_float64_to_int) \
  X(scoop_float64_to_string) \
  X(scoop_format_i64) \
  X(scoop_format_u64) \
  X(scoop_gc_collect) \
  X(scoop_gc_collect_minor) \
  X(scoop_gc_collect_safepoint) \
  X(scoop_gc_debug_alloc_garbage) \
  X(scoop_gc_debug_heap_bytes_allocated) \
  X(scoop_gc_debug_heap_bytes_freed) \
  X(scoop_gc_debug_heap_bytes_reserved) \
  X(scoop_gc_debug_heap_object_count) \
  X(scoop_gc_heap) \
  X(scoop_gc_heap_init) \
  X(scoop_gc_heap_register_object) \
  X(scoop_gc_register_global_root) \
  X(scoop_gc_safepoint) \
  X(scoop_gc_safepoint_poll) \
  X(scoop_gc_self_check) \
  X(scoop_gc_thread_clear_managed_root_snapshot_current) \
  X(scoop_gc_thread_register) \
  X(scoop_gc_thread_unregister) \
  X(scoop_gc_try_collect_minor) \
  X(scoop_gc_type_descriptor_trace) \
  X(scoop_gc_write_barrier) \
  X(scoop_handle_drop) \
  X(scoop_handle_drop_in_release) \
  X(scoop_handle_get) \
  X(scoop_handle_new) \
  X(scoop_leave_native) \
  X(scoop_once_begin) \
  X(scoop_once_end) \
  X(scoop_once_guard_canonicalize) \
  X(scoop_panic) \
  X(scoop_pin) \
  X(scoop_print) \
  X(scoop_println) \
  X(scoop_entry_argv_array) \
  X(scoop_runtime_error_fatal) \
  X(scoop_runtime_init) \
  X(scoop_runtime_init_count) \
  X(scoop_runtime_is_initialized) \
  X(scoop_stackmap_record_visit_root_slots) \
  X(scoop_stackmap_registry_lookup) \
  X(scoop_stackmap_registry_record_count) \
  X(scoop_stackmap_registry_register_current_process) \
  X(scoop_stackmap_registry_register_section) \
  X(scoop_stackmap_registry_reset) \
  X(scoop_bool_to_string) \
  X(scoop_char_to_string) \
  X(scoop_composite_copy) \
  X(scoop_composite_drop) \
  X(scoop_composite_trace) \
  X(scoop_int_to_string) \
  X(scoop_string_char_at) \
  X(scoop_string_compare_to) \
  X(scoop_string_concat) \
  X(scoop_string_equals) \
  X(scoop_string_hash) \
  X(scoop_string_is_empty) \
  X(scoop_string_length) \
  X(scoop_string_repeat) \
  X(scoop_string_replace) \
  X(scoop_string_to_float64) \
  X(scoop_string_to_int) \
  X(scoop_string_trim_indent) \
  X(scoop_string_unsafe_slice_bytes) \
  X(scoop_sync_condvar_create) \
  X(scoop_sync_condvar_destroy) \
  X(scoop_sync_condvar_notify_all) \
  X(scoop_sync_condvar_notify_one) \
  X(scoop_sync_condvar_wait) \
  X(scoop_sync_mutex_create) \
  X(scoop_sync_mutex_destroy) \
  X(scoop_sync_mutex_lock) \
  X(scoop_sync_mutex_unlock) \
  X(scoop_sync_once_create) \
  X(scoop_sync_once_is_done) \
  X(scoop_sync_once_run) \
  X(scoop_test_add_int) \
  X(scoop_test_callee_suspend_state_set) \
  X(scoop_test_extern_global_counter) \
  X(scoop_test_explicit_root_frame_enter_native_smoke) \
  X(scoop_test_continuation_resume_replay_state_create) \
  X(scoop_test_explicit_root_frame_root_map_smoke) \
  X(scoop_test_explicit_root_frame_top) \
  X(scoop_test_gc_collect_in_native) \
  X(scoop_test_gc_native_sleep_entered) \
  X(scoop_test_gc_native_sleep_reset) \
  X(scoop_test_gc_sleep_in_native_ms) \
  X(scoop_test_gc_stackmap_multiframe_keepalive) \
  X(scoop_test_gc_stackmap_roots_enum_smoke) \
  X(scoop_test_gc_stack_walking_ctx_smoke) \
  X(scoop_test_gc_stack_walking_unwind_smoke) \
  X(scoop_test_get_add_int_funptr) \
  X(scoop_test_get_make_int_pair_funptr) \
  X(scoop_test_handle_token_slot_reset) \
  X(scoop_test_handle_token_slot_store) \
  X(scoop_test_handle_token_slot_take) \
  X(scoop_test_handle_get_object_addr) \
  X(scoop_test_sync_condvar_destroy_count) \
  X(scoop_test_sync_destroy_counts_reset) \
  X(scoop_test_sync_mutex_destroy_count) \
  X(scoop_test_sync_once_destroy_count) \
  X(scoop_test_stackmap_statepoint_smoke) \
  X(scoop_test_thread_spawn_gate_enable) \
  X(scoop_test_thread_spawn_gate_entered) \
  X(scoop_test_thread_spawn_gate_release) \
  X(scoop_test_thread_spawn_gate_reset) \
  X(scoop_test_unwind_capture_ips) \
  X(scoop_test_unwind_dump_frames_and_stackmap_hits) \
  X(scoop_thread_current_id) \
  X(scoop_thread_is_registered) \
  X(scoop_thread_join) \
  X(scoop_thread_register) \
  X(scoop_thread_sleep_millis) \
  X(scoop_thread_spawn) \
  X(scoop_thread_spawn_join_refactor_resume_transport) \
  X(scoop_thread_spawn_join_refactor_resume_u64) \
  X(scoop_thread_spawn_join_resume_u64) \
  X(scoop_thread_unregister) \
  X(scoop_thread_yield) \
  X(scoop_unpin)

#endif // SCOOP_RUNTIME_API_H
