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
@__scoop_explicit_root_offsets__fixtures_build_main = internal constant [10 x i32] [i32 16, i32 24, i32 32, i32 40, i32 48, i32 56, i32 64, i32 72, i32 80, i32 88]
@__scoop_explicit_root_desc__fixtures_build_main = internal constant %scoop.runtime.ScoopRootFrameDesc { i32 10, ptr @__scoop_explicit_root_offsets__fixtures_build_main }
@__scoop_explicit_root_frame_top = external thread_local global ptr
@__scoop_explicit_root_offsets__scoop_core_println___Int_ = internal constant [2 x i32] [i32 16, i32 24]
@__scoop_explicit_root_desc__scoop_core_println___Int_ = internal constant %scoop.runtime.ScoopRootFrameDesc { i32 2, ptr @__scoop_explicit_root_offsets__scoop_core_println___Int_ }
@__scoop_explicit_root_offsets__scoop_core_println___String_ = internal constant [3 x i32] [i32 16, i32 24, i32 32]
@__scoop_explicit_root_desc__scoop_core_println___String_ = internal constant %scoop.runtime.ScoopRootFrameDesc { i32 3, ptr @__scoop_explicit_root_offsets__scoop_core_println___String_ }
@__scoop_explicit_root_offsets____scoop_refactor_surface_resume_owner_dispatch__fixtures_build_main__k0 = internal constant [12 x i32] [i32 16, i32 24, i32 32, i32 40, i32 48, i32 56, i32 64, i32 72, i32 80, i32 88, i32 96, i32 104]
@__scoop_explicit_root_desc____scoop_refactor_surface_resume_owner_dispatch__fixtures_build_main__k0 = internal constant %scoop.runtime.ScoopRootFrameDesc { i32 12, ptr @__scoop_explicit_root_offsets____scoop_refactor_surface_resume_owner_dispatch__fixtures_build_main__k0 }
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
  %explicit_root_frame_storage = alloca ptr, i32 12, align 8
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
  store ptr %explicit_root_frame_storage, ptr @__scoop_explicit_root_frame_top, align 8
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
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_7, align 8
  %rt_alloc_refactor_frame = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_frame_layout__fixtures_build_main__type_desc, i64 96)
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
  store ptr addrspace(1) %rt_alloc_refactor_frame, ptr %explicit_root_frame_slot_7, align 8
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
  store ptr %explicit_root_frame_pop_prev, ptr @__scoop_explicit_root_frame_top, align 8
  ret i64 0

refactor.st0:                                     ; preds = %entry
  %rt_alloc_string_lit = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_type_desc_runtime__ScoopString, i64 48)
  %str_len_gep = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit, i32 0, i32 1
  %str_data_gep = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit, i32 0, i32 2
  store i64 4, ptr addrspace(1) %str_len_gep, align 8
  store ptr @__scoop_str_data_795_801, ptr addrspace(1) %str_data_gep, align 8
  store ptr addrspace(1) %rt_alloc_string_lit, ptr %explicit_root_frame_slot_1, align 8
  %pass_mir_load = load ptr addrspace(1), ptr %explicit_root_frame_slot_1, align 8
  store ptr addrspace(1) %pass_mir_load, ptr %explicit_root_frame_slot_0, align 8
  br label %refactor.st2

refactor.st1:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr26 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev27 = load ptr, ptr %explicit_root_frame_pop_prev_ptr26, align 8
  %explicit_root_frame_pop_slot_028 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_028, align 8
  %explicit_root_frame_pop_slot_129 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_129, align 8
  %explicit_root_frame_pop_slot_230 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_230, align 8
  %explicit_root_frame_pop_slot_331 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_331, align 8
  %explicit_root_frame_pop_slot_432 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_432, align 8
  %explicit_root_frame_pop_slot_533 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_533, align 8
  %explicit_root_frame_pop_slot_634 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_634, align 8
  %explicit_root_frame_pop_slot_735 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_735, align 8
  %explicit_root_frame_pop_slot_836 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_836, align 8
  %explicit_root_frame_pop_slot_937 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_937, align 8
  store ptr %explicit_root_frame_pop_prev27, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

refactor.st2:                                     ; preds = %refactor.st0
  %refactor_frame_root_reload = load ptr addrspace(1), ptr %explicit_root_frame_slot_7, align 8
  %refactor_frame_slot_store_gep = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_frame_root_reload, i32 0, i32 1
  %pass_mir_load1 = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  %gc_wb_slot_addr = addrspacecast ptr addrspace(1) %refactor_frame_slot_store_gep to ptr
  %gc_write_barrier = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr, ptr addrspace(1) %pass_mir_load1)
  %refactor_frame_root_reload2 = load ptr addrspace(1), ptr %explicit_root_frame_slot_7, align 8
  %refactor_frame_slot_store_gep3 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_frame_root_reload2, i32 0, i32 2
  store i64 undef, ptr addrspace(1) %refactor_frame_slot_store_gep3, align 8
  %rt_alloc_refactor_cont = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_continuation_layout__fixtures_build_main__type_desc, i64 72)
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
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_8, align 8
  store ptr addrspace(1) %rt_alloc_refactor_cont, ptr %explicit_root_frame_slot_8, align 8
  %refactor_cont_root_reload = load ptr addrspace(1), ptr %explicit_root_frame_slot_8, align 8
  %refactor_frame_root_reload5 = load ptr addrspace(1), ptr %explicit_root_frame_slot_7, align 8
  %refactor_cont_frame_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 1
  %gc_wb_slot_addr6 = addrspacecast ptr addrspace(1) %refactor_cont_frame_gep to ptr
  %gc_write_barrier7 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr6, ptr addrspace(1) %refactor_frame_root_reload5)
  %refactor_cont_state_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 2
  store i32 5, ptr addrspace(1) %refactor_cont_state_gep, align 4
  %refactor_cont_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_one_shot_gep, align 1
  %refactor_cont_composed_callee_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 4
  %gc_wb_slot_addr8 = addrspacecast ptr addrspace(1) %refactor_cont_composed_callee_gep to ptr
  %gc_write_barrier9 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr8, ptr addrspace(1) null)
  store ptr addrspace(1) %refactor_cont_root_reload, ptr %explicit_root_frame_slot_2, align 8
  br label %refactor.st3

refactor.st3:                                     ; preds = %refactor.st2
  %rt_alloc_string_lit10 = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_type_desc_runtime__ScoopString, i64 48)
  %str_len_gep11 = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit10, i32 0, i32 1
  %str_data_gep12 = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit10, i32 0, i32 2
  store i64 2, ptr addrspace(1) %str_len_gep11, align 8
  store ptr @__scoop_str_data_913_917, ptr addrspace(1) %str_data_gep12, align 8
  store ptr addrspace(1) %rt_alloc_string_lit10, ptr %explicit_root_frame_slot_5, align 8
  %rt_alloc_string_lit13 = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_type_desc_runtime__ScoopString, i64 48)
  %str_len_gep14 = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit13, i32 0, i32 1
  %str_data_gep15 = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit13, i32 0, i32 2
  store i64 1, ptr addrspace(1) %str_len_gep14, align 8
  store ptr @__scoop_str_data_925_928, ptr addrspace(1) %str_data_gep15, align 8
  store ptr addrspace(1) %rt_alloc_string_lit13, ptr %explicit_root_frame_slot_6, align 8
  %pass_mir_load16 = load ptr addrspace(1), ptr %explicit_root_frame_slot_5, align 8
  %pass_mir_load17 = load ptr addrspace(1), ptr %explicit_root_frame_slot_6, align 8
  %refactor_core_string_concat = call ptr addrspace(1) @scoop_string_concat(ptr addrspace(1) %pass_mir_load16, ptr addrspace(1) %pass_mir_load17)
  store ptr addrspace(1) %refactor_core_string_concat, ptr %explicit_root_frame_slot_3, align 8
  %pass_mir_load18 = load ptr addrspace(1), ptr %explicit_root_frame_slot_3, align 8
  store ptr addrspace(1) %pass_mir_load18, ptr %explicit_root_frame_slot_0, align 8
  br label %refactor.st4

refactor.st4:                                     ; preds = %refactor.st5, %refactor.st3
  call void @"scoop.core.println::<Int>"(i64 0)
  %pass_mir_load23 = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  store ptr addrspace(1) %pass_mir_load23, ptr %explicit_root_frame_slot_9, align 8
  %pass_mir_call_arg_reload_0 = load ptr addrspace(1), ptr %explicit_root_frame_slot_9, align 8
  %tracked_explicit_gc_root_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  %gc_root_keepalive_4294967295 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0, align 8
  call void @"scoop.core.println::<String>"(ptr addrspace(1) %pass_mir_call_arg_reload_0)
  %gc_root_keepalive_reload = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_9, align 8
  br label %return

refactor.st5:                                     ; No predecessors!
  %tmp2.0.load52 = load i64, ptr poison, align 8
  store i64 %tmp2.0.load52, ptr poison, align 8
  br label %refactor.st4

refactor.st6:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr38 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev39 = load ptr, ptr %explicit_root_frame_pop_prev_ptr38, align 8
  %explicit_root_frame_pop_slot_040 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_040, align 8
  %explicit_root_frame_pop_slot_141 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_141, align 8
  %explicit_root_frame_pop_slot_242 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_242, align 8
  %explicit_root_frame_pop_slot_343 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_343, align 8
  %explicit_root_frame_pop_slot_444 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_444, align 8
  %explicit_root_frame_pop_slot_545 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_545, align 8
  %explicit_root_frame_pop_slot_646 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_646, align 8
  %explicit_root_frame_pop_slot_747 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_747, align 8
  %explicit_root_frame_pop_slot_848 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_848, align 8
  %explicit_root_frame_pop_slot_949 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_949, align 8
  store ptr %explicit_root_frame_pop_prev39, ptr @__scoop_explicit_root_frame_top, align 8
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
  %explicit_root_frame_storage = alloca ptr, i32 14, align 8
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
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_7, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_8, align 8
  store ptr addrspace(1) %0, ptr %explicit_root_frame_slot_8, align 8
  %refactor_resume_cont_root_reload = load ptr addrspace(1), ptr %explicit_root_frame_slot_8, align 8
  %refactor_load_frame_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 1
  %refactor_load_frame_gc = load ptr addrspace(1), ptr addrspace(1) %refactor_load_frame_gep, align 8
  store ptr addrspace(1) %refactor_load_frame_gc, ptr %explicit_root_frame_slot_7, align 8
  %refactor_frame_root_reload = load ptr addrspace(1), ptr %explicit_root_frame_slot_7, align 8
  %refactor_frame_slot_load_gep = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_frame_root_reload, i32 0, i32 1
  %refactor_frame_slot_load = load ptr addrspace(1), ptr addrspace(1) %refactor_frame_slot_load_gep, align 8
  store ptr addrspace(1) %refactor_frame_slot_load, ptr %explicit_root_frame_slot_0, align 8
  %refactor_frame_root_reload1 = load ptr addrspace(1), ptr %explicit_root_frame_slot_7, align 8
  %refactor_frame_slot_load_gep2 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_frame_root_reload1, i32 0, i32 2
  %refactor_frame_slot_load3 = load i64, ptr addrspace(1) %refactor_frame_slot_load_gep2, align 8
  %refactor_resume_state_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 2
  %refactor_resume_state = load i32, ptr addrspace(1) %refactor_resume_state_gep, align 4
  %refactor_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 3
  %refactor_one_shot = load i1, ptr addrspace(1) %refactor_one_shot_gep, align 1
  br i1 %refactor_one_shot, label %resume_double, label %resume_first

refactor.st0:                                     ; No predecessors!
  %rt_alloc_string_lit = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_type_desc_runtime__ScoopString, i64 48)
  %str_len_gep = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit, i32 0, i32 1
  %str_data_gep = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit, i32 0, i32 2
  store i64 4, ptr addrspace(1) %str_len_gep, align 8
  store ptr @__scoop_str_data_795_801, ptr addrspace(1) %str_data_gep, align 8
  store ptr addrspace(1) %rt_alloc_string_lit, ptr %explicit_root_frame_slot_1, align 8
  %pass_mir_load16 = load ptr addrspace(1), ptr %explicit_root_frame_slot_1, align 8
  store ptr addrspace(1) %pass_mir_load16, ptr poison, align 8
  store ptr addrspace(1) %pass_mir_load16, ptr %explicit_root_frame_slot_0, align 8
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
  store ptr %explicit_root_frame_pop_prev, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

refactor.st2:                                     ; preds = %refactor.st0
  %refactor_frame_root_reload17 = load ptr addrspace(1), ptr %explicit_root_frame_slot_7, align 8
  %refactor_frame_slot_store_gep18 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_frame_root_reload17, i32 0, i32 1
  %pass_mir_load19 = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  %gc_wb_slot_addr20 = addrspacecast ptr addrspace(1) %refactor_frame_slot_store_gep18 to ptr
  %gc_write_barrier21 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr20, ptr addrspace(1) %pass_mir_load19)
  %refactor_frame_root_reload22 = load ptr addrspace(1), ptr %explicit_root_frame_slot_7, align 8
  %refactor_frame_slot_store_gep23 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_frame_root_reload22, i32 0, i32 2
  %tmp2.0.load208 = load i64, ptr poison, align 8
  store i64 %tmp2.0.load208, ptr addrspace(1) %refactor_frame_slot_store_gep23, align 8
  %rt_alloc_refactor_cont25 = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_continuation_layout__fixtures_build_main__type_desc, i64 72)
  %refactor_cont_zero_field_126 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont25, i32 0, i32 1
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_126, align 8
  %refactor_cont_zero_field_227 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont25, i32 0, i32 2
  store i32 0, ptr addrspace(1) %refactor_cont_zero_field_227, align 4
  %refactor_cont_zero_field_328 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont25, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_zero_field_328, align 1
  %refactor_cont_zero_field_429 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont25, i32 0, i32 4
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_429, align 8
  %refactor_cont_zero_field_530 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont25, i32 0, i32 5
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_530, align 8
  %refactor_cont_zero_field_631 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_cont25, i32 0, i32 6
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_631, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_10, align 8
  store ptr addrspace(1) %rt_alloc_refactor_cont25, ptr %explicit_root_frame_slot_10, align 8
  %refactor_cont_root_reload33 = load ptr addrspace(1), ptr %explicit_root_frame_slot_10, align 8
  %refactor_frame_root_reload34 = load ptr addrspace(1), ptr %explicit_root_frame_slot_7, align 8
  %refactor_cont_frame_gep35 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %refactor_cont_root_reload33, i32 0, i32 1
  %gc_wb_slot_addr36 = addrspacecast ptr addrspace(1) %refactor_cont_frame_gep35 to ptr
  %gc_write_barrier37 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr36, ptr addrspace(1) %refactor_frame_root_reload34)
  %refactor_cont_state_gep38 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %refactor_cont_root_reload33, i32 0, i32 2
  store i32 5, ptr addrspace(1) %refactor_cont_state_gep38, align 4
  %refactor_cont_one_shot_gep39 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %refactor_cont_root_reload33, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_one_shot_gep39, align 1
  %refactor_cont_composed_callee_gep40 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %refactor_cont_root_reload33, i32 0, i32 4
  %gc_wb_slot_addr41 = addrspacecast ptr addrspace(1) %refactor_cont_composed_callee_gep40 to ptr
  %gc_write_barrier42 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr41, ptr addrspace(1) null)
  store ptr addrspace(1) %refactor_cont_root_reload33, ptr %explicit_root_frame_slot_2, align 8
  br label %refactor.st3

refactor.st3:                                     ; preds = %refactor.st2
  %rt_alloc_string_lit43 = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_type_desc_runtime__ScoopString, i64 48)
  %str_len_gep44 = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit43, i32 0, i32 1
  %str_data_gep45 = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit43, i32 0, i32 2
  store i64 2, ptr addrspace(1) %str_len_gep44, align 8
  store ptr @__scoop_str_data_913_917, ptr addrspace(1) %str_data_gep45, align 8
  store ptr addrspace(1) %rt_alloc_string_lit43, ptr %explicit_root_frame_slot_5, align 8
  %rt_alloc_string_lit46 = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_type_desc_runtime__ScoopString, i64 48)
  %str_len_gep47 = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit46, i32 0, i32 1
  %str_data_gep48 = getelementptr inbounds nuw %scoop.runtime.ScoopString, ptr addrspace(1) %rt_alloc_string_lit46, i32 0, i32 2
  store i64 1, ptr addrspace(1) %str_len_gep47, align 8
  store ptr @__scoop_str_data_925_928, ptr addrspace(1) %str_data_gep48, align 8
  store ptr addrspace(1) %rt_alloc_string_lit46, ptr %explicit_root_frame_slot_6, align 8
  %pass_mir_load49 = load ptr addrspace(1), ptr %explicit_root_frame_slot_5, align 8
  %pass_mir_load50 = load ptr addrspace(1), ptr %explicit_root_frame_slot_6, align 8
  %refactor_core_string_concat = call ptr addrspace(1) @scoop_string_concat(ptr addrspace(1) %pass_mir_load49, ptr addrspace(1) %pass_mir_load50)
  store ptr addrspace(1) %refactor_core_string_concat, ptr %explicit_root_frame_slot_3, align 8
  %pass_mir_load51 = load ptr addrspace(1), ptr %explicit_root_frame_slot_3, align 8
  store ptr addrspace(1) %pass_mir_load51, ptr poison, align 8
  store ptr addrspace(1) %pass_mir_load51, ptr %explicit_root_frame_slot_0, align 8
  store i64 0, ptr poison, align 8
  %tmp1.0.load = load i64, ptr poison, align 8
  %refactor_step_tmp54 = alloca %scoop.refactor.Step__fixtures_build_main, align 8
  store %scoop.refactor.Step__fixtures_build_main zeroinitializer, ptr %refactor_step_tmp54, align 8
  %refactor_step_tag_gep55 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp54, i32 0, i32 0
  store i32 0, ptr %refactor_step_tag_gep55, align 4
  %refactor_step_storage_gep56 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp54, i32 0, i32 1
  %refactor_step_payload_insert = insertvalue %scoop.refactor.StepComplete__fixtures_build_main undef, i64 %tmp1.0.load, 0
  store %scoop.refactor.StepComplete__fixtures_build_main %refactor_step_payload_insert, ptr %refactor_step_storage_gep56, align 8
  %refactor_step57 = load %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp54, align 8
  %refactor_frame_root_reload58 = load ptr addrspace(1), ptr %explicit_root_frame_slot_7, align 8
  %refactor_frame_slot_store_gep59 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_frame_root_reload58, i32 0, i32 1
  %pass_mir_load60 = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  %gc_wb_slot_addr61 = addrspacecast ptr addrspace(1) %refactor_frame_slot_store_gep59 to ptr
  %gc_write_barrier62 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr61, ptr addrspace(1) %pass_mir_load60)
  %refactor_frame_root_reload63 = load ptr addrspace(1), ptr %explicit_root_frame_slot_7, align 8
  %refactor_frame_slot_store_gep64 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_frame_root_reload63, i32 0, i32 2
  %tmp2.0.load209 = load i64, ptr poison, align 8
  store i64 %tmp2.0.load209, ptr addrspace(1) %refactor_frame_slot_store_gep64, align 8
  %explicit_root_frame_pop_prev_ptr105 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev106 = load ptr, ptr %explicit_root_frame_pop_prev_ptr105, align 8
  %explicit_root_frame_pop_slot_0107 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0107, align 8
  %explicit_root_frame_pop_slot_1108 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1108, align 8
  %explicit_root_frame_pop_slot_2109 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2109, align 8
  %explicit_root_frame_pop_slot_3110 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3110, align 8
  %explicit_root_frame_pop_slot_4111 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_4111, align 8
  %explicit_root_frame_pop_slot_5112 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_5112, align 8
  %explicit_root_frame_pop_slot_6113 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_6113, align 8
  %explicit_root_frame_pop_slot_7114 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_7114, align 8
  %explicit_root_frame_pop_slot_8115 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_8115, align 8
  %explicit_root_frame_pop_slot_9116 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_9116, align 8
  %explicit_root_frame_pop_slot_10117 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_10117, align 8
  %explicit_root_frame_pop_slot_11118 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_11118, align 8
  store ptr %explicit_root_frame_pop_prev106, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_main %refactor_step57

refactor.st4:                                     ; No predecessors!
  %tmp1.0.load205 = load i64, ptr poison, align 8
  call void @"scoop.core.println::<Int>"(i64 %tmp1.0.load205)
  %pass_mir_load69 = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  store ptr addrspace(1) %pass_mir_load69, ptr poison, align 8
  store ptr addrspace(1) %pass_mir_load69, ptr %explicit_root_frame_slot_11, align 8
  %pass_mir_call_arg_reload_0 = load ptr addrspace(1), ptr %explicit_root_frame_slot_11, align 8
  %tracked_explicit_gc_root_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  %gc_root_keepalive_4294967295 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0, align 8
  call void @"scoop.core.println::<String>"(ptr addrspace(1) %pass_mir_call_arg_reload_0)
  %gc_root_keepalive_reload = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload, ptr poison, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_11, align 8
  %refactor_step_tmp71 = alloca %scoop.refactor.Step__fixtures_build_main, align 8
  store %scoop.refactor.Step__fixtures_build_main zeroinitializer, ptr %refactor_step_tmp71, align 8
  %refactor_step_tag_gep72 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp71, i32 0, i32 0
  store i32 0, ptr %refactor_step_tag_gep72, align 4
  %refactor_step_storage_gep73 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp71, i32 0, i32 1
  %refactor_step_payload_insert74 = insertvalue %scoop.refactor.StepComplete__fixtures_build_main undef, i64 %tmp1.0.load205, 0
  store %scoop.refactor.StepComplete__fixtures_build_main %refactor_step_payload_insert74, ptr %refactor_step_storage_gep73, align 8
  %refactor_step75 = load %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp71, align 8
  %refactor_frame_root_reload76 = load ptr addrspace(1), ptr %explicit_root_frame_slot_7, align 8
  %refactor_frame_slot_store_gep77 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_frame_root_reload76, i32 0, i32 1
  %pass_mir_load78 = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  %gc_wb_slot_addr79 = addrspacecast ptr addrspace(1) %refactor_frame_slot_store_gep77 to ptr
  %tracked_explicit_gc_root_slot_080 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  %gc_root_keepalive_429496729581 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_080, align 8
  %gc_write_barrier82 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr79, ptr addrspace(1) %pass_mir_load78)
  %gc_root_keepalive_reload83 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_080, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload83, ptr poison, align 8
  %refactor_frame_root_reload84 = load ptr addrspace(1), ptr %explicit_root_frame_slot_7, align 8
  %refactor_frame_slot_store_gep85 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_frame_root_reload84, i32 0, i32 2
  %tmp2.0.load210 = load i64, ptr poison, align 8
  store i64 %tmp2.0.load210, ptr addrspace(1) %refactor_frame_slot_store_gep85, align 8
  %explicit_root_frame_pop_prev_ptr119 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev120 = load ptr, ptr %explicit_root_frame_pop_prev_ptr119, align 8
  %explicit_root_frame_pop_slot_0121 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0121, align 8
  %explicit_root_frame_pop_slot_1122 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1122, align 8
  %explicit_root_frame_pop_slot_2123 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2123, align 8
  %explicit_root_frame_pop_slot_3124 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3124, align 8
  %explicit_root_frame_pop_slot_4125 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_4125, align 8
  %explicit_root_frame_pop_slot_5126 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_5126, align 8
  %explicit_root_frame_pop_slot_6127 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_6127, align 8
  %explicit_root_frame_pop_slot_7128 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_7128, align 8
  %explicit_root_frame_pop_slot_8129 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_8129, align 8
  %explicit_root_frame_pop_slot_9130 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_9130, align 8
  %explicit_root_frame_pop_slot_10131 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_10131, align 8
  %explicit_root_frame_pop_slot_11132 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_11132, align 8
  store ptr %explicit_root_frame_pop_prev120, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_main %refactor_step75

refactor.st5:                                     ; preds = %resume_payload_st5
  %refactor_step_tmp89 = alloca %scoop.refactor.Step__fixtures_build_main, align 8
  store %scoop.refactor.Step__fixtures_build_main zeroinitializer, ptr %refactor_step_tmp89, align 8
  %refactor_step_tag_gep90 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp89, i32 0, i32 0
  store i32 0, ptr %refactor_step_tag_gep90, align 4
  %refactor_step_storage_gep91 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp89, i32 0, i32 1
  %refactor_step_payload_insert92 = insertvalue %scoop.refactor.StepComplete__fixtures_build_main undef, i64 %1, 0
  store %scoop.refactor.StepComplete__fixtures_build_main %refactor_step_payload_insert92, ptr %refactor_step_storage_gep91, align 8
  %refactor_step93 = load %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp89, align 8
  %refactor_frame_root_reload94 = load ptr addrspace(1), ptr %explicit_root_frame_slot_7, align 8
  %refactor_frame_slot_store_gep95 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_frame_root_reload94, i32 0, i32 1
  %pass_mir_load96 = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  %gc_wb_slot_addr97 = addrspacecast ptr addrspace(1) %refactor_frame_slot_store_gep95 to ptr
  %tracked_explicit_gc_root_slot_098 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  %gc_root_keepalive_429496729599 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_098, align 8
  %gc_write_barrier100 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr97, ptr addrspace(1) %pass_mir_load96)
  %gc_root_keepalive_reload101 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_098, align 8
  %refactor_frame_root_reload102 = load ptr addrspace(1), ptr %explicit_root_frame_slot_7, align 8
  %refactor_frame_slot_store_gep103 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_frame_root_reload102, i32 0, i32 2
  store i64 %1, ptr addrspace(1) %refactor_frame_slot_store_gep103, align 8
  %explicit_root_frame_pop_prev_ptr133 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev134 = load ptr, ptr %explicit_root_frame_pop_prev_ptr133, align 8
  %explicit_root_frame_pop_slot_0135 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0135, align 8
  %explicit_root_frame_pop_slot_1136 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1136, align 8
  %explicit_root_frame_pop_slot_2137 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2137, align 8
  %explicit_root_frame_pop_slot_3138 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3138, align 8
  %explicit_root_frame_pop_slot_4139 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_4139, align 8
  %explicit_root_frame_pop_slot_5140 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_5140, align 8
  %explicit_root_frame_pop_slot_6141 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_6141, align 8
  %explicit_root_frame_pop_slot_7142 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_7142, align 8
  %explicit_root_frame_pop_slot_8143 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_8143, align 8
  %explicit_root_frame_pop_slot_9144 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_9144, align 8
  %explicit_root_frame_pop_slot_10145 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_10145, align 8
  %explicit_root_frame_pop_slot_11146 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_11146, align 8
  store ptr %explicit_root_frame_pop_prev134, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_main %refactor_step93

refactor.st6:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr147 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev148 = load ptr, ptr %explicit_root_frame_pop_prev_ptr147, align 8
  %explicit_root_frame_pop_slot_0149 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0149, align 8
  %explicit_root_frame_pop_slot_1150 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1150, align 8
  %explicit_root_frame_pop_slot_2151 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2151, align 8
  %explicit_root_frame_pop_slot_3152 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3152, align 8
  %explicit_root_frame_pop_slot_4153 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_4153, align 8
  %explicit_root_frame_pop_slot_5154 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_5154, align 8
  %explicit_root_frame_pop_slot_6155 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_6155, align 8
  %explicit_root_frame_pop_slot_7156 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_7156, align 8
  %explicit_root_frame_pop_slot_8157 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_8157, align 8
  %explicit_root_frame_pop_slot_9158 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_9158, align 8
  %explicit_root_frame_pop_slot_10159 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_10159, align 8
  %explicit_root_frame_pop_slot_11160 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_11160, align 8
  store ptr %explicit_root_frame_pop_prev148, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_first:                                     ; preds = %entry
  %refactor_store_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 3
  store i1 true, ptr addrspace(1) %refactor_store_one_shot_gep, align 1
  %refactor_composed_callee_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 4
  %refactor_composed_callee = load ptr addrspace(1), ptr addrspace(1) %refactor_composed_callee_gep, align 8
  %refactor_composed_callee_is_null = icmp eq ptr addrspace(1) %refactor_composed_callee, null
  br i1 %refactor_composed_callee_is_null, label %resume_plain_dispatch, label %resume_composed_dispatch

resume_double:                                    ; preds = %entry
  %rt_alloc_refactor_cont = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_continuation_layout__fixtures_build_main__type_desc, i64 72)
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
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_9, align 8
  store ptr addrspace(1) %rt_alloc_refactor_cont, ptr %explicit_root_frame_slot_9, align 8
  %refactor_cont_root_reload = load ptr addrspace(1), ptr %explicit_root_frame_slot_9, align 8
  %refactor_frame_root_reload4 = load ptr addrspace(1), ptr %explicit_root_frame_slot_7, align 8
  %refactor_cont_frame_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 1
  %gc_wb_slot_addr = addrspacecast ptr addrspace(1) %refactor_cont_frame_gep to ptr
  %gc_write_barrier = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr, ptr addrspace(1) %refactor_frame_root_reload4)
  %refactor_cont_state_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 2
  store i32 %refactor_resume_state, ptr addrspace(1) %refactor_cont_state_gep, align 4
  %refactor_cont_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_one_shot_gep, align 1
  %refactor_cont_composed_callee_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_main, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 4
  %gc_wb_slot_addr5 = addrspacecast ptr addrspace(1) %refactor_cont_composed_callee_gep to ptr
  %gc_write_barrier6 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr5, ptr addrspace(1) null)
  %refactor_step_tmp = alloca %scoop.refactor.Step__fixtures_build_main, align 8
  store %scoop.refactor.Step__fixtures_build_main zeroinitializer, ptr %refactor_step_tmp, align 8
  %refactor_step_tag_gep = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp, i32 0, i32 0
  store i32 2, ptr %refactor_step_tag_gep, align 4
  %refactor_step_storage_gep = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp, i32 0, i32 1
  %refactor_step_cont_insert = insertvalue %scoop.refactor.StepCase__fixtures_build_main__case1 { %scoop.core.RuntimeError { i32 2, i64 0, ptr addrspace(1) null }, ptr addrspace(1) undef }, ptr addrspace(1) %refactor_cont_root_reload, 1
  store %scoop.refactor.StepCase__fixtures_build_main__case1 %refactor_step_cont_insert, ptr %refactor_step_storage_gep, align 8
  %refactor_step = load %scoop.refactor.Step__fixtures_build_main, ptr %refactor_step_tmp, align 8
  %refactor_frame_root_reload7 = load ptr addrspace(1), ptr %explicit_root_frame_slot_7, align 8
  %refactor_frame_slot_store_gep = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_frame_root_reload7, i32 0, i32 1
  %pass_mir_load = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  %gc_wb_slot_addr8 = addrspacecast ptr addrspace(1) %refactor_frame_slot_store_gep to ptr
  %gc_write_barrier9 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr8, ptr addrspace(1) %pass_mir_load)
  %refactor_frame_root_reload10 = load ptr addrspace(1), ptr %explicit_root_frame_slot_7, align 8
  %refactor_frame_slot_store_gep11 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_frame_root_reload10, i32 0, i32 2
  store i64 %refactor_frame_slot_load3, ptr addrspace(1) %refactor_frame_slot_store_gep11, align 8
  %explicit_root_frame_pop_prev_ptr161 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev162 = load ptr, ptr %explicit_root_frame_pop_prev_ptr161, align 8
  %explicit_root_frame_pop_slot_0163 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0163, align 8
  %explicit_root_frame_pop_slot_1164 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1164, align 8
  %explicit_root_frame_pop_slot_2165 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2165, align 8
  %explicit_root_frame_pop_slot_3166 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3166, align 8
  %explicit_root_frame_pop_slot_4167 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_4167, align 8
  %explicit_root_frame_pop_slot_5168 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_5168, align 8
  %explicit_root_frame_pop_slot_6169 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_6169, align 8
  %explicit_root_frame_pop_slot_7170 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_7170, align 8
  %explicit_root_frame_pop_slot_8171 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_8171, align 8
  %explicit_root_frame_pop_slot_9172 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_9172, align 8
  %explicit_root_frame_pop_slot_10173 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_10173, align 8
  %explicit_root_frame_pop_slot_11174 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_11174, align 8
  store ptr %explicit_root_frame_pop_prev162, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_main %refactor_step

resume_plain_dispatch:                            ; preds = %resume_first
  switch i32 %refactor_resume_state, label %resume_invalid_state [
    i32 5, label %resume_payload_st5
  ]

resume_composed_dispatch:                         ; preds = %resume_first
  %explicit_root_frame_pop_prev_ptr175 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev176 = load ptr, ptr %explicit_root_frame_pop_prev_ptr175, align 8
  %explicit_root_frame_pop_slot_0177 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0177, align 8
  %explicit_root_frame_pop_slot_1178 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1178, align 8
  %explicit_root_frame_pop_slot_2179 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2179, align 8
  %explicit_root_frame_pop_slot_3180 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3180, align 8
  %explicit_root_frame_pop_slot_4181 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_4181, align 8
  %explicit_root_frame_pop_slot_5182 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_5182, align 8
  %explicit_root_frame_pop_slot_6183 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_6183, align 8
  %explicit_root_frame_pop_slot_7184 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_7184, align 8
  %explicit_root_frame_pop_slot_8185 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_8185, align 8
  %explicit_root_frame_pop_slot_9186 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_9186, align 8
  %explicit_root_frame_pop_slot_10187 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_10187, align 8
  %explicit_root_frame_pop_slot_11188 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_11188, align 8
  store ptr %explicit_root_frame_pop_prev176, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_invalid_state:                             ; preds = %resume_plain_dispatch
  %explicit_root_frame_pop_prev_ptr189 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev190 = load ptr, ptr %explicit_root_frame_pop_prev_ptr189, align 8
  %explicit_root_frame_pop_slot_0191 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0191, align 8
  %explicit_root_frame_pop_slot_1192 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1192, align 8
  %explicit_root_frame_pop_slot_2193 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2193, align 8
  %explicit_root_frame_pop_slot_3194 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3194, align 8
  %explicit_root_frame_pop_slot_4195 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 48
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_4195, align 8
  %explicit_root_frame_pop_slot_5196 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 56
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_5196, align 8
  %explicit_root_frame_pop_slot_6197 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 64
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_6197, align 8
  %explicit_root_frame_pop_slot_7198 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 72
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_7198, align 8
  %explicit_root_frame_pop_slot_8199 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 80
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_8199, align 8
  %explicit_root_frame_pop_slot_9200 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 88
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_9200, align 8
  %explicit_root_frame_pop_slot_10201 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 96
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_10201, align 8
  %explicit_root_frame_pop_slot_11202 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 104
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_11202, align 8
  store ptr %explicit_root_frame_pop_prev190, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_payload_st5:                               ; preds = %resume_plain_dispatch
  %refactor_frame_root_reload13 = load ptr addrspace(1), ptr %explicit_root_frame_slot_7, align 8
  %refactor_frame_slot_store_gep14 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %refactor_frame_root_reload13, i32 0, i32 2
  store i64 %1, ptr addrspace(1) %refactor_frame_slot_store_gep14, align 8
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
