; ModuleID = 'effect_no_perform_no_handler_symbols_basic'
source_filename = "effect_no_perform_no_handler_symbols_basic"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-darwin25.4.0"

%scoop.refactor.StepComplete__fixtures_build_main = type {}
%scoop.refactor.StepCase__fixtures_build_main__case0 = type { %scoop.core.RuntimeError, ptr addrspace(1) }
%scoop.core.RuntimeError = type { i32, i64, ptr addrspace(1) }
%scoop.refactor.Step__fixtures_build_main = type { i32, %scoop.refactor.StepStorage__fixtures_build_main }
%scoop.refactor.StepStorage__fixtures_build_main = type { [4 x i64] }
%scoop.refactor.ResumeVtable__fixtures_build_main__scoop_core_Raise = type { ptr }
%scoop.refactor.Frame__fixtures_build_main = type { %scoop.runtime.ScoopGcObjectHeader, i64, ptr addrspace(1), i1, i1, i64 }
%scoop.runtime.ScoopGcObjectHeader = type { ptr, ptr, i64, i32, i32 }
%scoop.refactor.Continuation__fixtures_build_main = type { %scoop.runtime.ScoopGcObjectHeader, ptr addrspace(1), i32, i1, ptr addrspace(1), ptr }
%scoop.runtime.ScoopRootFrameDesc = type { i32, ptr }
%scoop.runtime.ScoopTypeDescriptor = type { i32, i32, i64, i64, i64, i32, i32, ptr, ptr, ptr, i64, ptr, ptr, ptr }
%scoop.runtime.ScoopRootFrameHeader = type { ptr, ptr }

@__scoop_refactor_step_variant_payload__fixtures_build_main__complete = internal constant %scoop.refactor.StepComplete__fixtures_build_main zeroinitializer
@__scoop_refactor_step_case_tag__fixtures_build_main__complete = internal constant i32 0
@__scoop_refactor_step_variant_payload__fixtures_build_main__case0 = internal constant %scoop.refactor.StepCase__fixtures_build_main__case0 zeroinitializer
@__scoop_refactor_step_case_tag__fixtures_build_main__case0 = internal constant i32 1
@__scoop_refactor_step_layout__fixtures_build_main = internal constant %scoop.refactor.Step__fixtures_build_main zeroinitializer
@__scoop_refactor_resume_vtable_layout__fixtures_build_main__scoop_core_Raise = internal constant %scoop.refactor.ResumeVtable__fixtures_build_main__scoop_core_Raise zeroinitializer
@__scoop_refactor_frame_layout__fixtures_build_main = internal constant %scoop.refactor.Frame__fixtures_build_main zeroinitializer
@__scoop_refactor_continuation_layout__fixtures_build_main = internal constant %scoop.refactor.Continuation__fixtures_build_main zeroinitializer
@__scoop_explicit_root_desc__fixtures_build_helper = internal constant %scoop.runtime.ScoopRootFrameDesc zeroinitializer
@__scoop_explicit_root_frame_top = external thread_local global ptr
@__scoop_refactor_frame_layout__fixtures_build_main__type_desc__trace_bitmap = internal constant [1 x i64] [i64 2]
@__scoop_refactor_frame_layout__fixtures_build_main__type_desc = internal constant %scoop.runtime.ScoopTypeDescriptor { i32 0, i32 0, i64 64, i64 8, i64 32, i32 1, i32 0, ptr @__scoop_refactor_frame_layout__fixtures_build_main__type_desc__trace_bitmap, ptr null, ptr null, i64 -8046005298092833786, ptr null, ptr null, ptr null }
@__scoop_explicit_root_offsets__fixtures_build_main = internal constant [3 x i32] [i32 16, i32 24, i32 32]
@__scoop_explicit_root_desc__fixtures_build_main = internal constant %scoop.runtime.ScoopRootFrameDesc { i32 3, ptr @__scoop_explicit_root_offsets__fixtures_build_main }
@__scoop_explicit_root_offsets__scoop_core_println___Int_ = internal constant [2 x i32] [i32 16, i32 24]
@__scoop_explicit_root_desc__scoop_core_println___Int_ = internal constant %scoop.runtime.ScoopRootFrameDesc { i32 2, ptr @__scoop_explicit_root_offsets__scoop_core_println___Int_ }
@__scoop_explicit_root_desc__main = internal constant %scoop.runtime.ScoopRootFrameDesc zeroinitializer

define %scoop.refactor.Step__fixtures_build_main @__scoop_refactor_resume__fixtures_build_main__case0(ptr addrspace(1) %0, i8 %1) {
entry:
  unreachable
}

define %scoop.refactor.Step__fixtures_build_main @__scoop_refactor_surface_resume__k0(ptr addrspace(1) %0, i8 %1) {
entry:
  unreachable
}

define i64 @fixtures.build.helper(i64 %0) {
entry:
  %explicit_root_frame_storage = alloca ptr, i32 2, align 8
  %explicit_root_frame_prev_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_desc_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 1
  %explicit_root_frame_prev = load ptr, ptr @__scoop_explicit_root_frame_top, align 8
  store ptr %explicit_root_frame_prev, ptr %explicit_root_frame_prev_ptr, align 8
  store ptr @__scoop_explicit_root_desc__fixtures_build_helper, ptr %explicit_root_frame_desc_ptr, align 8
  store ptr %explicit_root_frame_storage, ptr @__scoop_explicit_root_frame_top, align 8
  br label %plain.bb0

return:                                           ; preds = %plain.bb0
  %explicit_root_frame_pop_prev_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev = load ptr, ptr %explicit_root_frame_pop_prev_ptr, align 8
  store ptr %explicit_root_frame_pop_prev, ptr @__scoop_explicit_root_frame_top, align 8
  ret i64 %pass_mir_iadd

plain.bb0:                                        ; preds = %entry
  %pass_mir_iadd = add i64 %0, 1
  br label %return
}

define void @fixtures.build.main() {
entry:
  %explicit_root_frame_storage = alloca ptr, i32 5, align 8
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
  store ptr %explicit_root_frame_storage, ptr @__scoop_explicit_root_frame_top, align 8
  %explicit_root_frame_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %explicit_root_frame_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %explicit_root_frame_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_2, align 8
  %tracked_explicit_gc_root_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %gc_root_keepalive_4294967295 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0, align 8
  %rt_alloc_refactor_frame = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_frame_layout__fixtures_build_main__type_desc, i64 64)
  %gc_root_keepalive_reload = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0, align 8
  %refactor_frame_zero_field_1 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_frame, i32 0, i32 1
  store i64 0, ptr addrspace(1) %refactor_frame_zero_field_1, align 8
  %refactor_frame_zero_field_2 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_frame, i32 0, i32 2
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_frame_zero_field_2, align 8
  %refactor_frame_zero_field_3 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_frame, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_frame_zero_field_3, align 1
  %refactor_frame_zero_field_4 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_frame, i32 0, i32 4
  store i1 false, ptr addrspace(1) %refactor_frame_zero_field_4, align 1
  %refactor_frame_zero_field_5 = getelementptr inbounds nuw %scoop.refactor.Frame__fixtures_build_main, ptr addrspace(1) %rt_alloc_refactor_frame, i32 0, i32 5
  store i64 0, ptr addrspace(1) %refactor_frame_zero_field_5, align 8
  store ptr addrspace(1) %rt_alloc_refactor_frame, ptr %explicit_root_frame_slot_2, align 8
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
  store ptr %explicit_root_frame_pop_prev, ptr @__scoop_explicit_root_frame_top, align 8
  ret void

refactor.st0:                                     ; preds = %entry
  br label %refactor.st2

refactor.st1:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr13 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev14 = load ptr, ptr %explicit_root_frame_pop_prev_ptr13, align 8
  %explicit_root_frame_pop_slot_015 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_015, align 8
  %explicit_root_frame_pop_slot_116 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_116, align 8
  %explicit_root_frame_pop_slot_217 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_217, align 8
  store ptr %explicit_root_frame_pop_prev14, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

refactor.st2:                                     ; preds = %refactor.st0
  %tracked_explicit_gc_root_slot_02 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %gc_root_keepalive_42949672953 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_02, align 8
  %pass_mir_call = call i64 @fixtures.build.helper(i64 41)
  %gc_root_keepalive_reload4 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_02, align 8
  br label %refactor.st4

refactor.st3:                                     ; No predecessors!
  store i64 0, ptr poison, align 8
  br label %refactor.st4

refactor.st4:                                     ; preds = %refactor.st3, %refactor.st2
  %tracked_explicit_gc_root_slot_010 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %gc_root_keepalive_429496729511 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_010, align 8
  call void @"scoop.core.println::<Int>"(i64 %pass_mir_call)
  %gc_root_keepalive_reload12 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_010, align 8
  br label %return

refactor.st5:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr18 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev19 = load ptr, ptr %explicit_root_frame_pop_prev_ptr18, align 8
  %explicit_root_frame_pop_slot_020 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_020, align 8
  %explicit_root_frame_pop_slot_121 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_121, align 8
  %explicit_root_frame_pop_slot_222 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_222, align 8
  store ptr %explicit_root_frame_pop_prev19, ptr @__scoop_explicit_root_frame_top, align 8
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

declare ptr addrspace(1) @scoop_alloc_typed(ptr, i64)

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
  call void @fixtures.build.main()
  %explicit_root_frame_pop_prev_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev = load ptr, ptr %explicit_root_frame_pop_prev_ptr, align 8
  store ptr %explicit_root_frame_pop_prev, ptr @__scoop_explicit_root_frame_top, align 8
  ret i32 0
}

declare void @scoop_runtime_init()
