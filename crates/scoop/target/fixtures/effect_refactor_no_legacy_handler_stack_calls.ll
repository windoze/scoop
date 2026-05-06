; ModuleID = 'effect_refactor_no_legacy_handler_stack_calls'
source_filename = "effect_refactor_no_legacy_handler_stack_calls"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-darwin25.4.0"

%scoop.refactor.StepComplete__fixtures_build_main = type { i64 }
%scoop.refactor.StepCase__fixtures_build_main__case0 = type { ptr addrspace(1) }
%scoop.refactor.StepCase__fixtures_build_main__case1 = type { %scoop.core.RuntimeError, ptr addrspace(1) }
%scoop.core.RuntimeError = type { i32, i64, ptr addrspace(1) }
%scoop.refactor.Step__fixtures_build_main = type { i32, %scoop.refactor.StepStorage__fixtures_build_main }
%scoop.refactor.StepStorage__fixtures_build_main = type { [4 x i64] }
%scoop.refactor.ResumeVtable__fixtures_build_main__fixtures_build_Ping = type { ptr }
%scoop.refactor.ResumeVtable__fixtures_build_main__scoop_core_Raise = type { ptr }
%scoop.refactor.Frame__fixtures_build_main = type { %scoop.runtime.ScoopGcObjectHeader, ptr addrspace(1), i64, i64, i8, i64, ptr addrspace(1), i1, i1, i64 }
%scoop.runtime.ScoopGcObjectHeader = type { ptr, ptr, i64, i32, i32 }
%scoop.refactor.Continuation__fixtures_build_main = type { %scoop.runtime.ScoopGcObjectHeader, ptr addrspace(1), i32, i1, ptr addrspace(1), ptr, ptr }
%scoop.runtime.ScoopTypeDescriptor = type { i32, i32, i64, i64, i64, i32, i32, ptr, ptr, ptr, i64, ptr, ptr, ptr }
%scoop.runtime.ScoopRootFrameDesc = type { i32, ptr }
%scoop.runtime.ScoopRootFrameHeader = type { ptr, ptr }
%scoop.runtime.ScoopString = type { %scoop.runtime.ScoopGcObjectHeader, i64, ptr }

@__scoop_refactor_step_variant_payload__fixtures_build_main__complete = internal constant %scoop.refactor.StepComplete__fixtures_build_main zeroinitializer
@__scoop_refactor_step_case_tag__fixtures_build_main__complete = internal constant i32 0
@__scoop_refactor_step_variant_payload__fixtures_build_main__case0 = internal constant %scoop.refactor.StepCase__fixtures_build_main__case0 zeroinitializer
@__scoop_refactor_step_case_tag__fixtures_build_main__case0 = internal constant i32 1
@__scoop_refactor_step_variant_payload__fixtures_build_main__case1 = internal constant %scoop.refactor.StepCase__fixtures_build_main__case1 zeroinitializer
@__scoop_refactor_step_case_tag__fixtures_build_main__case1 = internal constant i32 2
@__scoop_refactor_step_layout__fixtures_build_main = internal constant %scoop.refactor.Step__fixtures_build_main zeroinitializer
@__scoop_refactor_resume_vtable_layout__fixtures_build_main__fixtures_build_Ping = internal constant %scoop.refactor.ResumeVtable__fixtures_build_main__fixtures_build_Ping zeroinitializer
@__scoop_refactor_resume_vtable_layout__fixtures_build_main__scoop_core_Raise = internal constant %scoop.refactor.ResumeVtable__fixtures_build_main__scoop_core_Raise zeroinitializer
@__scoop_refactor_frame_layout__fixtures_build_main = internal constant %scoop.refactor.Frame__fixtures_build_main zeroinitializer
@__scoop_refactor_continuation_layout__fixtures_build_main = internal constant %scoop.refactor.Continuation__fixtures_build_main zeroinitializer
@__scoop_refactor_frame_layout__fixtures_build_main__type_desc__trace_bitmap = internal constant [1 x i64] [i64 33]
@__scoop_refactor_frame_layout__fixtures_build_main__type_desc = internal constant %scoop.runtime.ScoopTypeDescriptor { i32 0, i32 0, i64 96, i64 8, i64 32, i32 1, i32 0, ptr @__scoop_refactor_frame_layout__fixtures_build_main__type_desc__trace_bitmap, ptr null, ptr null, i64 -8046005298092833786, ptr null, ptr null, ptr null }
@__scoop_type_desc_runtime__ScoopString = internal constant %scoop.runtime.ScoopTypeDescriptor { i32 0, i32 0, i64 48, i64 8, i64 32, i32 0, i32 0, ptr null, ptr null, ptr null, i64 -1303988992855010267, ptr null, ptr null, ptr null }
@__scoop_str_data_795_801 = constant [4 x i8] c"none"
@__scoop_refactor_continuation_layout__fixtures_build_main__type_desc__trace_bitmap = internal constant [1 x i64] [i64 5]
@__scoop_refactor_continuation_layout__fixtures_build_main__type_desc = internal constant %scoop.runtime.ScoopTypeDescriptor { i32 0, i32 0, i64 72, i64 8, i64 32, i32 1, i32 0, ptr @__scoop_refactor_continuation_layout__fixtures_build_main__type_desc__trace_bitmap, ptr null, ptr null, i64 5067225366741973987, ptr null, ptr null, ptr null }
@__scoop_str_data_913_917 = constant [2 x i8] c"ok"
@__scoop_str_data_925_928 = constant [1 x i8] c"!"
@__scoop_explicit_root_offsets__fixtures_build_main = internal constant [12 x i32] [i32 16, i32 24, i32 32, i32 40, i32 48, i32 56, i32 64, i32 72, i32 80, i32 88, i32 96, i32 104]
@__scoop_explicit_root_desc__fixtures_build_main = internal constant %scoop.runtime.ScoopRootFrameDesc { i32 12, ptr @__scoop_explicit_root_offsets__fixtures_build_main }
@__scoop_explicit_root_frame_top = external thread_local global ptr
@__scoop_explicit_root_offsets__scoop_core_println___Int_ = internal constant [2 x i32] [i32 16, i32 24]
@__scoop_explicit_root_desc__scoop_core_println___Int_ = internal constant %scoop.runtime.ScoopRootFrameDesc { i32 2, ptr @__scoop_explicit_root_offsets__scoop_core_println___Int_ }
@__scoop_explicit_root_offsets__scoop_core_println___String_ = internal constant [3 x i32] [i32 16, i32 24, i32 32]
@__scoop_explicit_root_desc__scoop_core_println___String_ = internal constant %scoop.runtime.ScoopRootFrameDesc { i32 3, ptr @__scoop_explicit_root_offsets__scoop_core_println___String_ }
@__scoop_explicit_root_offsets____scoop_refactor_surface_resume_owner_dispatch__fixtures_build_main__k0 = internal constant [14 x i32] [i32 16, i32 24, i32 32, i32 40, i32 48, i32 56, i32 64, i32 72, i32 80, i32 88, i32 96, i32 104, i32 112, i32 120]
@__scoop_explicit_root_desc____scoop_refactor_surface_resume_owner_dispatch__fixtures_build_main__k0 = internal constant %scoop.runtime.ScoopRootFrameDesc { i32 14, ptr @__scoop_explicit_root_offsets____scoop_refactor_surface_resume_owner_dispatch__fixtures_build_main__k0 }
@__scoop_explicit_root_desc__main = internal constant %scoop.runtime.ScoopRootFrameDesc zeroinitializer

define %scoop.refactor.Step__fixtures_build_main @__scoop_refactor_resume__fixtures_build_main__case0(ptr addrspace(1) %0, i64 %1) {
entry:
  unreachable
}

define %scoop.refactor.Step__fixtures_build_main @__scoop_refactor_resume__fixtures_build_main__case1(ptr addrspace(1) %0, i8 %1) {
entry:
  unreachable
}

define %scoop.refactor.Step__fixtures_build_main @__scoop_refactor_surface_resume__k0(ptr addrspace(1) %0, i64 %1) {
entry:
  %refactor_surface_resume_call = call %scoop.refactor.Step__fixtures_build_main @__scoop_refactor_surface_resume_owner_dispatch__fixtures_build_main__k0(ptr addrspace(1) %0, i64 %1)
  ret %scoop.refactor.Step__fixtures_build_main %refactor_surface_resume_call
}

define %scoop.refactor.Step__fixtures_build_main @__scoop_refactor_surface_resume__k1(ptr addrspace(1) %0, i8 %1) {
entry:
  unreachable
}

define i64 @fixtures.build.main() {
entry:
  %explicit_root_frame_storage = alloca ptr, i32 14, align 8
  %explicit_root_frame_prev_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_desc_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 1
  %explicit_root_frame_prev = load ptr, ptr @__scoop_explicit_root_frame_top, align 8
  store ptr %explicit_root_frame_prev, ptr %explicit_root_frame_prev_ptr, align 8
  store ptr @__scoop_explicit_root_desc__fixtures_build_main, ptr %explicit_root_frame_desc_ptr, align 8
  %explicit_root_frame_init_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_0, align 8
  %explicit_root_frame_init_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_1, align 8
  %explicit_root_frame_init_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_2, align 8
  %explicit_root_frame_init_slot_3 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_3, align 8
  %explicit_root_frame_init_slot_4 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_4, align 8
  %explicit_root_frame_init_slot_5 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_5, align 8
  %explicit_root_frame_init_slot_6 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_6, align 8
  %explicit_root_frame_init_slot_7 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_7, align 8
  %explicit_root_frame_init_slot_8 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_8, align 8
  %explicit_root_frame_init_slot_9 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_9, align 8
  %explicit_root_frame_init_slot_10 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_10, align 8
  %explicit_root_frame_init_slot_11 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_11, align 8
  store ptr %explicit_root_frame_storage, ptr @__scoop_explicit_root_frame_top, align 8
  %explicit_root_frame_slot_11 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  %explicit_root_frame_slot_10 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %explicit_root_frame_slot_9 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %explicit_root_frame_slot_8 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  %explicit_root_frame_slot_7 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  %explicit_root_frame_slot_6 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  %explicit_root_frame_slot_5 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  %explicit_root_frame_slot_4 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  %explicit_root_frame_slot_3 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  %explicit_root_frame_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %explicit_root_frame_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %explicit_root_frame_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_9, align 8
  %tracked_explicit_gc_root_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %gc_root_keepalive_4294967295 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0, align 8
  %rt_alloc_refactor_frame = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_frame_layout__fixtures_build_main__type_desc, i64 96)
  %gc_root_keepalive_reload = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0, align 8
  %refactor_frame_zero_field_1 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_frame, i32 0, i32 1
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_frame_zero_field_1, align 8
  %refactor_frame_zero_field_2 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_frame, i32 0, i32 2
  store i64 0, ptr addrspace(1) %refactor_frame_zero_field_2, align 8
  %refactor_frame_zero_field_3 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_frame, i32 0, i32 3
  store i64 0, ptr addrspace(1) %refactor_frame_zero_field_3, align 8
  %refactor_frame_zero_field_4 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_frame, i32 0, i32 4
  store i8 0, ptr addrspace(1) %refactor_frame_zero_field_4, align 1
  %refactor_frame_zero_field_5 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_frame, i32 0, i32 5
  store i64 0, ptr addrspace(1) %refactor_frame_zero_field_5, align 8
  %refactor_frame_zero_field_6 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_frame, i32 0, i32 6
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_frame_zero_field_6, align 8
  %refactor_frame_zero_field_7 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_frame, i32 0, i32 7
  store i1 false, ptr addrspace(1) %refactor_frame_zero_field_7, align 1
  %refactor_frame_zero_field_8 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_frame, i32 0, i32 8
  store i1 false, ptr addrspace(1) %refactor_frame_zero_field_8, align 1
  %refactor_frame_zero_field_9 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_frame, i32 0, i32 9
  store i64 0, ptr addrspace(1) %refactor_frame_zero_field_9, align 8
  store ptr addrspace(1) %rt_alloc_refactor_frame, ptr %explicit_root_frame_slot_9, align 8
  br label %refactor.st0

return:                                           ; preds = %refactor.st4
  %explicit_root_frame_pop_prev_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev = load ptr, ptr %explicit_root_frame_pop_prev_ptr, align 8
  %explicit_root_frame_pop_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0, align 8
  %explicit_root_frame_pop_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1, align 8
  %explicit_root_frame_pop_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2, align 8
  %explicit_root_frame_pop_slot_3 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3, align 8
  %explicit_root_frame_pop_slot_4 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_4, align 8
  %explicit_root_frame_pop_slot_5 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_5, align 8
  %explicit_root_frame_pop_slot_6 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_6, align 8
  %explicit_root_frame_pop_slot_7 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_7, align 8
  %explicit_root_frame_pop_slot_8 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_8, align 8
  %explicit_root_frame_pop_slot_9 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_9, align 8
  %explicit_root_frame_pop_slot_10 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_10, align 8
  %explicit_root_frame_pop_slot_11 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_11, align 8
  store ptr %explicit_root_frame_pop_prev, ptr @__scoop_explicit_root_frame_top, align 8
  ret i64 0

refactor.st0:                                     ; preds = %entry
  %tracked_explicit_gc_root_slot_02 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %gc_root_keepalive_42949672953 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_02, align 8
  %rt_alloc_string_lit = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_type_desc_runtime__ScoopString, i64 48)
  %gc_root_keepalive_reload4 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_02, align 8
  %str_len_gep = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit, i32 0, i32 1
  %str_data_gep = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit, i32 0, i32 2
  store i64 4, ptr addrspace(1) %str_len_gep, align 8
  store ptr @__scoop_str_data_795_801, ptr addrspace(1) %str_data_gep, align 8
  store ptr addrspace(1) %rt_alloc_string_lit, ptr %explicit_root_frame_slot_1, align 8
  %pass_mir_load = load ptr addrspace(1), ptr %explicit_root_frame_slot_1, align 8
  store ptr addrspace(1) %pass_mir_load, ptr %explicit_root_frame_slot_0, align 8
  br label %refactor.st2

refactor.st1:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr79 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev80 = load ptr, ptr %explicit_root_frame_pop_prev_ptr79, align 8
  %explicit_root_frame_pop_slot_081 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_081, align 8
  %explicit_root_frame_pop_slot_182 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_182, align 8
  %explicit_root_frame_pop_slot_283 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_283, align 8
  %explicit_root_frame_pop_slot_384 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_384, align 8
  %explicit_root_frame_pop_slot_485 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_485, align 8
  %explicit_root_frame_pop_slot_586 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_586, align 8
  %explicit_root_frame_pop_slot_687 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_687, align 8
  %explicit_root_frame_pop_slot_788 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_788, align 8
  %explicit_root_frame_pop_slot_889 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_889, align 8
  %explicit_root_frame_pop_slot_990 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_990, align 8
  %explicit_root_frame_pop_slot_1091 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1091, align 8
  %explicit_root_frame_pop_slot_1192 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1192, align 8
  store ptr %explicit_root_frame_pop_prev80, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

refactor.st2:                                     ; preds = %refactor.st0
  %refactor_frame_slot_store_gep = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %gc_root_keepalive_reload4, i32 0, i32 1
  %pass_mir_load6 = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  %gc_wb_slot_addr = addrspacecast ptr addrspace(1) %refactor_frame_slot_store_gep to ptr
  %tracked_explicit_gc_root_slot_07 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %gc_root_keepalive_42949672958 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_07, align 8
  %gc_write_barrier = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr, ptr addrspace(1) %pass_mir_load6)
  %gc_root_keepalive_reload9 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_07, align 8
  %refactor_frame_slot_store_gep11 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %gc_root_keepalive_reload9, i32 0, i32 2
  store i64 undef, ptr addrspace(1) %refactor_frame_slot_store_gep11, align 8
  %tracked_explicit_gc_root_slot_013 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %gc_root_keepalive_429496729514 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_013, align 8
  %rt_alloc_refactor_cont = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_continuation_layout__fixtures_build_main__type_desc, i64 72)
  %gc_root_keepalive_reload15 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_013, align 8
  %refactor_cont_zero_field_1 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 1
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_1, align 8
  %refactor_cont_zero_field_2 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 2
  store i32 0, ptr addrspace(1) %refactor_cont_zero_field_2, align 4
  %refactor_cont_zero_field_3 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_zero_field_3, align 1
  %refactor_cont_zero_field_4 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 4
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_4, align 8
  %refactor_cont_zero_field_5 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 5
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_5, align 8
  %refactor_cont_zero_field_6 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 6
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_6, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_10, align 8
  store ptr addrspace(1) %rt_alloc_refactor_cont, ptr %explicit_root_frame_slot_10, align 8
  %refactor_cont_frame_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 1
  %gc_wb_slot_addr19 = addrspacecast ptr addrspace(1) %refactor_cont_frame_gep to ptr
  %tracked_explicit_gc_root_slot_020 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %gc_root_keepalive_4294967294 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1, align 8
  %gc_root_keepalive_429496729521 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_020, align 8
  %gc_write_barrier22 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr19, ptr addrspace(1) %gc_root_keepalive_reload15)
  %gc_root_keepalive_reload23 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1, align 8
  %gc_root_keepalive_reload24 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_020, align 8
  %refactor_cont_state_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 2
  store i32 5, ptr addrspace(1) %refactor_cont_state_gep, align 4
  %refactor_cont_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_one_shot_gep, align 1
  %refactor_cont_composed_callee_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 4
  %gc_wb_slot_addr25 = addrspacecast ptr addrspace(1) %refactor_cont_composed_callee_gep to ptr
  %tracked_explicit_gc_root_slot_026 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_127 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %gc_root_keepalive_429496729428 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_127, align 8
  %gc_root_keepalive_429496729529 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_026, align 8
  %gc_write_barrier30 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr25, ptr addrspace(1) null)
  %gc_root_keepalive_reload31 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_127, align 8
  %gc_root_keepalive_reload32 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_026, align 8
  store ptr addrspace(1) %rt_alloc_refactor_cont, ptr %explicit_root_frame_slot_2, align 8
  br label %refactor.st3

refactor.st3:                                     ; preds = %refactor.st2
  %tracked_explicit_gc_root_slot_033 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_134 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %gc_root_keepalive_429496729435 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_134, align 8
  %gc_root_keepalive_429496729536 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_033, align 8
  %rt_alloc_string_lit37 = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_type_desc_runtime__ScoopString, i64 48)
  %gc_root_keepalive_reload38 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_134, align 8
  %gc_root_keepalive_reload39 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_033, align 8
  %str_len_gep40 = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit37, i32 0, i32 1
  %str_data_gep41 = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit37, i32 0, i32 2
  store i64 2, ptr addrspace(1) %str_len_gep40, align 8
  store ptr @__scoop_str_data_913_917, ptr addrspace(1) %str_data_gep41, align 8
  store ptr addrspace(1) %rt_alloc_string_lit37, ptr %explicit_root_frame_slot_5, align 8
  %tracked_explicit_gc_root_slot_042 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_143 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %gc_root_keepalive_429496729444 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_143, align 8
  %gc_root_keepalive_429496729545 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_042, align 8
  %rt_alloc_string_lit46 = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_type_desc_runtime__ScoopString, i64 48)
  %gc_root_keepalive_reload47 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_143, align 8
  %gc_root_keepalive_reload48 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_042, align 8
  %str_len_gep49 = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit46, i32 0, i32 1
  %str_data_gep50 = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit46, i32 0, i32 2
  store i64 1, ptr addrspace(1) %str_len_gep49, align 8
  store ptr @__scoop_str_data_925_928, ptr addrspace(1) %str_data_gep50, align 8
  store ptr addrspace(1) %rt_alloc_string_lit46, ptr %explicit_root_frame_slot_6, align 8
  %pass_mir_load51 = load ptr addrspace(1), ptr %explicit_root_frame_slot_5, align 8
  %pass_mir_load52 = load ptr addrspace(1), ptr %explicit_root_frame_slot_6, align 8
  %tracked_explicit_gc_root_slot_053 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_154 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %gc_root_keepalive_429496729455 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_154, align 8
  %gc_root_keepalive_429496729556 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_053, align 8
  %refactor_string_concat = call ptr addrspace(1) @scoop_string_concat(ptr addrspace(1) %pass_mir_load51, ptr addrspace(1) %pass_mir_load52)
  %gc_root_keepalive_reload57 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_154, align 8
  %gc_root_keepalive_reload58 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_053, align 8
  store ptr addrspace(1) %refactor_string_concat, ptr %explicit_root_frame_slot_3, align 8
  %pass_mir_load59 = load ptr addrspace(1), ptr %explicit_root_frame_slot_3, align 8
  store ptr addrspace(1) %pass_mir_load59, ptr %explicit_root_frame_slot_0, align 8
  br label %refactor.st4

refactor.st4:                                     ; preds = %refactor.st5, %refactor.st3
  %tracked_explicit_gc_root_slot_063 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_164 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %gc_root_keepalive_429496729465 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_164, align 8
  %gc_root_keepalive_429496729566 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_063, align 8
  call void @"scoop.core.println::<Int>"(i64 0)
  %gc_root_keepalive_reload67 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_164, align 8
  %gc_root_keepalive_reload68 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_063, align 8
  %pass_mir_load69 = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  store ptr addrspace(1) %pass_mir_load69, ptr %explicit_root_frame_slot_11, align 8
  %pass_mir_call_arg_reload_0 = load ptr addrspace(1), ptr %explicit_root_frame_slot_11, align 8
  %tracked_explicit_gc_root_slot_070 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_171 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %tracked_explicit_gc_root_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  %gc_root_keepalive_4294967293 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2, align 8
  %gc_root_keepalive_429496729472 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_171, align 8
  %gc_root_keepalive_429496729573 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_070, align 8
  call void @"scoop.core.println::<String>"(ptr addrspace(1) %pass_mir_call_arg_reload_0)
  %gc_root_keepalive_reload74 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2, align 8
  %gc_root_keepalive_reload75 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_171, align 8
  %gc_root_keepalive_reload76 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_070, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_11, align 8
  br label %return

refactor.st5:                                     ; No predecessors!
  %tmp2.0.load108 = load i64, ptr poison, align 8
  store i64 %tmp2.0.load108, ptr poison, align 8
  br label %refactor.st4

refactor.st6:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr93 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev94 = load ptr, ptr %explicit_root_frame_pop_prev_ptr93, align 8
  %explicit_root_frame_pop_slot_095 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_095, align 8
  %explicit_root_frame_pop_slot_196 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_196, align 8
  %explicit_root_frame_pop_slot_297 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_297, align 8
  %explicit_root_frame_pop_slot_398 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_398, align 8
  %explicit_root_frame_pop_slot_499 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_499, align 8
  %explicit_root_frame_pop_slot_5100 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_5100, align 8
  %explicit_root_frame_pop_slot_6101 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_6101, align 8
  %explicit_root_frame_pop_slot_7102 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_7102, align 8
  %explicit_root_frame_pop_slot_8103 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_8103, align 8
  %explicit_root_frame_pop_slot_9104 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_9104, align 8
  %explicit_root_frame_pop_slot_10105 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_10105, align 8
  %explicit_root_frame_pop_slot_11106 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_11106, align 8
  store ptr %explicit_root_frame_pop_prev94, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable
}

define void @"scoop.core.println::<Int>"(i64 %0) {
entry:
  %explicit_root_frame_storage = alloca ptr, i32 4, align 8
  %explicit_root_frame_prev_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_desc_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 1
  %explicit_root_frame_prev = load ptr, ptr @__scoop_explicit_root_frame_top, align 8
  store ptr %explicit_root_frame_prev, ptr %explicit_root_frame_prev_ptr, align 8
  store ptr @__scoop_explicit_root_desc__scoop_core_println___Int_, ptr %explicit_root_frame_desc_ptr, align 8
  %explicit_root_frame_init_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_0, align 8
  %explicit_root_frame_init_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_1, align 8
  store ptr %explicit_root_frame_storage, ptr @__scoop_explicit_root_frame_top, align 8
  %explicit_root_frame_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %explicit_root_frame_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  br label %plain.bb0

return:                                           ; preds = %plain.bb0
  %explicit_root_frame_pop_prev_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev = load ptr, ptr %explicit_root_frame_pop_prev_ptr, align 8
  %explicit_root_frame_pop_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0, align 8
  %explicit_root_frame_pop_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1, align 8
  store ptr %explicit_root_frame_pop_prev, ptr @__scoop_explicit_root_frame_top, align 8
  ret void

plain.bb0:                                        ; preds = %entry
  %refactor_core_print_int_to_string = call ptr addrspace(1) @scoop_int_to_string(i64 %0)
  store ptr addrspace(1) %refactor_core_print_int_to_string, ptr %explicit_root_frame_slot_1, align 8
  %pass_mir_load1 = load ptr addrspace(1), ptr %explicit_root_frame_slot_1, align 8
  call void @scoop_println(ptr addrspace(1) %pass_mir_load1)
  br label %return
}

define void @"scoop.core.println::<String>"(ptr addrspace(1) %0) {
entry:
  %explicit_root_frame_storage = alloca ptr, i32 5, align 8
  %explicit_root_frame_prev_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_desc_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 1
  %explicit_root_frame_prev = load ptr, ptr @__scoop_explicit_root_frame_top, align 8
  store ptr %explicit_root_frame_prev, ptr %explicit_root_frame_prev_ptr, align 8
  store ptr @__scoop_explicit_root_desc__scoop_core_println___String_, ptr %explicit_root_frame_desc_ptr, align 8
  %explicit_root_frame_init_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_0, align 8
  %explicit_root_frame_init_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_1, align 8
  %explicit_root_frame_init_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_2, align 8
  store ptr %explicit_root_frame_storage, ptr @__scoop_explicit_root_frame_top, align 8
  %explicit_root_frame_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %explicit_root_frame_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %explicit_root_frame_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) %0, ptr %explicit_root_frame_slot_0, align 8
  br label %plain.bb0

return:                                           ; preds = %plain.bb0
  %explicit_root_frame_pop_prev_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev = load ptr, ptr %explicit_root_frame_pop_prev_ptr, align 8
  %explicit_root_frame_pop_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0, align 8
  %explicit_root_frame_pop_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1, align 8
  %explicit_root_frame_pop_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2, align 8
  store ptr %explicit_root_frame_pop_prev, ptr @__scoop_explicit_root_frame_top, align 8
  ret void

plain.bb0:                                        ; preds = %entry
  %pass_mir_load = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  store ptr addrspace(1) %pass_mir_load, ptr %explicit_root_frame_slot_2, align 8
  %pass_mir_load1 = load ptr addrspace(1), ptr %explicit_root_frame_slot_2, align 8
  call void @scoop_println(ptr addrspace(1) %pass_mir_load1)
  br label %return
}

define %scoop.refactor.Step__fixtures_build_main @__scoop_refactor_surface_resume_owner_dispatch__fixtures_build_main__k0(ptr addrspace(1) %0, i64 %1) {
entry:
  %explicit_root_frame_storage = alloca ptr, i32 16, align 8
  %explicit_root_frame_prev_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_desc_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 1
  %explicit_root_frame_prev = load ptr, ptr @__scoop_explicit_root_frame_top, align 8
  store ptr %explicit_root_frame_prev, ptr %explicit_root_frame_prev_ptr, align 8
  store ptr @__scoop_explicit_root_desc____scoop_refactor_surface_resume_owner_dispatch__fixtures_build_main__k0, ptr %explicit_root_frame_desc_ptr, align 8
  %explicit_root_frame_init_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_0, align 8
  %explicit_root_frame_init_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_1, align 8
  %explicit_root_frame_init_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_2, align 8
  %explicit_root_frame_init_slot_3 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_3, align 8
  %explicit_root_frame_init_slot_4 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_4, align 8
  %explicit_root_frame_init_slot_5 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_5, align 8
  %explicit_root_frame_init_slot_6 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_6, align 8
  %explicit_root_frame_init_slot_7 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_7, align 8
  %explicit_root_frame_init_slot_8 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_8, align 8
  %explicit_root_frame_init_slot_9 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_9, align 8
  %explicit_root_frame_init_slot_10 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_10, align 8
  %explicit_root_frame_init_slot_11 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_11, align 8
  %explicit_root_frame_init_slot_12 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 112
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_12, align 8
  %explicit_root_frame_init_slot_13 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 120
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_13, align 8
  store ptr %explicit_root_frame_storage, ptr @__scoop_explicit_root_frame_top, align 8
  %explicit_root_frame_slot_13 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 120
  %explicit_root_frame_slot_12 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 112
  %explicit_root_frame_slot_11 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  %explicit_root_frame_slot_10 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %explicit_root_frame_slot_9 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %explicit_root_frame_slot_8 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  %explicit_root_frame_slot_7 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  %explicit_root_frame_slot_6 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  %explicit_root_frame_slot_5 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  %explicit_root_frame_slot_4 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  %explicit_root_frame_slot_3 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  %explicit_root_frame_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %explicit_root_frame_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %explicit_root_frame_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_9, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_10, align 8
  store ptr addrspace(1) %0, ptr %explicit_root_frame_slot_10, align 8
  %refactor_load_frame_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %0, i32 0, i32 1
  %refactor_load_frame_gc = load ptr addrspace(1), ptr addrspace(1) %refactor_load_frame_gep, align 8
  store ptr addrspace(1) %refactor_load_frame_gc, ptr %explicit_root_frame_slot_9, align 8
  %refactor_frame_slot_load_gep = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_load_frame_gc, i32 0, i32 1
  %refactor_frame_slot_load = load ptr addrspace(1), ptr addrspace(1) %refactor_frame_slot_load_gep, align 8
  store ptr addrspace(1) %refactor_frame_slot_load, ptr %explicit_root_frame_slot_0, align 8
  %refactor_frame_slot_load_gep6 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_load_frame_gc, i32 0, i32 2
  %refactor_frame_slot_load7 = load i64, ptr addrspace(1) %refactor_frame_slot_load_gep6, align 8
  %refactor_resume_state_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %0, i32 0, i32 2
  %refactor_resume_state = load i32, ptr addrspace(1) %refactor_resume_state_gep, align 4
  %refactor_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %0, i32 0, i32 3
  %refactor_one_shot = load i1, ptr addrspace(1) %refactor_one_shot_gep, align 1
  br i1 %refactor_one_shot, label %resume_double, label %resume_first

refactor.st0:                                     ; No predecessors!
  %tracked_explicit_gc_root_slot_048 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_149 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %tracked_explicit_gc_root_slot_250 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  %gc_root_keepalive_429496729351 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_250, align 8
  %gc_root_keepalive_429496729452 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_149, align 8
  %gc_root_keepalive_429496729553 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_048, align 8
  %rt_alloc_string_lit = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_type_desc_runtime__ScoopString, i64 48)
  %gc_root_keepalive_reload54 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_250, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload54, ptr poison, align 8
  %gc_root_keepalive_reload55 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_149, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload55, ptr poison, align 8
  %gc_root_keepalive_reload56 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_048, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload56, ptr poison, align 8
  %str_len_gep = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit, i32 0, i32 1
  %str_data_gep = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit, i32 0, i32 2
  store i64 4, ptr addrspace(1) %str_len_gep, align 8
  store ptr @__scoop_str_data_795_801, ptr addrspace(1) %str_data_gep, align 8
  store ptr addrspace(1) %rt_alloc_string_lit, ptr %explicit_root_frame_slot_1, align 8
  %pass_mir_load57 = load ptr addrspace(1), ptr %explicit_root_frame_slot_1, align 8
  store ptr addrspace(1) %pass_mir_load57, ptr poison, align 8
  store ptr addrspace(1) %pass_mir_load57, ptr %explicit_root_frame_slot_0, align 8
  br label %refactor.st2

refactor.st1:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev = load ptr, ptr %explicit_root_frame_pop_prev_ptr, align 8
  %explicit_root_frame_pop_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0, align 8
  %explicit_root_frame_pop_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1, align 8
  %explicit_root_frame_pop_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2, align 8
  %explicit_root_frame_pop_slot_3 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3, align 8
  %explicit_root_frame_pop_slot_4 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_4, align 8
  %explicit_root_frame_pop_slot_5 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_5, align 8
  %explicit_root_frame_pop_slot_6 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_6, align 8
  %explicit_root_frame_pop_slot_7 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_7, align 8
  %explicit_root_frame_pop_slot_8 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_8, align 8
  %explicit_root_frame_pop_slot_9 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_9, align 8
  %explicit_root_frame_pop_slot_10 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_10, align 8
  %explicit_root_frame_pop_slot_11 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_11, align 8
  %explicit_root_frame_pop_slot_12 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 112
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_12, align 8
  %explicit_root_frame_pop_slot_13 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 120
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_13, align 8
  store ptr %explicit_root_frame_pop_prev, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

refactor.st2:                                     ; preds = %refactor.st0
  %refactor_frame_root.0.refactor_frame_root_reload58 = load ptr addrspace(1), ptr poison, align 8
  %refactor_frame_slot_store_gep59 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_frame_root.0.refactor_frame_root_reload58, i32 0, i32 1
  %pass_mir_load60 = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  %gc_wb_slot_addr61 = addrspacecast ptr addrspace(1) %refactor_frame_slot_store_gep59 to ptr
  %tracked_explicit_gc_root_slot_062 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_163 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %tracked_explicit_gc_root_slot_264 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  %gc_root_keepalive_429496729365 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_264, align 8
  %gc_root_keepalive_429496729466 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_163, align 8
  %gc_root_keepalive_429496729567 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_062, align 8
  %gc_write_barrier68 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr61, ptr addrspace(1) %pass_mir_load60)
  %gc_root_keepalive_reload69 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_264, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload69, ptr poison, align 8
  %gc_root_keepalive_reload70 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_163, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload70, ptr poison, align 8
  %gc_root_keepalive_reload71 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_062, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload71, ptr poison, align 8
  %refactor_frame_root.0.refactor_frame_root_reload72 = load ptr addrspace(1), ptr poison, align 8
  %refactor_frame_slot_store_gep73 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_frame_root.0.refactor_frame_root_reload72, i32 0, i32 2
  %tmp2.0.load400 = load i64, ptr poison, align 8
  store i64 %tmp2.0.load400, ptr addrspace(1) %refactor_frame_slot_store_gep73, align 8
  %tracked_explicit_gc_root_slot_075 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_176 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %tracked_explicit_gc_root_slot_277 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  %gc_root_keepalive_429496729378 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_277, align 8
  %gc_root_keepalive_429496729479 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_176, align 8
  %gc_root_keepalive_429496729580 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_075, align 8
  %rt_alloc_refactor_cont81 = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_continuation_layout__fixtures_build_main__type_desc, i64 72)
  %gc_root_keepalive_reload82 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_277, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload82, ptr poison, align 8
  %gc_root_keepalive_reload83 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_176, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload83, ptr poison, align 8
  %gc_root_keepalive_reload84 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_075, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload84, ptr poison, align 8
  %refactor_cont_zero_field_185 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont81, i32 0, i32 1
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_185, align 8
  %refactor_cont_zero_field_286 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont81, i32 0, i32 2
  store i32 0, ptr addrspace(1) %refactor_cont_zero_field_286, align 4
  %refactor_cont_zero_field_387 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont81, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_zero_field_387, align 1
  %refactor_cont_zero_field_488 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont81, i32 0, i32 4
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_488, align 8
  %refactor_cont_zero_field_589 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont81, i32 0, i32 5
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_589, align 8
  %refactor_cont_zero_field_690 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont81, i32 0, i32 6
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_690, align 8
  store ptr addrspace(1) null, ptr poison, align 8
  %refactor_cont_root91.0.refactor_cont_root_reload92 = load ptr addrspace(1), ptr poison, align 8
  store ptr addrspace(1) %refactor_cont_root91.0.refactor_cont_root_reload92, ptr %explicit_root_frame_slot_12, align 8
  store ptr addrspace(1) %rt_alloc_refactor_cont81, ptr poison, align 8
  %refactor_cont_root91.0.refactor_cont_root_reload93 = load ptr addrspace(1), ptr poison, align 8
  store ptr addrspace(1) %refactor_cont_root91.0.refactor_cont_root_reload93, ptr %explicit_root_frame_slot_12, align 8
  %refactor_cont_root91.0.refactor_cont_root_reload94 = load ptr addrspace(1), ptr poison, align 8
  %refactor_frame_root.0.refactor_frame_root_reload95 = load ptr addrspace(1), ptr poison, align 8
  %refactor_cont_frame_gep96 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %refactor_cont_root91.0.refactor_cont_root_reload94, i32 0, i32 1
  %gc_wb_slot_addr97 = addrspacecast ptr addrspace(1) %refactor_cont_frame_gep96 to ptr
  %tracked_explicit_gc_root_slot_098 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_199 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %tracked_explicit_gc_root_slot_2100 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  %tracked_explicit_gc_root_slot_3 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 112
  %gc_root_keepalive_4294967292 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3, align 8
  %gc_root_keepalive_4294967293101 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2100, align 8
  %gc_root_keepalive_4294967294102 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_199, align 8
  %gc_root_keepalive_4294967295103 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_098, align 8
  %gc_write_barrier104 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr97, ptr addrspace(1) %refactor_frame_root.0.refactor_frame_root_reload95)
  %gc_root_keepalive_reload105 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload105, ptr poison, align 8
  %gc_root_keepalive_reload106 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2100, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload106, ptr poison, align 8
  %gc_root_keepalive_reload107 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_199, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload107, ptr poison, align 8
  %gc_root_keepalive_reload108 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_098, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload108, ptr poison, align 8
  %refactor_cont_state_gep109 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %refactor_cont_root91.0.refactor_cont_root_reload94, i32 0, i32 2
  store i32 5, ptr addrspace(1) %refactor_cont_state_gep109, align 4
  %refactor_cont_one_shot_gep110 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %refactor_cont_root91.0.refactor_cont_root_reload94, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_one_shot_gep110, align 1
  %refactor_cont_composed_callee_gep111 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %refactor_cont_root91.0.refactor_cont_root_reload94, i32 0, i32 4
  %gc_wb_slot_addr112 = addrspacecast ptr addrspace(1) %refactor_cont_composed_callee_gep111 to ptr
  %tracked_explicit_gc_root_slot_0113 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_1114 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %tracked_explicit_gc_root_slot_2115 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  %tracked_explicit_gc_root_slot_3116 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 112
  %gc_root_keepalive_4294967292117 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3116, align 8
  %gc_root_keepalive_4294967293118 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2115, align 8
  %gc_root_keepalive_4294967294119 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1114, align 8
  %gc_root_keepalive_4294967295120 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0113, align 8
  %gc_write_barrier121 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr112, ptr addrspace(1) null)
  %gc_root_keepalive_reload122 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3116, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload122, ptr poison, align 8
  %gc_root_keepalive_reload123 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2115, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload123, ptr poison, align 8
  %gc_root_keepalive_reload124 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1114, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload124, ptr poison, align 8
  %gc_root_keepalive_reload125 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0113, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload125, ptr poison, align 8
  store ptr addrspace(1) %refactor_cont_root91.0.refactor_cont_root_reload94, ptr %explicit_root_frame_slot_2, align 8
  br label %refactor.st3

refactor.st3:                                     ; preds = %refactor.st2
  %tracked_explicit_gc_root_slot_0126 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_1127 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %tracked_explicit_gc_root_slot_2128 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  %tracked_explicit_gc_root_slot_3129 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 112
  %gc_root_keepalive_4294967292130 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3129, align 8
  %gc_root_keepalive_4294967293131 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2128, align 8
  %gc_root_keepalive_4294967294132 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1127, align 8
  %gc_root_keepalive_4294967295133 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0126, align 8
  %rt_alloc_string_lit134 = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_type_desc_runtime__ScoopString, i64 48)
  %gc_root_keepalive_reload135 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3129, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload135, ptr poison, align 8
  %gc_root_keepalive_reload136 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2128, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload136, ptr poison, align 8
  %gc_root_keepalive_reload137 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1127, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload137, ptr poison, align 8
  %gc_root_keepalive_reload138 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0126, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload138, ptr poison, align 8
  %str_len_gep139 = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit134, i32 0, i32 1
  %str_data_gep140 = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit134, i32 0, i32 2
  store i64 2, ptr addrspace(1) %str_len_gep139, align 8
  store ptr @__scoop_str_data_913_917, ptr addrspace(1) %str_data_gep140, align 8
  store ptr addrspace(1) %rt_alloc_string_lit134, ptr %explicit_root_frame_slot_5, align 8
  %tracked_explicit_gc_root_slot_0141 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_1142 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %tracked_explicit_gc_root_slot_2143 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  %tracked_explicit_gc_root_slot_3144 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 112
  %gc_root_keepalive_4294967292145 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3144, align 8
  %gc_root_keepalive_4294967293146 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2143, align 8
  %gc_root_keepalive_4294967294147 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1142, align 8
  %gc_root_keepalive_4294967295148 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0141, align 8
  %rt_alloc_string_lit149 = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_type_desc_runtime__ScoopString, i64 48)
  %gc_root_keepalive_reload150 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3144, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload150, ptr poison, align 8
  %gc_root_keepalive_reload151 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2143, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload151, ptr poison, align 8
  %gc_root_keepalive_reload152 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1142, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload152, ptr poison, align 8
  %gc_root_keepalive_reload153 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0141, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload153, ptr poison, align 8
  %str_len_gep154 = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit149, i32 0, i32 1
  %str_data_gep155 = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit149, i32 0, i32 2
  store i64 1, ptr addrspace(1) %str_len_gep154, align 8
  store ptr @__scoop_str_data_925_928, ptr addrspace(1) %str_data_gep155, align 8
  store ptr addrspace(1) %rt_alloc_string_lit149, ptr %explicit_root_frame_slot_6, align 8
  %pass_mir_load156 = load ptr addrspace(1), ptr %explicit_root_frame_slot_5, align 8
  %pass_mir_load157 = load ptr addrspace(1), ptr %explicit_root_frame_slot_6, align 8
  %tracked_explicit_gc_root_slot_0158 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_1159 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %tracked_explicit_gc_root_slot_2160 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  %tracked_explicit_gc_root_slot_3161 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 112
  %gc_root_keepalive_4294967292162 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3161, align 8
  %gc_root_keepalive_4294967293163 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2160, align 8
  %gc_root_keepalive_4294967294164 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1159, align 8
  %gc_root_keepalive_4294967295165 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0158, align 8
  %refactor_string_concat = call ptr addrspace(1) @scoop_string_concat(ptr addrspace(1) %pass_mir_load156, ptr addrspace(1) %pass_mir_load157)
  %gc_root_keepalive_reload166 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3161, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload166, ptr poison, align 8
  %gc_root_keepalive_reload167 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2160, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload167, ptr poison, align 8
  %gc_root_keepalive_reload168 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1159, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload168, ptr poison, align 8
  %gc_root_keepalive_reload169 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0158, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload169, ptr poison, align 8
  store ptr addrspace(1) %refactor_string_concat, ptr %explicit_root_frame_slot_3, align 8
  %pass_mir_load170 = load ptr addrspace(1), ptr %explicit_root_frame_slot_3, align 8
  store ptr addrspace(1) %pass_mir_load170, ptr poison, align 8
  store ptr addrspace(1) %pass_mir_load170, ptr %explicit_root_frame_slot_0, align 8
  store i64 0, ptr poison, align 8
  %tmp1.0.load = load i64, ptr poison, align 8
  %refactor_step_tmp173 = alloca %scoop.refactor.Step__fixtures_build_main, align 8
  store %scoop.refactor.Step__fixtures_build_main zeroinitializer, ptr %refactor_step_tmp173, align 8
  %refactor_step_tag_gep174 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp173, i32 0, i32 0
  store i32 0, ptr %refactor_step_tag_gep174, align 4
  %refactor_step_storage_gep175 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp173, i32 0, i32 1
  %refactor_step_payload_insert = insertvalue %scoop.refactor.StepComplete__fixtures_build_main undef, i64 %tmp1.0.load, 0
  store %scoop.refactor.StepComplete__fixtures_build_main %refactor_step_payload_insert, ptr %refactor_step_storage_gep175, align 8
  %refactor_step176 = load %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp173, align 8
  %refactor_frame_root.0.refactor_frame_root_reload177 = load ptr addrspace(1), ptr poison, align 8
  %refactor_frame_slot_store_gep178 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_frame_root.0.refactor_frame_root_reload177, i32 0, i32 1
  %pass_mir_load179 = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  %gc_wb_slot_addr180 = addrspacecast ptr addrspace(1) %refactor_frame_slot_store_gep178 to ptr
  %tracked_explicit_gc_root_slot_0181 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_1182 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %tracked_explicit_gc_root_slot_2183 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  %tracked_explicit_gc_root_slot_3184 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 112
  %gc_root_keepalive_4294967292185 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3184, align 8
  %gc_root_keepalive_4294967293186 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2183, align 8
  %gc_root_keepalive_4294967294187 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1182, align 8
  %gc_root_keepalive_4294967295188 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0181, align 8
  %gc_write_barrier189 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr180, ptr addrspace(1) %pass_mir_load179)
  %gc_root_keepalive_reload190 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3184, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload190, ptr poison, align 8
  %gc_root_keepalive_reload191 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2183, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload191, ptr poison, align 8
  %gc_root_keepalive_reload192 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1182, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload192, ptr poison, align 8
  %gc_root_keepalive_reload193 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0181, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload193, ptr poison, align 8
  %refactor_frame_root.0.refactor_frame_root_reload194 = load ptr addrspace(1), ptr poison, align 8
  %refactor_frame_slot_store_gep195 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_frame_root.0.refactor_frame_root_reload194, i32 0, i32 2
  %tmp2.0.load401 = load i64, ptr poison, align 8
  store i64 %tmp2.0.load401, ptr addrspace(1) %refactor_frame_slot_store_gep195, align 8
  %explicit_root_frame_pop_prev_ptr284 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev285 = load ptr, ptr %explicit_root_frame_pop_prev_ptr284, align 8
  %explicit_root_frame_pop_slot_0286 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0286, align 8
  %explicit_root_frame_pop_slot_1287 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1287, align 8
  %explicit_root_frame_pop_slot_2288 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2288, align 8
  %explicit_root_frame_pop_slot_3289 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3289, align 8
  %explicit_root_frame_pop_slot_4290 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_4290, align 8
  %explicit_root_frame_pop_slot_5291 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_5291, align 8
  %explicit_root_frame_pop_slot_6292 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_6292, align 8
  %explicit_root_frame_pop_slot_7293 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_7293, align 8
  %explicit_root_frame_pop_slot_8294 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_8294, align 8
  %explicit_root_frame_pop_slot_9295 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_9295, align 8
  %explicit_root_frame_pop_slot_10296 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_10296, align 8
  %explicit_root_frame_pop_slot_11297 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_11297, align 8
  %explicit_root_frame_pop_slot_12298 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 112
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_12298, align 8
  %explicit_root_frame_pop_slot_13299 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 120
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_13299, align 8
  store ptr %explicit_root_frame_pop_prev285, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_main %refactor_step176

refactor.st4:                                     ; No predecessors!
  %tmp1.0.load397 = load i64, ptr poison, align 8
  %tracked_explicit_gc_root_slot_0199 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_1200 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %tracked_explicit_gc_root_slot_2201 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  %tracked_explicit_gc_root_slot_3202 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 112
  %gc_root_keepalive_4294967292203 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3202, align 8
  %gc_root_keepalive_4294967293204 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2201, align 8
  %gc_root_keepalive_4294967294205 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1200, align 8
  %gc_root_keepalive_4294967295206 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0199, align 8
  call void @"scoop.core.println::<Int>"(i64 %tmp1.0.load397)
  %gc_root_keepalive_reload207 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3202, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload207, ptr poison, align 8
  %gc_root_keepalive_reload208 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2201, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload208, ptr poison, align 8
  %gc_root_keepalive_reload209 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1200, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload209, ptr poison, align 8
  %gc_root_keepalive_reload210 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0199, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload210, ptr poison, align 8
  %pass_mir_load211 = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  store ptr addrspace(1) %pass_mir_load211, ptr poison, align 8
  store ptr addrspace(1) %pass_mir_load211, ptr %explicit_root_frame_slot_13, align 8
  %pass_mir_call_arg_reload_0 = load ptr addrspace(1), ptr %explicit_root_frame_slot_13, align 8
  %tracked_explicit_gc_root_slot_0212 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_1213 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %tracked_explicit_gc_root_slot_2214 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  %tracked_explicit_gc_root_slot_3215 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 112
  %tracked_explicit_gc_root_slot_4 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 120
  %gc_root_keepalive_4294967291 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_4, align 8
  %gc_root_keepalive_4294967292216 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3215, align 8
  %gc_root_keepalive_4294967293217 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2214, align 8
  %gc_root_keepalive_4294967294218 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1213, align 8
  %gc_root_keepalive_4294967295219 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0212, align 8
  call void @"scoop.core.println::<String>"(ptr addrspace(1) %pass_mir_call_arg_reload_0)
  %gc_root_keepalive_reload220 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_4, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload220, ptr poison, align 8
  %gc_root_keepalive_reload221 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3215, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload221, ptr poison, align 8
  %gc_root_keepalive_reload222 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2214, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload222, ptr poison, align 8
  %gc_root_keepalive_reload223 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1213, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload223, ptr poison, align 8
  %gc_root_keepalive_reload224 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0212, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload224, ptr poison, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_13, align 8
  %refactor_step_tmp226 = alloca %scoop.refactor.Step__fixtures_build_main, align 8
  store %scoop.refactor.Step__fixtures_build_main zeroinitializer, ptr %refactor_step_tmp226, align 8
  %refactor_step_tag_gep227 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp226, i32 0, i32 0
  store i32 0, ptr %refactor_step_tag_gep227, align 4
  %refactor_step_storage_gep228 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp226, i32 0, i32 1
  %refactor_step_payload_insert229 = insertvalue %scoop.refactor.StepComplete__fixtures_build_main undef, i64 %tmp1.0.load397, 0
  store %scoop.refactor.StepComplete__fixtures_build_main %refactor_step_payload_insert229, ptr %refactor_step_storage_gep228, align 8
  %refactor_step230 = load %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp226, align 8
  %refactor_frame_root.0.refactor_frame_root_reload231 = load ptr addrspace(1), ptr poison, align 8
  %refactor_frame_slot_store_gep232 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_frame_root.0.refactor_frame_root_reload231, i32 0, i32 1
  %pass_mir_load233 = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  %gc_wb_slot_addr234 = addrspacecast ptr addrspace(1) %refactor_frame_slot_store_gep232 to ptr
  %tracked_explicit_gc_root_slot_0235 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_1236 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %tracked_explicit_gc_root_slot_2237 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  %tracked_explicit_gc_root_slot_3238 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 112
  %tracked_explicit_gc_root_slot_4239 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 120
  %gc_root_keepalive_4294967291240 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_4239, align 8
  %gc_root_keepalive_4294967292241 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3238, align 8
  %gc_root_keepalive_4294967293242 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2237, align 8
  %gc_root_keepalive_4294967294243 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1236, align 8
  %gc_root_keepalive_4294967295244 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0235, align 8
  %gc_write_barrier245 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr234, ptr addrspace(1) %pass_mir_load233)
  %gc_root_keepalive_reload246 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_4239, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload246, ptr poison, align 8
  %gc_root_keepalive_reload247 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3238, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload247, ptr poison, align 8
  %gc_root_keepalive_reload248 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2237, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload248, ptr poison, align 8
  %gc_root_keepalive_reload249 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1236, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload249, ptr poison, align 8
  %gc_root_keepalive_reload250 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0235, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload250, ptr poison, align 8
  %refactor_frame_root.0.refactor_frame_root_reload251 = load ptr addrspace(1), ptr poison, align 8
  %refactor_frame_slot_store_gep252 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_frame_root.0.refactor_frame_root_reload251, i32 0, i32 2
  %tmp2.0.load402 = load i64, ptr poison, align 8
  store i64 %tmp2.0.load402, ptr addrspace(1) %refactor_frame_slot_store_gep252, align 8
  %explicit_root_frame_pop_prev_ptr300 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev301 = load ptr, ptr %explicit_root_frame_pop_prev_ptr300, align 8
  %explicit_root_frame_pop_slot_0302 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0302, align 8
  %explicit_root_frame_pop_slot_1303 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1303, align 8
  %explicit_root_frame_pop_slot_2304 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2304, align 8
  %explicit_root_frame_pop_slot_3305 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3305, align 8
  %explicit_root_frame_pop_slot_4306 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_4306, align 8
  %explicit_root_frame_pop_slot_5307 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_5307, align 8
  %explicit_root_frame_pop_slot_6308 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_6308, align 8
  %explicit_root_frame_pop_slot_7309 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_7309, align 8
  %explicit_root_frame_pop_slot_8310 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_8310, align 8
  %explicit_root_frame_pop_slot_9311 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_9311, align 8
  %explicit_root_frame_pop_slot_10312 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_10312, align 8
  %explicit_root_frame_pop_slot_11313 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_11313, align 8
  %explicit_root_frame_pop_slot_12314 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 112
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_12314, align 8
  %explicit_root_frame_pop_slot_13315 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 120
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_13315, align 8
  store ptr %explicit_root_frame_pop_prev301, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_main %refactor_step230

refactor.st5:                                     ; preds = %resume_payload_st5
  %refactor_step_tmp256 = alloca %scoop.refactor.Step__fixtures_build_main, align 8
  store %scoop.refactor.Step__fixtures_build_main zeroinitializer, ptr %refactor_step_tmp256, align 8
  %refactor_step_tag_gep257 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp256, i32 0, i32 0
  store i32 0, ptr %refactor_step_tag_gep257, align 4
  %refactor_step_storage_gep258 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp256, i32 0, i32 1
  %refactor_step_payload_insert259 = insertvalue %scoop.refactor.StepComplete__fixtures_build_main undef, i64 %1, 0
  store %scoop.refactor.StepComplete__fixtures_build_main %refactor_step_payload_insert259, ptr %refactor_step_storage_gep258, align 8
  %refactor_step260 = load %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp256, align 8
  %refactor_frame_slot_store_gep262 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_load_frame_gc, i32 0, i32 1
  %pass_mir_load263 = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  %gc_wb_slot_addr264 = addrspacecast ptr addrspace(1) %refactor_frame_slot_store_gep262 to ptr
  %tracked_explicit_gc_root_slot_0265 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_1266 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %tracked_explicit_gc_root_slot_2267 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  %tracked_explicit_gc_root_slot_3268 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 112
  %tracked_explicit_gc_root_slot_4269 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 120
  %gc_root_keepalive_4294967291270 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_4269, align 8
  %gc_root_keepalive_4294967292271 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3268, align 8
  %gc_root_keepalive_4294967293272 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2267, align 8
  %gc_root_keepalive_4294967294273 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1266, align 8
  %gc_root_keepalive_4294967295274 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0265, align 8
  %gc_write_barrier275 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr264, ptr addrspace(1) %pass_mir_load263)
  %gc_root_keepalive_reload276 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_4269, align 8
  %gc_root_keepalive_reload277 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3268, align 8
  %gc_root_keepalive_reload278 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2267, align 8
  %gc_root_keepalive_reload279 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1266, align 8
  %gc_root_keepalive_reload280 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0265, align 8
  %refactor_frame_slot_store_gep282 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %gc_root_keepalive_reload280, i32 0, i32 2
  store i64 %1, ptr addrspace(1) %refactor_frame_slot_store_gep282, align 8
  %explicit_root_frame_pop_prev_ptr316 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev317 = load ptr, ptr %explicit_root_frame_pop_prev_ptr316, align 8
  %explicit_root_frame_pop_slot_0318 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0318, align 8
  %explicit_root_frame_pop_slot_1319 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1319, align 8
  %explicit_root_frame_pop_slot_2320 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2320, align 8
  %explicit_root_frame_pop_slot_3321 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3321, align 8
  %explicit_root_frame_pop_slot_4322 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_4322, align 8
  %explicit_root_frame_pop_slot_5323 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_5323, align 8
  %explicit_root_frame_pop_slot_6324 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_6324, align 8
  %explicit_root_frame_pop_slot_7325 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_7325, align 8
  %explicit_root_frame_pop_slot_8326 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_8326, align 8
  %explicit_root_frame_pop_slot_9327 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_9327, align 8
  %explicit_root_frame_pop_slot_10328 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_10328, align 8
  %explicit_root_frame_pop_slot_11329 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_11329, align 8
  %explicit_root_frame_pop_slot_12330 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 112
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_12330, align 8
  %explicit_root_frame_pop_slot_13331 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 120
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_13331, align 8
  store ptr %explicit_root_frame_pop_prev317, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_main %refactor_step260

refactor.st6:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr332 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev333 = load ptr, ptr %explicit_root_frame_pop_prev_ptr332, align 8
  %explicit_root_frame_pop_slot_0334 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0334, align 8
  %explicit_root_frame_pop_slot_1335 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1335, align 8
  %explicit_root_frame_pop_slot_2336 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2336, align 8
  %explicit_root_frame_pop_slot_3337 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3337, align 8
  %explicit_root_frame_pop_slot_4338 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_4338, align 8
  %explicit_root_frame_pop_slot_5339 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_5339, align 8
  %explicit_root_frame_pop_slot_6340 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_6340, align 8
  %explicit_root_frame_pop_slot_7341 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_7341, align 8
  %explicit_root_frame_pop_slot_8342 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_8342, align 8
  %explicit_root_frame_pop_slot_9343 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_9343, align 8
  %explicit_root_frame_pop_slot_10344 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_10344, align 8
  %explicit_root_frame_pop_slot_11345 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_11345, align 8
  %explicit_root_frame_pop_slot_12346 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 112
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_12346, align 8
  %explicit_root_frame_pop_slot_13347 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 120
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_13347, align 8
  store ptr %explicit_root_frame_pop_prev333, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_first:                                     ; preds = %entry
  %refactor_store_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %0, i32 0, i32 3
  store i1 true, ptr addrspace(1) %refactor_store_one_shot_gep, align 1
  %refactor_composed_callee_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %0, i32 0, i32 4
  %refactor_composed_callee = load ptr addrspace(1), ptr addrspace(1) %refactor_composed_callee_gep, align 8
  %refactor_composed_callee_is_null = icmp eq ptr addrspace(1) %refactor_composed_callee, null
  br i1 %refactor_composed_callee_is_null, label %resume_plain_dispatch, label %resume_composed_dispatch

resume_double:                                    ; preds = %entry
  %tracked_explicit_gc_root_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %gc_root_keepalive_4294967294 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1, align 8
  %gc_root_keepalive_4294967295 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0, align 8
  %rt_alloc_refactor_cont = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_continuation_layout__fixtures_build_main__type_desc, i64 72)
  %gc_root_keepalive_reload = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1, align 8
  %gc_root_keepalive_reload8 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0, align 8
  %refactor_cont_zero_field_1 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 1
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_1, align 8
  %refactor_cont_zero_field_2 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 2
  store i32 0, ptr addrspace(1) %refactor_cont_zero_field_2, align 4
  %refactor_cont_zero_field_3 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_zero_field_3, align 1
  %refactor_cont_zero_field_4 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 4
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_4, align 8
  %refactor_cont_zero_field_5 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 5
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_5, align 8
  %refactor_cont_zero_field_6 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 6
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_6, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_11, align 8
  store ptr addrspace(1) %rt_alloc_refactor_cont, ptr %explicit_root_frame_slot_11, align 8
  %refactor_cont_frame_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 1
  %gc_wb_slot_addr = addrspacecast ptr addrspace(1) %refactor_cont_frame_gep to ptr
  %tracked_explicit_gc_root_slot_012 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_113 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %tracked_explicit_gc_root_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  %gc_root_keepalive_4294967293 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2, align 8
  %gc_root_keepalive_429496729414 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_113, align 8
  %gc_root_keepalive_429496729515 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_012, align 8
  %gc_write_barrier = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr, ptr addrspace(1) %gc_root_keepalive_reload8)
  %gc_root_keepalive_reload16 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2, align 8
  %gc_root_keepalive_reload17 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_113, align 8
  %gc_root_keepalive_reload18 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_012, align 8
  %refactor_cont_state_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 2
  store i32 %refactor_resume_state, ptr addrspace(1) %refactor_cont_state_gep, align 4
  %refactor_cont_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_one_shot_gep, align 1
  %refactor_cont_composed_callee_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 4
  %gc_wb_slot_addr19 = addrspacecast ptr addrspace(1) %refactor_cont_composed_callee_gep to ptr
  %tracked_explicit_gc_root_slot_020 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_121 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %tracked_explicit_gc_root_slot_222 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  %gc_root_keepalive_429496729323 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_222, align 8
  %gc_root_keepalive_429496729424 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_121, align 8
  %gc_root_keepalive_429496729525 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_020, align 8
  %gc_write_barrier26 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr19, ptr addrspace(1) null)
  %gc_root_keepalive_reload27 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_222, align 8
  %gc_root_keepalive_reload28 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_121, align 8
  %gc_root_keepalive_reload29 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_020, align 8
  %refactor_step_tmp = alloca %scoop.refactor.Step__fixtures_build_main, align 8
  store %scoop.refactor.Step__fixtures_build_main zeroinitializer, ptr %refactor_step_tmp, align 8
  %refactor_step_tag_gep = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp, i32 0, i32 0
  store i32 2, ptr %refactor_step_tag_gep, align 4
  %refactor_step_storage_gep = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp, i32 0, i32 1
  %refactor_step_cont_insert = insertvalue %scoop.refactor.StepCase__fixtures_build_main__case1 { %scoop.core.RuntimeError { i32 2, i64 0, ptr addrspace(1) null }, ptr addrspace(1) undef }, ptr addrspace(1) %rt_alloc_refactor_cont, 1
  store %scoop.refactor.StepCase__fixtures_build_main__case1 %refactor_step_cont_insert, ptr %refactor_step_storage_gep, align 8
  %refactor_step = load %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp, align 8
  %refactor_frame_slot_store_gep = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %gc_root_keepalive_reload29, i32 0, i32 1
  %pass_mir_load = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  %gc_wb_slot_addr31 = addrspacecast ptr addrspace(1) %refactor_frame_slot_store_gep to ptr
  %tracked_explicit_gc_root_slot_032 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %tracked_explicit_gc_root_slot_133 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  %tracked_explicit_gc_root_slot_234 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  %gc_root_keepalive_429496729335 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_234, align 8
  %gc_root_keepalive_429496729436 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_133, align 8
  %gc_root_keepalive_429496729537 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_032, align 8
  %gc_write_barrier38 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr31, ptr addrspace(1) %pass_mir_load)
  %gc_root_keepalive_reload39 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_234, align 8
  %gc_root_keepalive_reload40 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_133, align 8
  %gc_root_keepalive_reload41 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_032, align 8
  %refactor_frame_slot_store_gep43 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %gc_root_keepalive_reload41, i32 0, i32 2
  store i64 %refactor_frame_slot_load7, ptr addrspace(1) %refactor_frame_slot_store_gep43, align 8
  %explicit_root_frame_pop_prev_ptr348 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev349 = load ptr, ptr %explicit_root_frame_pop_prev_ptr348, align 8
  %explicit_root_frame_pop_slot_0350 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0350, align 8
  %explicit_root_frame_pop_slot_1351 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1351, align 8
  %explicit_root_frame_pop_slot_2352 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2352, align 8
  %explicit_root_frame_pop_slot_3353 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3353, align 8
  %explicit_root_frame_pop_slot_4354 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_4354, align 8
  %explicit_root_frame_pop_slot_5355 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_5355, align 8
  %explicit_root_frame_pop_slot_6356 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_6356, align 8
  %explicit_root_frame_pop_slot_7357 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_7357, align 8
  %explicit_root_frame_pop_slot_8358 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_8358, align 8
  %explicit_root_frame_pop_slot_9359 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_9359, align 8
  %explicit_root_frame_pop_slot_10360 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_10360, align 8
  %explicit_root_frame_pop_slot_11361 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_11361, align 8
  %explicit_root_frame_pop_slot_12362 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 112
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_12362, align 8
  %explicit_root_frame_pop_slot_13363 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 120
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_13363, align 8
  store ptr %explicit_root_frame_pop_prev349, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_main %refactor_step

resume_plain_dispatch:                            ; preds = %resume_first
  switch i32 %refactor_resume_state, label %resume_invalid_state [
    i32 5, label %resume_payload_st5
  ]

resume_composed_dispatch:                         ; preds = %resume_first
  %explicit_root_frame_pop_prev_ptr364 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev365 = load ptr, ptr %explicit_root_frame_pop_prev_ptr364, align 8
  %explicit_root_frame_pop_slot_0366 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0366, align 8
  %explicit_root_frame_pop_slot_1367 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1367, align 8
  %explicit_root_frame_pop_slot_2368 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2368, align 8
  %explicit_root_frame_pop_slot_3369 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3369, align 8
  %explicit_root_frame_pop_slot_4370 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_4370, align 8
  %explicit_root_frame_pop_slot_5371 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_5371, align 8
  %explicit_root_frame_pop_slot_6372 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_6372, align 8
  %explicit_root_frame_pop_slot_7373 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_7373, align 8
  %explicit_root_frame_pop_slot_8374 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_8374, align 8
  %explicit_root_frame_pop_slot_9375 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_9375, align 8
  %explicit_root_frame_pop_slot_10376 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_10376, align 8
  %explicit_root_frame_pop_slot_11377 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_11377, align 8
  %explicit_root_frame_pop_slot_12378 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 112
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_12378, align 8
  %explicit_root_frame_pop_slot_13379 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 120
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_13379, align 8
  store ptr %explicit_root_frame_pop_prev365, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_invalid_state:                             ; preds = %resume_plain_dispatch
  %explicit_root_frame_pop_prev_ptr380 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev381 = load ptr, ptr %explicit_root_frame_pop_prev_ptr380, align 8
  %explicit_root_frame_pop_slot_0382 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0382, align 8
  %explicit_root_frame_pop_slot_1383 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1383, align 8
  %explicit_root_frame_pop_slot_2384 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2384, align 8
  %explicit_root_frame_pop_slot_3385 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3385, align 8
  %explicit_root_frame_pop_slot_4386 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_4386, align 8
  %explicit_root_frame_pop_slot_5387 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_5387, align 8
  %explicit_root_frame_pop_slot_6388 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_6388, align 8
  %explicit_root_frame_pop_slot_7389 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_7389, align 8
  %explicit_root_frame_pop_slot_8390 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_8390, align 8
  %explicit_root_frame_pop_slot_9391 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_9391, align 8
  %explicit_root_frame_pop_slot_10392 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_10392, align 8
  %explicit_root_frame_pop_slot_11393 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_11393, align 8
  %explicit_root_frame_pop_slot_12394 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 112
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_12394, align 8
  %explicit_root_frame_pop_slot_13395 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 120
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_13395, align 8
  store ptr %explicit_root_frame_pop_prev381, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_payload_st5:                               ; preds = %resume_plain_dispatch
  %refactor_frame_slot_store_gep46 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_load_frame_gc, i32 0, i32 2
  store i64 %1, ptr addrspace(1) %refactor_frame_slot_store_gep46, align 8
  br label %refactor.st5
}

declare ptr addrspace(1) @scoop_alloc_typed(ptr, i64)

declare ptr addrspace(1) @scoop_gc_write_barrier(ptr, ptr addrspace(1))

declare ptr addrspace(1) @scoop_string_concat(ptr addrspace(1), ptr addrspace(1))

declare ptr addrspace(1) @scoop_int_to_string(i64)

declare void @scoop_println(ptr addrspace(1))

define i32 @main(i32 %argc, ptr %argv) {
entry:
  %explicit_root_frame_storage = alloca ptr, i32 2, align 8
  %explicit_root_frame_prev_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_desc_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 1
  %explicit_root_frame_prev = load ptr, ptr @__scoop_explicit_root_frame_top, align 8
  store ptr %explicit_root_frame_prev, ptr %explicit_root_frame_prev_ptr, align 8
  store ptr @__scoop_explicit_root_desc__main, ptr %explicit_root_frame_desc_ptr, align 8
  store ptr %explicit_root_frame_storage, ptr @__scoop_explicit_root_frame_top, align 8
  call void @scoop_runtime_init()
  %refactor_plain_main = call i64 @fixtures.build.main()
  %refactor_plain_main_exit_i32 = trunc i64 %refactor_plain_main to i32
  %explicit_root_frame_pop_prev_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev = load ptr, ptr %explicit_root_frame_pop_prev_ptr, align 8
  store ptr %explicit_root_frame_pop_prev, ptr @__scoop_explicit_root_frame_top, align 8
  ret i32 %refactor_plain_main_exit_i32
}

declare void @scoop_runtime_init()
