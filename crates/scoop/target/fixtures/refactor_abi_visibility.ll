; ModuleID = 'refactor_abi_visibility'
source_filename = "refactor_abi_visibility"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-darwin25.4.0"

%scoop.refactor.StepComplete__fixtures_build_fixture_visibility_hiddenWorker = type {}
%scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case0 = type { ptr addrspace(1) }
%scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case1 = type { %scoop.core.RuntimeError, ptr addrspace(1) }
%scoop.core.RuntimeError = type { i32, i64, ptr addrspace(1) }
%scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker = type { i32, %scoop.refactor.StepStorage__fixtures_build_fixture_visibility_hiddenWorker }
%scoop.refactor.StepStorage__fixtures_build_fixture_visibility_hiddenWorker = type { [4 x i64] }
%scoop.refactor.ResumeVtable__fixtures_build_fixture_visibility_hiddenWorker__fixtures_build_fixture_visibility_Ping = type { ptr }
%scoop.refactor.ResumeVtable__fixtures_build_fixture_visibility_hiddenWorker__scoop_core_Raise = type { ptr }
%scoop.refactor.Frame__fixtures_build_fixture_visibility_hiddenWorker = type { %scoop.runtime.ScoopGcObjectHeader, {}, {}, i64, ptr addrspace(1), i1, i1, i64 }
%scoop.runtime.ScoopGcObjectHeader = type { ptr, ptr, i64, i32, i32 }
%scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker = type { %scoop.runtime.ScoopGcObjectHeader, ptr addrspace(1), i32, i1, ptr addrspace(1), ptr, ptr }
%scoop.runtime.ScoopRootFrameDesc = type { i32, ptr }
%scoop.runtime.ScoopTypeDescriptor = type { i32, i32, i64, i64, i64, i32, i32, ptr, ptr, ptr, i64, ptr, ptr, ptr }
%scoop.runtime.ScoopRootFrameHeader = type { ptr, ptr }

@__scoop_refactor_step_variant_payload__fixtures_build_fixture_visibility_hiddenWorker__complete = internal constant %scoop.refactor.StepComplete__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer
@__scoop_refactor_step_case_tag__fixtures_build_fixture_visibility_hiddenWorker__complete = internal constant i32 0
@__scoop_refactor_step_variant_payload__fixtures_build_fixture_visibility_hiddenWorker__case0 = internal constant %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case0 zeroinitializer
@__scoop_refactor_step_case_tag__fixtures_build_fixture_visibility_hiddenWorker__case0 = internal constant i32 1
@__scoop_refactor_step_variant_payload__fixtures_build_fixture_visibility_hiddenWorker__case1 = internal constant %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case1 zeroinitializer
@__scoop_refactor_step_case_tag__fixtures_build_fixture_visibility_hiddenWorker__case1 = internal constant i32 2
@__scoop_refactor_step_layout__fixtures_build_fixture_visibility_hiddenWorker = internal constant %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer
@__scoop_refactor_resume_vtable_layout__fixtures_build_fixture_visibility_hiddenWorker__fixtures_build_fixture_visibility_Ping = internal constant %scoop.refactor.ResumeVtable__fixtures_build_fixture_visibility_hiddenWorker__fixtures_build_fixture_visibility_Ping zeroinitializer
@__scoop_refactor_resume_vtable_layout__fixtures_build_fixture_visibility_hiddenWorker__scoop_core_Raise = internal constant %scoop.refactor.ResumeVtable__fixtures_build_fixture_visibility_hiddenWorker__scoop_core_Raise zeroinitializer
@__scoop_refactor_frame_layout__fixtures_build_fixture_visibility_hiddenWorker = internal constant %scoop.refactor.Frame__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer
@__scoop_refactor_continuation_layout__fixtures_build_fixture_visibility_hiddenWorker = internal constant %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer
@__scoop_explicit_root_desc__fixtures_build_fixture_visibility_main = internal constant %scoop.runtime.ScoopRootFrameDesc zeroinitializer
@__scoop_explicit_root_frame_top = external thread_local global ptr
@__scoop_refactor_continuation_layout__fixtures_build_fixture_visibility_hiddenWorker__type_desc__trace_bitmap = internal constant [1 x i64] [i64 5]
@__scoop_refactor_continuation_layout__fixtures_build_fixture_visibility_hiddenWorker__type_desc = internal constant %scoop.runtime.ScoopTypeDescriptor { i32 0, i32 0, i64 72, i64 8, i64 32, i32 1, i32 0, ptr @__scoop_refactor_continuation_layout__fixtures_build_fixture_visibility_hiddenWorker__type_desc__trace_bitmap, ptr null, ptr null, i64 1891513081878600535, ptr null, ptr null, ptr null }
@__scoop_explicit_root_offsets____scoop_refactor_resume__fixtures_build_fixture_visibility_hiddenWorker__case0 = internal constant [4 x i32] [i32 16, i32 24, i32 32, i32 40]
@__scoop_explicit_root_desc____scoop_refactor_resume__fixtures_build_fixture_visibility_hiddenWorker__case0 = internal constant %scoop.runtime.ScoopRootFrameDesc { i32 4, ptr @__scoop_explicit_root_offsets____scoop_refactor_resume__fixtures_build_fixture_visibility_hiddenWorker__case0 }
@__scoop_explicit_root_offsets____scoop_refactor_resume__fixtures_build_fixture_visibility_hiddenWorker__case1 = internal constant [4 x i32] [i32 16, i32 24, i32 32, i32 40]
@__scoop_explicit_root_desc____scoop_refactor_resume__fixtures_build_fixture_visibility_hiddenWorker__case1 = internal constant %scoop.runtime.ScoopRootFrameDesc { i32 4, ptr @__scoop_explicit_root_offsets____scoop_refactor_resume__fixtures_build_fixture_visibility_hiddenWorker__case1 }
@__scoop_explicit_root_offsets____scoop_refactor_surface_resume_owner_dispatch__fixtures_build_fixture_visibility_hiddenWorker__k0 = internal constant [4 x i32] [i32 16, i32 24, i32 32, i32 40]
@__scoop_explicit_root_desc____scoop_refactor_surface_resume_owner_dispatch__fixtures_build_fixture_visibility_hiddenWorker__k0 = internal constant %scoop.runtime.ScoopRootFrameDesc { i32 4, ptr @__scoop_explicit_root_offsets____scoop_refactor_surface_resume_owner_dispatch__fixtures_build_fixture_visibility_hiddenWorker__k0 }
@__scoop_explicit_root_offsets____scoop_refactor_surface_resume_owner_dispatch__fixtures_build_fixture_visibility_hiddenWorker__k1 = internal constant [4 x i32] [i32 16, i32 24, i32 32, i32 40]
@__scoop_explicit_root_desc____scoop_refactor_surface_resume_owner_dispatch__fixtures_build_fixture_visibility_hiddenWorker__k1 = internal constant %scoop.runtime.ScoopRootFrameDesc { i32 4, ptr @__scoop_explicit_root_offsets____scoop_refactor_surface_resume_owner_dispatch__fixtures_build_fixture_visibility_hiddenWorker__k1 }
@__scoop_explicit_root_desc__main = internal constant %scoop.runtime.ScoopRootFrameDesc zeroinitializer

define %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker @__scoop_refactor_resume__fixtures_build_fixture_visibility_hiddenWorker__case0(ptr addrspace(1) %0) {
entry:
  %explicit_root_frame_storage = alloca ptr, i32 6, align 8
  %explicit_root_frame_prev_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_desc_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 1
  %explicit_root_frame_prev = load ptr, ptr @__scoop_explicit_root_frame_top, align 8
  store ptr %explicit_root_frame_prev, ptr %explicit_root_frame_prev_ptr, align 8
  store ptr @__scoop_explicit_root_desc____scoop_refactor_resume__fixtures_build_fixture_visibility_hiddenWorker__case0, ptr %explicit_root_frame_desc_ptr, align 8
  %explicit_root_frame_init_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_0, align 8
  %explicit_root_frame_init_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_1, align 8
  %explicit_root_frame_init_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_2, align 8
  %explicit_root_frame_init_slot_3 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_3, align 8
  store ptr %explicit_root_frame_storage, ptr @__scoop_explicit_root_frame_top, align 8
  %explicit_root_frame_slot_3 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  %explicit_root_frame_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %explicit_root_frame_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %explicit_root_frame_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_0, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_1, align 8
  store ptr addrspace(1) %0, ptr %explicit_root_frame_slot_1, align 8
  %refactor_load_frame_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %0, i32 0, i32 1
  %refactor_load_frame_gc = load ptr addrspace(1), ptr addrspace(1) %refactor_load_frame_gep, align 8
  store ptr addrspace(1) %refactor_load_frame_gc, ptr %explicit_root_frame_slot_0, align 8
  %refactor_resume_state_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %0, i32 0, i32 2
  %refactor_resume_state = load i32, ptr addrspace(1) %refactor_resume_state_gep, align 4
  %refactor_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %0, i32 0, i32 3
  %refactor_one_shot = load i1, ptr addrspace(1) %refactor_one_shot_gep, align 1
  br i1 %refactor_one_shot, label %resume_double, label %resume_first

refactor.st0:                                     ; No predecessors!
  %tracked_explicit_gc_root_slot_026 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_127 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %tracked_explicit_gc_root_slot_228 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %gc_root_keepalive_429496729329 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_228, align 8
  %gc_root_keepalive_429496729430 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_127, align 8
  %gc_root_keepalive_429496729531 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_026, align 8
  %rt_alloc_refactor_cont32 = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_continuation_layout__fixtures_build_fixture_visibility_hiddenWorker__type_desc, i64 72)
  %gc_root_keepalive_reload33 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_228, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload33, ptr poison, align 8
  %gc_root_keepalive_reload34 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_127, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload34, ptr poison, align 8
  %gc_root_keepalive_reload35 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_026, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload35, ptr poison, align 8
  %refactor_cont_zero_field_136 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 1
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_136, align 8
  %refactor_cont_zero_field_237 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 2
  store i32 0, ptr addrspace(1) %refactor_cont_zero_field_237, align 4
  %refactor_cont_zero_field_338 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_zero_field_338, align 1
  %refactor_cont_zero_field_439 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 4
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_439, align 8
  %refactor_cont_zero_field_540 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 5
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_540, align 8
  %refactor_cont_zero_field_641 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 6
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_641, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_3, align 8
  store ptr addrspace(1) %rt_alloc_refactor_cont32, ptr %explicit_root_frame_slot_3, align 8
  %refactor_frame_root.0.refactor_frame_root_reload46 = load ptr addrspace(1), ptr poison, align 8
  %refactor_cont_frame_gep47 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 1
  %gc_wb_slot_addr48 = addrspacecast ptr addrspace(1) %refactor_cont_frame_gep47 to ptr
  %tracked_explicit_gc_root_slot_049 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_150 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %tracked_explicit_gc_root_slot_251 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %tracked_explicit_gc_root_slot_3 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  %gc_root_keepalive_4294967292 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3, align 8
  %gc_root_keepalive_429496729352 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_251, align 8
  %gc_root_keepalive_429496729453 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_150, align 8
  %gc_root_keepalive_429496729554 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_049, align 8
  %gc_write_barrier55 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr48, ptr addrspace(1) %refactor_frame_root.0.refactor_frame_root_reload46)
  %gc_root_keepalive_reload56 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3, align 8
  %gc_root_keepalive_reload57 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_251, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload57, ptr poison, align 8
  %gc_root_keepalive_reload58 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_150, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload58, ptr poison, align 8
  %gc_root_keepalive_reload59 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_049, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload59, ptr poison, align 8
  %refactor_cont_state_gep60 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 2
  store i32 2, ptr addrspace(1) %refactor_cont_state_gep60, align 4
  %refactor_cont_one_shot_gep61 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_one_shot_gep61, align 1
  %refactor_cont_composed_callee_gep62 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 4
  %gc_wb_slot_addr63 = addrspacecast ptr addrspace(1) %refactor_cont_composed_callee_gep62 to ptr
  %tracked_explicit_gc_root_slot_064 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_165 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %tracked_explicit_gc_root_slot_266 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %tracked_explicit_gc_root_slot_367 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  %gc_root_keepalive_429496729268 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_367, align 8
  %gc_root_keepalive_429496729369 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_266, align 8
  %gc_root_keepalive_429496729470 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_165, align 8
  %gc_root_keepalive_429496729571 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_064, align 8
  %gc_write_barrier72 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr63, ptr addrspace(1) null)
  %gc_root_keepalive_reload73 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_367, align 8
  %gc_root_keepalive_reload74 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_266, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload74, ptr poison, align 8
  %gc_root_keepalive_reload75 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_165, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload75, ptr poison, align 8
  %gc_root_keepalive_reload76 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_064, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload76, ptr poison, align 8
  %refactor_step_tmp77 = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp77, align 8
  %refactor_step_tag_gep78 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp77, i32 0, i32 0
  store i32 1, ptr %refactor_step_tag_gep78, align 4
  %refactor_step_storage_gep79 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp77, i32 0, i32 1
  %refactor_step_cont_insert80 = insertvalue %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case0 undef, ptr addrspace(1) %rt_alloc_refactor_cont32, 0
  store %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case0 %refactor_step_cont_insert80, ptr %refactor_step_storage_gep79, align 8
  %refactor_step81 = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp77, align 8
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
  store ptr %explicit_root_frame_pop_prev, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step81

refactor.st1:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr86 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev87 = load ptr, ptr %explicit_root_frame_pop_prev_ptr86, align 8
  %explicit_root_frame_pop_slot_088 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_088, align 8
  %explicit_root_frame_pop_slot_189 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_189, align 8
  %explicit_root_frame_pop_slot_290 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_290, align 8
  %explicit_root_frame_pop_slot_391 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_391, align 8
  store ptr %explicit_root_frame_pop_prev87, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

refactor.st2:                                     ; preds = %resume_payload_st2
  %refactor_step_tmp82 = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp82, align 8
  %refactor_step_tag_gep83 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp82, i32 0, i32 0
  store i32 0, ptr %refactor_step_tag_gep83, align 4
  %refactor_step_storage_gep84 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp82, i32 0, i32 1
  store %scoop.refactor.StepComplete__fixtures_build_fixture_visibility_hiddenWorker undef, ptr %refactor_step_storage_gep84, align 1
  %refactor_step85 = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp82, align 8
  %explicit_root_frame_pop_prev_ptr92 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev93 = load ptr, ptr %explicit_root_frame_pop_prev_ptr92, align 8
  %explicit_root_frame_pop_slot_094 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_094, align 8
  %explicit_root_frame_pop_slot_195 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_195, align 8
  %explicit_root_frame_pop_slot_296 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_296, align 8
  %explicit_root_frame_pop_slot_397 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_397, align 8
  store ptr %explicit_root_frame_pop_prev93, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step85

refactor.st3:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr98 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev99 = load ptr, ptr %explicit_root_frame_pop_prev_ptr98, align 8
  %explicit_root_frame_pop_slot_0100 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0100, align 8
  %explicit_root_frame_pop_slot_1101 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1101, align 8
  %explicit_root_frame_pop_slot_2102 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2102, align 8
  %explicit_root_frame_pop_slot_3103 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3103, align 8
  store ptr %explicit_root_frame_pop_prev99, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_first:                                     ; preds = %entry
  %refactor_store_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %0, i32 0, i32 3
  store i1 true, ptr addrspace(1) %refactor_store_one_shot_gep, align 1
  %refactor_composed_callee_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %0, i32 0, i32 4
  %refactor_composed_callee = load ptr addrspace(1), ptr addrspace(1) %refactor_composed_callee_gep, align 8
  %refactor_composed_callee_is_null = icmp eq ptr addrspace(1) %refactor_composed_callee, null
  br i1 %refactor_composed_callee_is_null, label %resume_plain_dispatch, label %resume_composed_dispatch

resume_double:                                    ; preds = %entry
  %tracked_explicit_gc_root_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %gc_root_keepalive_4294967294 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1, align 8
  %gc_root_keepalive_4294967295 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0, align 8
  %rt_alloc_refactor_cont = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_continuation_layout__fixtures_build_fixture_visibility_hiddenWorker__type_desc, i64 72)
  %gc_root_keepalive_reload = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1, align 8
  %gc_root_keepalive_reload4 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0, align 8
  %refactor_cont_zero_field_1 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 1
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_1, align 8
  %refactor_cont_zero_field_2 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 2
  store i32 0, ptr addrspace(1) %refactor_cont_zero_field_2, align 4
  %refactor_cont_zero_field_3 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_zero_field_3, align 1
  %refactor_cont_zero_field_4 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 4
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_4, align 8
  %refactor_cont_zero_field_5 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 5
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_5, align 8
  %refactor_cont_zero_field_6 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 6
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_6, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_2, align 8
  store ptr addrspace(1) %rt_alloc_refactor_cont, ptr %explicit_root_frame_slot_2, align 8
  %refactor_cont_frame_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 1
  %gc_wb_slot_addr = addrspacecast ptr addrspace(1) %refactor_cont_frame_gep to ptr
  %tracked_explicit_gc_root_slot_08 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_19 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %tracked_explicit_gc_root_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %gc_root_keepalive_4294967293 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2, align 8
  %gc_root_keepalive_429496729410 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_19, align 8
  %gc_root_keepalive_429496729511 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_08, align 8
  %gc_write_barrier = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr, ptr addrspace(1) %gc_root_keepalive_reload4)
  %gc_root_keepalive_reload12 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2, align 8
  %gc_root_keepalive_reload13 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_19, align 8
  %gc_root_keepalive_reload14 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_08, align 8
  %refactor_cont_state_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 2
  store i32 %refactor_resume_state, ptr addrspace(1) %refactor_cont_state_gep, align 4
  %refactor_cont_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_one_shot_gep, align 1
  %refactor_cont_composed_callee_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 4
  %gc_wb_slot_addr15 = addrspacecast ptr addrspace(1) %refactor_cont_composed_callee_gep to ptr
  %tracked_explicit_gc_root_slot_016 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_117 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %tracked_explicit_gc_root_slot_218 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %gc_root_keepalive_429496729319 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_218, align 8
  %gc_root_keepalive_429496729420 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_117, align 8
  %gc_root_keepalive_429496729521 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_016, align 8
  %gc_write_barrier22 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr15, ptr addrspace(1) null)
  %gc_root_keepalive_reload23 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_218, align 8
  %gc_root_keepalive_reload24 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_117, align 8
  %gc_root_keepalive_reload25 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_016, align 8
  %refactor_step_tmp = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp, align 8
  %refactor_step_tag_gep = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, i32 0, i32 0
  store i32 2, ptr %refactor_step_tag_gep, align 4
  %refactor_step_storage_gep = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, i32 0, i32 1
  %refactor_step_cont_insert = insertvalue %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case1 { %scoop.core.RuntimeError { i32 2, i64 0, ptr addrspace(1) null }, ptr addrspace(1) undef }, ptr addrspace(1) %rt_alloc_refactor_cont, 1
  store %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case1 %refactor_step_cont_insert, ptr %refactor_step_storage_gep, align 8
  %refactor_step = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, align 8
  %explicit_root_frame_pop_prev_ptr104 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev105 = load ptr, ptr %explicit_root_frame_pop_prev_ptr104, align 8
  %explicit_root_frame_pop_slot_0106 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0106, align 8
  %explicit_root_frame_pop_slot_1107 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1107, align 8
  %explicit_root_frame_pop_slot_2108 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2108, align 8
  %explicit_root_frame_pop_slot_3109 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3109, align 8
  store ptr %explicit_root_frame_pop_prev105, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step

resume_plain_dispatch:                            ; preds = %resume_first
  switch i32 %refactor_resume_state, label %resume_invalid_state [
    i32 2, label %resume_payload_st2
  ]

resume_composed_dispatch:                         ; preds = %resume_first
  %explicit_root_frame_pop_prev_ptr110 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev111 = load ptr, ptr %explicit_root_frame_pop_prev_ptr110, align 8
  %explicit_root_frame_pop_slot_0112 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0112, align 8
  %explicit_root_frame_pop_slot_1113 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1113, align 8
  %explicit_root_frame_pop_slot_2114 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2114, align 8
  %explicit_root_frame_pop_slot_3115 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3115, align 8
  store ptr %explicit_root_frame_pop_prev111, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_invalid_state:                             ; preds = %resume_plain_dispatch
  %explicit_root_frame_pop_prev_ptr116 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev117 = load ptr, ptr %explicit_root_frame_pop_prev_ptr116, align 8
  %explicit_root_frame_pop_slot_0118 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0118, align 8
  %explicit_root_frame_pop_slot_1119 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1119, align 8
  %explicit_root_frame_pop_slot_2120 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2120, align 8
  %explicit_root_frame_pop_slot_3121 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3121, align 8
  store ptr %explicit_root_frame_pop_prev117, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_payload_st2:                               ; preds = %resume_plain_dispatch
  br label %refactor.st2
}

define %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker @__scoop_refactor_resume__fixtures_build_fixture_visibility_hiddenWorker__case1(ptr addrspace(1) %0, i8 %1) {
entry:
  %explicit_root_frame_storage = alloca ptr, i32 6, align 8
  %explicit_root_frame_prev_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_desc_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 1
  %explicit_root_frame_prev = load ptr, ptr @__scoop_explicit_root_frame_top, align 8
  store ptr %explicit_root_frame_prev, ptr %explicit_root_frame_prev_ptr, align 8
  store ptr @__scoop_explicit_root_desc____scoop_refactor_resume__fixtures_build_fixture_visibility_hiddenWorker__case1, ptr %explicit_root_frame_desc_ptr, align 8
  %explicit_root_frame_init_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_0, align 8
  %explicit_root_frame_init_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_1, align 8
  %explicit_root_frame_init_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_2, align 8
  %explicit_root_frame_init_slot_3 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_3, align 8
  store ptr %explicit_root_frame_storage, ptr @__scoop_explicit_root_frame_top, align 8
  %explicit_root_frame_slot_3 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  %explicit_root_frame_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %explicit_root_frame_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %explicit_root_frame_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_0, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_1, align 8
  store ptr addrspace(1) %0, ptr %explicit_root_frame_slot_1, align 8
  %refactor_load_frame_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %0, i32 0, i32 1
  %refactor_load_frame_gc = load ptr addrspace(1), ptr addrspace(1) %refactor_load_frame_gep, align 8
  store ptr addrspace(1) %refactor_load_frame_gc, ptr %explicit_root_frame_slot_0, align 8
  %refactor_resume_state_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %0, i32 0, i32 2
  %refactor_resume_state = load i32, ptr addrspace(1) %refactor_resume_state_gep, align 4
  %refactor_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %0, i32 0, i32 3
  %refactor_one_shot = load i1, ptr addrspace(1) %refactor_one_shot_gep, align 1
  br i1 %refactor_one_shot, label %resume_double, label %resume_first

refactor.st0:                                     ; No predecessors!
  %tracked_explicit_gc_root_slot_026 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_127 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %tracked_explicit_gc_root_slot_228 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %gc_root_keepalive_429496729329 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_228, align 8
  %gc_root_keepalive_429496729430 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_127, align 8
  %gc_root_keepalive_429496729531 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_026, align 8
  %rt_alloc_refactor_cont32 = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_continuation_layout__fixtures_build_fixture_visibility_hiddenWorker__type_desc, i64 72)
  %gc_root_keepalive_reload33 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_228, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload33, ptr poison, align 8
  %gc_root_keepalive_reload34 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_127, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload34, ptr poison, align 8
  %gc_root_keepalive_reload35 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_026, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload35, ptr poison, align 8
  %refactor_cont_zero_field_136 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 1
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_136, align 8
  %refactor_cont_zero_field_237 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 2
  store i32 0, ptr addrspace(1) %refactor_cont_zero_field_237, align 4
  %refactor_cont_zero_field_338 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_zero_field_338, align 1
  %refactor_cont_zero_field_439 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 4
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_439, align 8
  %refactor_cont_zero_field_540 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 5
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_540, align 8
  %refactor_cont_zero_field_641 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 6
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_641, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_3, align 8
  store ptr addrspace(1) %rt_alloc_refactor_cont32, ptr %explicit_root_frame_slot_3, align 8
  %refactor_frame_root.0.refactor_frame_root_reload46 = load ptr addrspace(1), ptr poison, align 8
  %refactor_cont_frame_gep47 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 1
  %gc_wb_slot_addr48 = addrspacecast ptr addrspace(1) %refactor_cont_frame_gep47 to ptr
  %tracked_explicit_gc_root_slot_049 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_150 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %tracked_explicit_gc_root_slot_251 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %tracked_explicit_gc_root_slot_3 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  %gc_root_keepalive_4294967292 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3, align 8
  %gc_root_keepalive_429496729352 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_251, align 8
  %gc_root_keepalive_429496729453 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_150, align 8
  %gc_root_keepalive_429496729554 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_049, align 8
  %gc_write_barrier55 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr48, ptr addrspace(1) %refactor_frame_root.0.refactor_frame_root_reload46)
  %gc_root_keepalive_reload56 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3, align 8
  %gc_root_keepalive_reload57 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_251, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload57, ptr poison, align 8
  %gc_root_keepalive_reload58 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_150, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload58, ptr poison, align 8
  %gc_root_keepalive_reload59 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_049, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload59, ptr poison, align 8
  %refactor_cont_state_gep60 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 2
  store i32 2, ptr addrspace(1) %refactor_cont_state_gep60, align 4
  %refactor_cont_one_shot_gep61 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_one_shot_gep61, align 1
  %refactor_cont_composed_callee_gep62 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 4
  %gc_wb_slot_addr63 = addrspacecast ptr addrspace(1) %refactor_cont_composed_callee_gep62 to ptr
  %tracked_explicit_gc_root_slot_064 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_165 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %tracked_explicit_gc_root_slot_266 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %tracked_explicit_gc_root_slot_367 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  %gc_root_keepalive_429496729268 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_367, align 8
  %gc_root_keepalive_429496729369 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_266, align 8
  %gc_root_keepalive_429496729470 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_165, align 8
  %gc_root_keepalive_429496729571 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_064, align 8
  %gc_write_barrier72 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr63, ptr addrspace(1) null)
  %gc_root_keepalive_reload73 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_367, align 8
  %gc_root_keepalive_reload74 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_266, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload74, ptr poison, align 8
  %gc_root_keepalive_reload75 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_165, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload75, ptr poison, align 8
  %gc_root_keepalive_reload76 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_064, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload76, ptr poison, align 8
  %refactor_step_tmp77 = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp77, align 8
  %refactor_step_tag_gep78 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp77, i32 0, i32 0
  store i32 1, ptr %refactor_step_tag_gep78, align 4
  %refactor_step_storage_gep79 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp77, i32 0, i32 1
  %refactor_step_cont_insert80 = insertvalue %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case0 undef, ptr addrspace(1) %rt_alloc_refactor_cont32, 0
  store %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case0 %refactor_step_cont_insert80, ptr %refactor_step_storage_gep79, align 8
  %refactor_step81 = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp77, align 8
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
  store ptr %explicit_root_frame_pop_prev, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step81

refactor.st1:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr86 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev87 = load ptr, ptr %explicit_root_frame_pop_prev_ptr86, align 8
  %explicit_root_frame_pop_slot_088 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_088, align 8
  %explicit_root_frame_pop_slot_189 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_189, align 8
  %explicit_root_frame_pop_slot_290 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_290, align 8
  %explicit_root_frame_pop_slot_391 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_391, align 8
  store ptr %explicit_root_frame_pop_prev87, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

refactor.st2:                                     ; No predecessors!
  %refactor_step_tmp82 = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp82, align 8
  %refactor_step_tag_gep83 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp82, i32 0, i32 0
  store i32 0, ptr %refactor_step_tag_gep83, align 4
  %refactor_step_storage_gep84 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp82, i32 0, i32 1
  store %scoop.refactor.StepComplete__fixtures_build_fixture_visibility_hiddenWorker undef, ptr %refactor_step_storage_gep84, align 1
  %refactor_step85 = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp82, align 8
  %explicit_root_frame_pop_prev_ptr92 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev93 = load ptr, ptr %explicit_root_frame_pop_prev_ptr92, align 8
  %explicit_root_frame_pop_slot_094 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_094, align 8
  %explicit_root_frame_pop_slot_195 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_195, align 8
  %explicit_root_frame_pop_slot_296 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_296, align 8
  %explicit_root_frame_pop_slot_397 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_397, align 8
  store ptr %explicit_root_frame_pop_prev93, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step85

refactor.st3:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr98 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev99 = load ptr, ptr %explicit_root_frame_pop_prev_ptr98, align 8
  %explicit_root_frame_pop_slot_0100 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0100, align 8
  %explicit_root_frame_pop_slot_1101 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1101, align 8
  %explicit_root_frame_pop_slot_2102 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2102, align 8
  %explicit_root_frame_pop_slot_3103 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3103, align 8
  store ptr %explicit_root_frame_pop_prev99, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_first:                                     ; preds = %entry
  %refactor_store_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %0, i32 0, i32 3
  store i1 true, ptr addrspace(1) %refactor_store_one_shot_gep, align 1
  %refactor_composed_callee_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %0, i32 0, i32 4
  %refactor_composed_callee = load ptr addrspace(1), ptr addrspace(1) %refactor_composed_callee_gep, align 8
  %refactor_composed_callee_is_null = icmp eq ptr addrspace(1) %refactor_composed_callee, null
  br i1 %refactor_composed_callee_is_null, label %resume_plain_dispatch, label %resume_composed_dispatch

resume_double:                                    ; preds = %entry
  %tracked_explicit_gc_root_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %gc_root_keepalive_4294967294 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1, align 8
  %gc_root_keepalive_4294967295 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0, align 8
  %rt_alloc_refactor_cont = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_continuation_layout__fixtures_build_fixture_visibility_hiddenWorker__type_desc, i64 72)
  %gc_root_keepalive_reload = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1, align 8
  %gc_root_keepalive_reload4 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0, align 8
  %refactor_cont_zero_field_1 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 1
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_1, align 8
  %refactor_cont_zero_field_2 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 2
  store i32 0, ptr addrspace(1) %refactor_cont_zero_field_2, align 4
  %refactor_cont_zero_field_3 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_zero_field_3, align 1
  %refactor_cont_zero_field_4 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 4
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_4, align 8
  %refactor_cont_zero_field_5 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 5
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_5, align 8
  %refactor_cont_zero_field_6 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 6
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_6, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_2, align 8
  store ptr addrspace(1) %rt_alloc_refactor_cont, ptr %explicit_root_frame_slot_2, align 8
  %refactor_cont_frame_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 1
  %gc_wb_slot_addr = addrspacecast ptr addrspace(1) %refactor_cont_frame_gep to ptr
  %tracked_explicit_gc_root_slot_08 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_19 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %tracked_explicit_gc_root_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %gc_root_keepalive_4294967293 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2, align 8
  %gc_root_keepalive_429496729410 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_19, align 8
  %gc_root_keepalive_429496729511 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_08, align 8
  %gc_write_barrier = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr, ptr addrspace(1) %gc_root_keepalive_reload4)
  %gc_root_keepalive_reload12 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2, align 8
  %gc_root_keepalive_reload13 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_19, align 8
  %gc_root_keepalive_reload14 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_08, align 8
  %refactor_cont_state_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 2
  store i32 %refactor_resume_state, ptr addrspace(1) %refactor_cont_state_gep, align 4
  %refactor_cont_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_one_shot_gep, align 1
  %refactor_cont_composed_callee_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 4
  %gc_wb_slot_addr15 = addrspacecast ptr addrspace(1) %refactor_cont_composed_callee_gep to ptr
  %tracked_explicit_gc_root_slot_016 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_117 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %tracked_explicit_gc_root_slot_218 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %gc_root_keepalive_429496729319 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_218, align 8
  %gc_root_keepalive_429496729420 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_117, align 8
  %gc_root_keepalive_429496729521 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_016, align 8
  %gc_write_barrier22 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr15, ptr addrspace(1) null)
  %gc_root_keepalive_reload23 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_218, align 8
  %gc_root_keepalive_reload24 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_117, align 8
  %gc_root_keepalive_reload25 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_016, align 8
  %refactor_step_tmp = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp, align 8
  %refactor_step_tag_gep = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, i32 0, i32 0
  store i32 2, ptr %refactor_step_tag_gep, align 4
  %refactor_step_storage_gep = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, i32 0, i32 1
  %refactor_step_cont_insert = insertvalue %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case1 { %scoop.core.RuntimeError { i32 2, i64 0, ptr addrspace(1) null }, ptr addrspace(1) undef }, ptr addrspace(1) %rt_alloc_refactor_cont, 1
  store %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case1 %refactor_step_cont_insert, ptr %refactor_step_storage_gep, align 8
  %refactor_step = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, align 8
  %explicit_root_frame_pop_prev_ptr104 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev105 = load ptr, ptr %explicit_root_frame_pop_prev_ptr104, align 8
  %explicit_root_frame_pop_slot_0106 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0106, align 8
  %explicit_root_frame_pop_slot_1107 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1107, align 8
  %explicit_root_frame_pop_slot_2108 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2108, align 8
  %explicit_root_frame_pop_slot_3109 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3109, align 8
  store ptr %explicit_root_frame_pop_prev105, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step

resume_plain_dispatch:                            ; preds = %resume_first
  switch i32 %refactor_resume_state, label %resume_invalid_state [
  ]

resume_composed_dispatch:                         ; preds = %resume_first
  %explicit_root_frame_pop_prev_ptr110 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev111 = load ptr, ptr %explicit_root_frame_pop_prev_ptr110, align 8
  %explicit_root_frame_pop_slot_0112 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0112, align 8
  %explicit_root_frame_pop_slot_1113 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1113, align 8
  %explicit_root_frame_pop_slot_2114 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2114, align 8
  %explicit_root_frame_pop_slot_3115 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3115, align 8
  store ptr %explicit_root_frame_pop_prev111, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_invalid_state:                             ; preds = %resume_plain_dispatch
  %explicit_root_frame_pop_prev_ptr116 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev117 = load ptr, ptr %explicit_root_frame_pop_prev_ptr116, align 8
  %explicit_root_frame_pop_slot_0118 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0118, align 8
  %explicit_root_frame_pop_slot_1119 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1119, align 8
  %explicit_root_frame_pop_slot_2120 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2120, align 8
  %explicit_root_frame_pop_slot_3121 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3121, align 8
  store ptr %explicit_root_frame_pop_prev117, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable
}

define %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker @__scoop_refactor_surface_resume__k0(ptr addrspace(1) %0) {
entry:
  %refactor_surface_resume_call = call %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker @__scoop_refactor_surface_resume_owner_dispatch__fixtures_build_fixture_visibility_hiddenWorker__k0(ptr addrspace(1) %0)
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_surface_resume_call
}

define %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker @__scoop_refactor_surface_resume__k1(ptr addrspace(1) %0, i8 %1) {
entry:
  %refactor_surface_resume_call = call %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker @__scoop_refactor_surface_resume_owner_dispatch__fixtures_build_fixture_visibility_hiddenWorker__k1(ptr addrspace(1) %0, i8 %1)
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_surface_resume_call
}

declare %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker @__scoop_refactor_dynamic_invoke__fixtures_build_fixture_visibility_hiddenWorker()

declare %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker @__scoop_refactor_direct_invoke__fixtures_build_fixture_visibility_hiddenWorker()

define i64 @fixtures.build_fixture_visibility.main() {
entry:
  %explicit_root_frame_storage = alloca ptr, i32 2, align 8
  %explicit_root_frame_prev_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_desc_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 1
  %explicit_root_frame_prev = load ptr, ptr @__scoop_explicit_root_frame_top, align 8
  store ptr %explicit_root_frame_prev, ptr %explicit_root_frame_prev_ptr, align 8
  store ptr @__scoop_explicit_root_desc__fixtures_build_fixture_visibility_main, ptr %explicit_root_frame_desc_ptr, align 8
  store ptr %explicit_root_frame_storage, ptr @__scoop_explicit_root_frame_top, align 8
  br label %plain.bb0

return:                                           ; preds = %plain.bb0
  %explicit_root_frame_pop_prev_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev = load ptr, ptr %explicit_root_frame_pop_prev_ptr, align 8
  store ptr %explicit_root_frame_pop_prev, ptr @__scoop_explicit_root_frame_top, align 8
  ret i64 0

plain.bb0:                                        ; preds = %entry
  br label %return
}

define %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker @__scoop_refactor_surface_resume_owner_dispatch__fixtures_build_fixture_visibility_hiddenWorker__k0(ptr addrspace(1) %0) {
entry:
  %explicit_root_frame_storage = alloca ptr, i32 6, align 8
  %explicit_root_frame_prev_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_desc_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 1
  %explicit_root_frame_prev = load ptr, ptr @__scoop_explicit_root_frame_top, align 8
  store ptr %explicit_root_frame_prev, ptr %explicit_root_frame_prev_ptr, align 8
  store ptr @__scoop_explicit_root_desc____scoop_refactor_surface_resume_owner_dispatch__fixtures_build_fixture_visibility_hiddenWorker__k0, ptr %explicit_root_frame_desc_ptr, align 8
  %explicit_root_frame_init_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_0, align 8
  %explicit_root_frame_init_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_1, align 8
  %explicit_root_frame_init_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_2, align 8
  %explicit_root_frame_init_slot_3 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_3, align 8
  store ptr %explicit_root_frame_storage, ptr @__scoop_explicit_root_frame_top, align 8
  %explicit_root_frame_slot_3 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  %explicit_root_frame_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %explicit_root_frame_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %explicit_root_frame_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_0, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_1, align 8
  store ptr addrspace(1) %0, ptr %explicit_root_frame_slot_1, align 8
  %refactor_load_frame_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %0, i32 0, i32 1
  %refactor_load_frame_gc = load ptr addrspace(1), ptr addrspace(1) %refactor_load_frame_gep, align 8
  store ptr addrspace(1) %refactor_load_frame_gc, ptr %explicit_root_frame_slot_0, align 8
  %refactor_resume_state_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %0, i32 0, i32 2
  %refactor_resume_state = load i32, ptr addrspace(1) %refactor_resume_state_gep, align 4
  %refactor_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %0, i32 0, i32 3
  %refactor_one_shot = load i1, ptr addrspace(1) %refactor_one_shot_gep, align 1
  br i1 %refactor_one_shot, label %resume_double, label %resume_first

refactor.st0:                                     ; No predecessors!
  %tracked_explicit_gc_root_slot_026 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_127 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %tracked_explicit_gc_root_slot_228 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %gc_root_keepalive_429496729329 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_228, align 8
  %gc_root_keepalive_429496729430 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_127, align 8
  %gc_root_keepalive_429496729531 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_026, align 8
  %rt_alloc_refactor_cont32 = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_continuation_layout__fixtures_build_fixture_visibility_hiddenWorker__type_desc, i64 72)
  %gc_root_keepalive_reload33 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_228, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload33, ptr poison, align 8
  %gc_root_keepalive_reload34 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_127, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload34, ptr poison, align 8
  %gc_root_keepalive_reload35 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_026, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload35, ptr poison, align 8
  %refactor_cont_zero_field_136 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 1
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_136, align 8
  %refactor_cont_zero_field_237 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 2
  store i32 0, ptr addrspace(1) %refactor_cont_zero_field_237, align 4
  %refactor_cont_zero_field_338 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_zero_field_338, align 1
  %refactor_cont_zero_field_439 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 4
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_439, align 8
  %refactor_cont_zero_field_540 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 5
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_540, align 8
  %refactor_cont_zero_field_641 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 6
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_641, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_3, align 8
  store ptr addrspace(1) %rt_alloc_refactor_cont32, ptr %explicit_root_frame_slot_3, align 8
  %refactor_frame_root.0.refactor_frame_root_reload46 = load ptr addrspace(1), ptr poison, align 8
  %refactor_cont_frame_gep47 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 1
  %gc_wb_slot_addr48 = addrspacecast ptr addrspace(1) %refactor_cont_frame_gep47 to ptr
  %tracked_explicit_gc_root_slot_049 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_150 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %tracked_explicit_gc_root_slot_251 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %tracked_explicit_gc_root_slot_3 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  %gc_root_keepalive_4294967292 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3, align 8
  %gc_root_keepalive_429496729352 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_251, align 8
  %gc_root_keepalive_429496729453 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_150, align 8
  %gc_root_keepalive_429496729554 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_049, align 8
  %gc_write_barrier55 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr48, ptr addrspace(1) %refactor_frame_root.0.refactor_frame_root_reload46)
  %gc_root_keepalive_reload56 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3, align 8
  %gc_root_keepalive_reload57 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_251, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload57, ptr poison, align 8
  %gc_root_keepalive_reload58 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_150, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload58, ptr poison, align 8
  %gc_root_keepalive_reload59 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_049, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload59, ptr poison, align 8
  %refactor_cont_state_gep60 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 2
  store i32 2, ptr addrspace(1) %refactor_cont_state_gep60, align 4
  %refactor_cont_one_shot_gep61 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_one_shot_gep61, align 1
  %refactor_cont_composed_callee_gep62 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 4
  %gc_wb_slot_addr63 = addrspacecast ptr addrspace(1) %refactor_cont_composed_callee_gep62 to ptr
  %tracked_explicit_gc_root_slot_064 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_165 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %tracked_explicit_gc_root_slot_266 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %tracked_explicit_gc_root_slot_367 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  %gc_root_keepalive_429496729268 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_367, align 8
  %gc_root_keepalive_429496729369 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_266, align 8
  %gc_root_keepalive_429496729470 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_165, align 8
  %gc_root_keepalive_429496729571 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_064, align 8
  %gc_write_barrier72 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr63, ptr addrspace(1) null)
  %gc_root_keepalive_reload73 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_367, align 8
  %gc_root_keepalive_reload74 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_266, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload74, ptr poison, align 8
  %gc_root_keepalive_reload75 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_165, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload75, ptr poison, align 8
  %gc_root_keepalive_reload76 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_064, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload76, ptr poison, align 8
  %refactor_step_tmp77 = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp77, align 8
  %refactor_step_tag_gep78 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp77, i32 0, i32 0
  store i32 1, ptr %refactor_step_tag_gep78, align 4
  %refactor_step_storage_gep79 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp77, i32 0, i32 1
  %refactor_step_cont_insert80 = insertvalue %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case0 undef, ptr addrspace(1) %rt_alloc_refactor_cont32, 0
  store %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case0 %refactor_step_cont_insert80, ptr %refactor_step_storage_gep79, align 8
  %refactor_step81 = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp77, align 8
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
  store ptr %explicit_root_frame_pop_prev, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step81

refactor.st1:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr86 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev87 = load ptr, ptr %explicit_root_frame_pop_prev_ptr86, align 8
  %explicit_root_frame_pop_slot_088 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_088, align 8
  %explicit_root_frame_pop_slot_189 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_189, align 8
  %explicit_root_frame_pop_slot_290 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_290, align 8
  %explicit_root_frame_pop_slot_391 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_391, align 8
  store ptr %explicit_root_frame_pop_prev87, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

refactor.st2:                                     ; preds = %resume_payload_st2
  %refactor_step_tmp82 = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp82, align 8
  %refactor_step_tag_gep83 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp82, i32 0, i32 0
  store i32 0, ptr %refactor_step_tag_gep83, align 4
  %refactor_step_storage_gep84 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp82, i32 0, i32 1
  store %scoop.refactor.StepComplete__fixtures_build_fixture_visibility_hiddenWorker undef, ptr %refactor_step_storage_gep84, align 1
  %refactor_step85 = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp82, align 8
  %explicit_root_frame_pop_prev_ptr92 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev93 = load ptr, ptr %explicit_root_frame_pop_prev_ptr92, align 8
  %explicit_root_frame_pop_slot_094 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_094, align 8
  %explicit_root_frame_pop_slot_195 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_195, align 8
  %explicit_root_frame_pop_slot_296 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_296, align 8
  %explicit_root_frame_pop_slot_397 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_397, align 8
  store ptr %explicit_root_frame_pop_prev93, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step85

refactor.st3:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr98 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev99 = load ptr, ptr %explicit_root_frame_pop_prev_ptr98, align 8
  %explicit_root_frame_pop_slot_0100 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0100, align 8
  %explicit_root_frame_pop_slot_1101 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1101, align 8
  %explicit_root_frame_pop_slot_2102 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2102, align 8
  %explicit_root_frame_pop_slot_3103 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3103, align 8
  store ptr %explicit_root_frame_pop_prev99, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_first:                                     ; preds = %entry
  %refactor_store_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %0, i32 0, i32 3
  store i1 true, ptr addrspace(1) %refactor_store_one_shot_gep, align 1
  %refactor_composed_callee_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %0, i32 0, i32 4
  %refactor_composed_callee = load ptr addrspace(1), ptr addrspace(1) %refactor_composed_callee_gep, align 8
  %refactor_composed_callee_is_null = icmp eq ptr addrspace(1) %refactor_composed_callee, null
  br i1 %refactor_composed_callee_is_null, label %resume_plain_dispatch, label %resume_composed_dispatch

resume_double:                                    ; preds = %entry
  %tracked_explicit_gc_root_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %gc_root_keepalive_4294967294 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1, align 8
  %gc_root_keepalive_4294967295 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0, align 8
  %rt_alloc_refactor_cont = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_continuation_layout__fixtures_build_fixture_visibility_hiddenWorker__type_desc, i64 72)
  %gc_root_keepalive_reload = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1, align 8
  %gc_root_keepalive_reload4 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0, align 8
  %refactor_cont_zero_field_1 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 1
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_1, align 8
  %refactor_cont_zero_field_2 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 2
  store i32 0, ptr addrspace(1) %refactor_cont_zero_field_2, align 4
  %refactor_cont_zero_field_3 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_zero_field_3, align 1
  %refactor_cont_zero_field_4 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 4
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_4, align 8
  %refactor_cont_zero_field_5 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 5
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_5, align 8
  %refactor_cont_zero_field_6 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 6
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_6, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_2, align 8
  store ptr addrspace(1) %rt_alloc_refactor_cont, ptr %explicit_root_frame_slot_2, align 8
  %refactor_cont_frame_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 1
  %gc_wb_slot_addr = addrspacecast ptr addrspace(1) %refactor_cont_frame_gep to ptr
  %tracked_explicit_gc_root_slot_08 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_19 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %tracked_explicit_gc_root_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %gc_root_keepalive_4294967293 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2, align 8
  %gc_root_keepalive_429496729410 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_19, align 8
  %gc_root_keepalive_429496729511 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_08, align 8
  %gc_write_barrier = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr, ptr addrspace(1) %gc_root_keepalive_reload4)
  %gc_root_keepalive_reload12 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2, align 8
  %gc_root_keepalive_reload13 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_19, align 8
  %gc_root_keepalive_reload14 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_08, align 8
  %refactor_cont_state_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 2
  store i32 %refactor_resume_state, ptr addrspace(1) %refactor_cont_state_gep, align 4
  %refactor_cont_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_one_shot_gep, align 1
  %refactor_cont_composed_callee_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 4
  %gc_wb_slot_addr15 = addrspacecast ptr addrspace(1) %refactor_cont_composed_callee_gep to ptr
  %tracked_explicit_gc_root_slot_016 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_117 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %tracked_explicit_gc_root_slot_218 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %gc_root_keepalive_429496729319 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_218, align 8
  %gc_root_keepalive_429496729420 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_117, align 8
  %gc_root_keepalive_429496729521 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_016, align 8
  %gc_write_barrier22 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr15, ptr addrspace(1) null)
  %gc_root_keepalive_reload23 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_218, align 8
  %gc_root_keepalive_reload24 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_117, align 8
  %gc_root_keepalive_reload25 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_016, align 8
  %refactor_step_tmp = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp, align 8
  %refactor_step_tag_gep = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, i32 0, i32 0
  store i32 2, ptr %refactor_step_tag_gep, align 4
  %refactor_step_storage_gep = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, i32 0, i32 1
  %refactor_step_cont_insert = insertvalue %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case1 { %scoop.core.RuntimeError { i32 2, i64 0, ptr addrspace(1) null }, ptr addrspace(1) undef }, ptr addrspace(1) %rt_alloc_refactor_cont, 1
  store %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case1 %refactor_step_cont_insert, ptr %refactor_step_storage_gep, align 8
  %refactor_step = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, align 8
  %explicit_root_frame_pop_prev_ptr104 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev105 = load ptr, ptr %explicit_root_frame_pop_prev_ptr104, align 8
  %explicit_root_frame_pop_slot_0106 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0106, align 8
  %explicit_root_frame_pop_slot_1107 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1107, align 8
  %explicit_root_frame_pop_slot_2108 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2108, align 8
  %explicit_root_frame_pop_slot_3109 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3109, align 8
  store ptr %explicit_root_frame_pop_prev105, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step

resume_plain_dispatch:                            ; preds = %resume_first
  switch i32 %refactor_resume_state, label %resume_invalid_state [
    i32 2, label %resume_payload_st2
  ]

resume_composed_dispatch:                         ; preds = %resume_first
  %explicit_root_frame_pop_prev_ptr110 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev111 = load ptr, ptr %explicit_root_frame_pop_prev_ptr110, align 8
  %explicit_root_frame_pop_slot_0112 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0112, align 8
  %explicit_root_frame_pop_slot_1113 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1113, align 8
  %explicit_root_frame_pop_slot_2114 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2114, align 8
  %explicit_root_frame_pop_slot_3115 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3115, align 8
  store ptr %explicit_root_frame_pop_prev111, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_invalid_state:                             ; preds = %resume_plain_dispatch
  %explicit_root_frame_pop_prev_ptr116 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev117 = load ptr, ptr %explicit_root_frame_pop_prev_ptr116, align 8
  %explicit_root_frame_pop_slot_0118 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0118, align 8
  %explicit_root_frame_pop_slot_1119 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1119, align 8
  %explicit_root_frame_pop_slot_2120 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2120, align 8
  %explicit_root_frame_pop_slot_3121 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3121, align 8
  store ptr %explicit_root_frame_pop_prev117, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_payload_st2:                               ; preds = %resume_plain_dispatch
  br label %refactor.st2
}

define %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker @__scoop_refactor_surface_resume_owner_dispatch__fixtures_build_fixture_visibility_hiddenWorker__k1(ptr addrspace(1) %0, i8 %1) {
entry:
  %explicit_root_frame_storage = alloca ptr, i32 6, align 8
  %explicit_root_frame_prev_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_desc_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 1
  %explicit_root_frame_prev = load ptr, ptr @__scoop_explicit_root_frame_top, align 8
  store ptr %explicit_root_frame_prev, ptr %explicit_root_frame_prev_ptr, align 8
  store ptr @__scoop_explicit_root_desc____scoop_refactor_surface_resume_owner_dispatch__fixtures_build_fixture_visibility_hiddenWorker__k1, ptr %explicit_root_frame_desc_ptr, align 8
  %explicit_root_frame_init_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_0, align 8
  %explicit_root_frame_init_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_1, align 8
  %explicit_root_frame_init_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_2, align 8
  %explicit_root_frame_init_slot_3 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_init_slot_3, align 8
  store ptr %explicit_root_frame_storage, ptr @__scoop_explicit_root_frame_top, align 8
  %explicit_root_frame_slot_3 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  %explicit_root_frame_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %explicit_root_frame_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %explicit_root_frame_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_0, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_1, align 8
  store ptr addrspace(1) %0, ptr %explicit_root_frame_slot_1, align 8
  %refactor_load_frame_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %0, i32 0, i32 1
  %refactor_load_frame_gc = load ptr addrspace(1), ptr addrspace(1) %refactor_load_frame_gep, align 8
  store ptr addrspace(1) %refactor_load_frame_gc, ptr %explicit_root_frame_slot_0, align 8
  %refactor_resume_state_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %0, i32 0, i32 2
  %refactor_resume_state = load i32, ptr addrspace(1) %refactor_resume_state_gep, align 4
  %refactor_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %0, i32 0, i32 3
  %refactor_one_shot = load i1, ptr addrspace(1) %refactor_one_shot_gep, align 1
  br i1 %refactor_one_shot, label %resume_double, label %resume_first

refactor.st0:                                     ; No predecessors!
  %tracked_explicit_gc_root_slot_026 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_127 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %tracked_explicit_gc_root_slot_228 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %gc_root_keepalive_429496729329 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_228, align 8
  %gc_root_keepalive_429496729430 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_127, align 8
  %gc_root_keepalive_429496729531 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_026, align 8
  %rt_alloc_refactor_cont32 = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_continuation_layout__fixtures_build_fixture_visibility_hiddenWorker__type_desc, i64 72)
  %gc_root_keepalive_reload33 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_228, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload33, ptr poison, align 8
  %gc_root_keepalive_reload34 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_127, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload34, ptr poison, align 8
  %gc_root_keepalive_reload35 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_026, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload35, ptr poison, align 8
  %refactor_cont_zero_field_136 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 1
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_136, align 8
  %refactor_cont_zero_field_237 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 2
  store i32 0, ptr addrspace(1) %refactor_cont_zero_field_237, align 4
  %refactor_cont_zero_field_338 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_zero_field_338, align 1
  %refactor_cont_zero_field_439 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 4
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_439, align 8
  %refactor_cont_zero_field_540 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 5
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_540, align 8
  %refactor_cont_zero_field_641 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 6
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_641, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_3, align 8
  store ptr addrspace(1) %rt_alloc_refactor_cont32, ptr %explicit_root_frame_slot_3, align 8
  %refactor_frame_root.0.refactor_frame_root_reload46 = load ptr addrspace(1), ptr poison, align 8
  %refactor_cont_frame_gep47 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 1
  %gc_wb_slot_addr48 = addrspacecast ptr addrspace(1) %refactor_cont_frame_gep47 to ptr
  %tracked_explicit_gc_root_slot_049 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_150 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %tracked_explicit_gc_root_slot_251 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %tracked_explicit_gc_root_slot_3 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  %gc_root_keepalive_4294967292 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3, align 8
  %gc_root_keepalive_429496729352 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_251, align 8
  %gc_root_keepalive_429496729453 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_150, align 8
  %gc_root_keepalive_429496729554 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_049, align 8
  %gc_write_barrier55 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr48, ptr addrspace(1) %refactor_frame_root.0.refactor_frame_root_reload46)
  %gc_root_keepalive_reload56 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_3, align 8
  %gc_root_keepalive_reload57 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_251, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload57, ptr poison, align 8
  %gc_root_keepalive_reload58 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_150, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload58, ptr poison, align 8
  %gc_root_keepalive_reload59 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_049, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload59, ptr poison, align 8
  %refactor_cont_state_gep60 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 2
  store i32 2, ptr addrspace(1) %refactor_cont_state_gep60, align 4
  %refactor_cont_one_shot_gep61 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_one_shot_gep61, align 1
  %refactor_cont_composed_callee_gep62 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont32, i32 0, i32 4
  %gc_wb_slot_addr63 = addrspacecast ptr addrspace(1) %refactor_cont_composed_callee_gep62 to ptr
  %tracked_explicit_gc_root_slot_064 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_165 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %tracked_explicit_gc_root_slot_266 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %tracked_explicit_gc_root_slot_367 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  %gc_root_keepalive_429496729268 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_367, align 8
  %gc_root_keepalive_429496729369 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_266, align 8
  %gc_root_keepalive_429496729470 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_165, align 8
  %gc_root_keepalive_429496729571 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_064, align 8
  %gc_write_barrier72 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr63, ptr addrspace(1) null)
  %gc_root_keepalive_reload73 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_367, align 8
  %gc_root_keepalive_reload74 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_266, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload74, ptr poison, align 8
  %gc_root_keepalive_reload75 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_165, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload75, ptr poison, align 8
  %gc_root_keepalive_reload76 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_064, align 8
  store ptr addrspace(1) %gc_root_keepalive_reload76, ptr poison, align 8
  %refactor_step_tmp77 = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp77, align 8
  %refactor_step_tag_gep78 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp77, i32 0, i32 0
  store i32 1, ptr %refactor_step_tag_gep78, align 4
  %refactor_step_storage_gep79 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp77, i32 0, i32 1
  %refactor_step_cont_insert80 = insertvalue %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case0 undef, ptr addrspace(1) %rt_alloc_refactor_cont32, 0
  store %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case0 %refactor_step_cont_insert80, ptr %refactor_step_storage_gep79, align 8
  %refactor_step81 = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp77, align 8
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
  store ptr %explicit_root_frame_pop_prev, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step81

refactor.st1:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr86 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev87 = load ptr, ptr %explicit_root_frame_pop_prev_ptr86, align 8
  %explicit_root_frame_pop_slot_088 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_088, align 8
  %explicit_root_frame_pop_slot_189 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_189, align 8
  %explicit_root_frame_pop_slot_290 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_290, align 8
  %explicit_root_frame_pop_slot_391 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_391, align 8
  store ptr %explicit_root_frame_pop_prev87, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

refactor.st2:                                     ; No predecessors!
  %refactor_step_tmp82 = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp82, align 8
  %refactor_step_tag_gep83 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp82, i32 0, i32 0
  store i32 0, ptr %refactor_step_tag_gep83, align 4
  %refactor_step_storage_gep84 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp82, i32 0, i32 1
  store %scoop.refactor.StepComplete__fixtures_build_fixture_visibility_hiddenWorker undef, ptr %refactor_step_storage_gep84, align 1
  %refactor_step85 = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp82, align 8
  %explicit_root_frame_pop_prev_ptr92 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev93 = load ptr, ptr %explicit_root_frame_pop_prev_ptr92, align 8
  %explicit_root_frame_pop_slot_094 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_094, align 8
  %explicit_root_frame_pop_slot_195 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_195, align 8
  %explicit_root_frame_pop_slot_296 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_296, align 8
  %explicit_root_frame_pop_slot_397 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_397, align 8
  store ptr %explicit_root_frame_pop_prev93, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step85

refactor.st3:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr98 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev99 = load ptr, ptr %explicit_root_frame_pop_prev_ptr98, align 8
  %explicit_root_frame_pop_slot_0100 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0100, align 8
  %explicit_root_frame_pop_slot_1101 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1101, align 8
  %explicit_root_frame_pop_slot_2102 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2102, align 8
  %explicit_root_frame_pop_slot_3103 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3103, align 8
  store ptr %explicit_root_frame_pop_prev99, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_first:                                     ; preds = %entry
  %refactor_store_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %0, i32 0, i32 3
  store i1 true, ptr addrspace(1) %refactor_store_one_shot_gep, align 1
  %refactor_composed_callee_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %0, i32 0, i32 4
  %refactor_composed_callee = load ptr addrspace(1), ptr addrspace(1) %refactor_composed_callee_gep, align 8
  %refactor_composed_callee_is_null = icmp eq ptr addrspace(1) %refactor_composed_callee, null
  br i1 %refactor_composed_callee_is_null, label %resume_plain_dispatch, label %resume_composed_dispatch

resume_double:                                    ; preds = %entry
  %tracked_explicit_gc_root_slot_0 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_1 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %gc_root_keepalive_4294967294 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1, align 8
  %gc_root_keepalive_4294967295 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0, align 8
  %rt_alloc_refactor_cont = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_continuation_layout__fixtures_build_fixture_visibility_hiddenWorker__type_desc, i64 72)
  %gc_root_keepalive_reload = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_1, align 8
  %gc_root_keepalive_reload4 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_0, align 8
  %refactor_cont_zero_field_1 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 1
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_1, align 8
  %refactor_cont_zero_field_2 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 2
  store i32 0, ptr addrspace(1) %refactor_cont_zero_field_2, align 4
  %refactor_cont_zero_field_3 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_zero_field_3, align 1
  %refactor_cont_zero_field_4 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 4
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_4, align 8
  %refactor_cont_zero_field_5 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 5
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_5, align 8
  %refactor_cont_zero_field_6 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 6
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_6, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_2, align 8
  store ptr addrspace(1) %rt_alloc_refactor_cont, ptr %explicit_root_frame_slot_2, align 8
  %refactor_cont_frame_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 1
  %gc_wb_slot_addr = addrspacecast ptr addrspace(1) %refactor_cont_frame_gep to ptr
  %tracked_explicit_gc_root_slot_08 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_19 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %tracked_explicit_gc_root_slot_2 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %gc_root_keepalive_4294967293 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2, align 8
  %gc_root_keepalive_429496729410 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_19, align 8
  %gc_root_keepalive_429496729511 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_08, align 8
  %gc_write_barrier = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr, ptr addrspace(1) %gc_root_keepalive_reload4)
  %gc_root_keepalive_reload12 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_2, align 8
  %gc_root_keepalive_reload13 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_19, align 8
  %gc_root_keepalive_reload14 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_08, align 8
  %refactor_cont_state_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 2
  store i32 %refactor_resume_state, ptr addrspace(1) %refactor_cont_state_gep, align 4
  %refactor_cont_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_one_shot_gep, align 1
  %refactor_cont_composed_callee_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont, i32 0, i32 4
  %gc_wb_slot_addr15 = addrspacecast ptr addrspace(1) %refactor_cont_composed_callee_gep to ptr
  %tracked_explicit_gc_root_slot_016 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  %tracked_explicit_gc_root_slot_117 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  %tracked_explicit_gc_root_slot_218 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  %gc_root_keepalive_429496729319 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_218, align 8
  %gc_root_keepalive_429496729420 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_117, align 8
  %gc_root_keepalive_429496729521 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_016, align 8
  %gc_write_barrier22 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr15, ptr addrspace(1) null)
  %gc_root_keepalive_reload23 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_218, align 8
  %gc_root_keepalive_reload24 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_117, align 8
  %gc_root_keepalive_reload25 = load ptr addrspace(1), ptr %tracked_explicit_gc_root_slot_016, align 8
  %refactor_step_tmp = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp, align 8
  %refactor_step_tag_gep = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, i32 0, i32 0
  store i32 2, ptr %refactor_step_tag_gep, align 4
  %refactor_step_storage_gep = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, i32 0, i32 1
  %refactor_step_cont_insert = insertvalue %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case1 { %scoop.core.RuntimeError { i32 2, i64 0, ptr addrspace(1) null }, ptr addrspace(1) undef }, ptr addrspace(1) %rt_alloc_refactor_cont, 1
  store %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case1 %refactor_step_cont_insert, ptr %refactor_step_storage_gep, align 8
  %refactor_step = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, align 8
  %explicit_root_frame_pop_prev_ptr104 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev105 = load ptr, ptr %explicit_root_frame_pop_prev_ptr104, align 8
  %explicit_root_frame_pop_slot_0106 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0106, align 8
  %explicit_root_frame_pop_slot_1107 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1107, align 8
  %explicit_root_frame_pop_slot_2108 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2108, align 8
  %explicit_root_frame_pop_slot_3109 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3109, align 8
  store ptr %explicit_root_frame_pop_prev105, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step

resume_plain_dispatch:                            ; preds = %resume_first
  switch i32 %refactor_resume_state, label %resume_invalid_state [
  ]

resume_composed_dispatch:                         ; preds = %resume_first
  %explicit_root_frame_pop_prev_ptr110 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev111 = load ptr, ptr %explicit_root_frame_pop_prev_ptr110, align 8
  %explicit_root_frame_pop_slot_0112 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0112, align 8
  %explicit_root_frame_pop_slot_1113 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1113, align 8
  %explicit_root_frame_pop_slot_2114 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2114, align 8
  %explicit_root_frame_pop_slot_3115 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3115, align 8
  store ptr %explicit_root_frame_pop_prev111, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_invalid_state:                             ; preds = %resume_plain_dispatch
  %explicit_root_frame_pop_prev_ptr116 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev117 = load ptr, ptr %explicit_root_frame_pop_prev_ptr116, align 8
  %explicit_root_frame_pop_slot_0118 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_0118, align 8
  %explicit_root_frame_pop_slot_1119 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_1119, align 8
  %explicit_root_frame_pop_slot_2120 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_2120, align 8
  %explicit_root_frame_pop_slot_3121 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_3121, align 8
  store ptr %explicit_root_frame_pop_prev117, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable
}

define %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker @__scoop_refactor_closure_dynamic_entry__fixtures_build_fixture_visibility_hiddenWorker(ptr addrspace(1) %0) {
entry:
  %refactor_carrier_to_direct = call %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker @__scoop_refactor_direct_invoke__fixtures_build_fixture_visibility_hiddenWorker()
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_carrier_to_direct
}

declare ptr addrspace(1) @scoop_alloc_typed(ptr, i64)

declare ptr addrspace(1) @scoop_gc_write_barrier(ptr, ptr addrspace(1))

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
  %refactor_plain_main = call i64 @fixtures.build_fixture_visibility.main()
  %refactor_plain_main_exit_i32 = trunc i64 %refactor_plain_main to i32
  %explicit_root_frame_pop_prev_ptr = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev = load ptr, ptr %explicit_root_frame_pop_prev_ptr, align 8
  store ptr %explicit_root_frame_pop_prev, ptr @__scoop_explicit_root_frame_top, align 8
  ret i32 %refactor_plain_main_exit_i32
}

declare void @scoop_runtime_init()
