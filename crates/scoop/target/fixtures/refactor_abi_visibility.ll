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
  %refactor_resume_cont_root_reload = load ptr addrspace(1), ptr %explicit_root_frame_slot_1, align 8
  %refactor_load_frame_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 1
  %refactor_load_frame_gc = load ptr addrspace(1), ptr addrspace(1) %refactor_load_frame_gep, align 8
  store ptr addrspace(1) %refactor_load_frame_gc, ptr %explicit_root_frame_slot_0, align 8
  %refactor_resume_state_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 2
  %refactor_resume_state = load i32, ptr addrspace(1) %refactor_resume_state_gep, align 4
  %refactor_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 3
  %refactor_one_shot = load i1, ptr addrspace(1) %refactor_one_shot_gep, align 1
  br i1 %refactor_one_shot, label %resume_double, label %resume_first

refactor.st0:                                     ; No predecessors!
  %rt_alloc_refactor_cont3 = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_continuation_layout__fixtures_build_fixture_visibility_hiddenWorker__type_desc, i64 72)
  %refactor_cont_zero_field_14 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 1
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_14, align 8
  %refactor_cont_zero_field_25 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 2
  store i32 0, ptr addrspace(1) %refactor_cont_zero_field_25, align 4
  %refactor_cont_zero_field_36 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_zero_field_36, align 1
  %refactor_cont_zero_field_47 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 4
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_47, align 8
  %refactor_cont_zero_field_58 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 5
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_58, align 8
  %refactor_cont_zero_field_69 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 6
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_69, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_3, align 8
  store ptr addrspace(1) %rt_alloc_refactor_cont3, ptr %explicit_root_frame_slot_3, align 8
  %refactor_cont_root_reload11 = load ptr addrspace(1), ptr %explicit_root_frame_slot_3, align 8
  %refactor_frame_root_reload12 = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  %refactor_cont_frame_gep13 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload11, i32 0, i32 1
  %gc_wb_slot_addr14 = addrspacecast ptr addrspace(1) %refactor_cont_frame_gep13 to ptr
  %gc_write_barrier15 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr14, ptr addrspace(1) %refactor_frame_root_reload12)
  %refactor_cont_state_gep16 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload11, i32 0, i32 2
  store i32 2, ptr addrspace(1) %refactor_cont_state_gep16, align 4
  %refactor_cont_one_shot_gep17 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload11, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_one_shot_gep17, align 1
  %refactor_cont_composed_callee_gep18 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload11, i32 0, i32 4
  %gc_wb_slot_addr19 = addrspacecast ptr addrspace(1) %refactor_cont_composed_callee_gep18 to ptr
  %gc_write_barrier20 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr19, ptr addrspace(1) null)
  %refactor_step_tmp21 = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp21, align 8
  %refactor_step_tag_gep22 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp21, i32 0, i32 0
  store i32 1, ptr %refactor_step_tag_gep22, align 4
  %refactor_step_storage_gep23 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp21, i32 0, i32 1
  %refactor_step_cont_insert24 = insertvalue %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case0 undef, ptr addrspace(1) %refactor_cont_root_reload11, 0
  store %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case0 %refactor_step_cont_insert24, ptr %refactor_step_storage_gep23, align 8
  %refactor_step25 = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp21, align 8
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
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step25

refactor.st1:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr30 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev31 = load ptr, ptr %explicit_root_frame_pop_prev_ptr30, align 8
  %explicit_root_frame_pop_slot_032 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_032, align 8
  %explicit_root_frame_pop_slot_133 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_133, align 8
  %explicit_root_frame_pop_slot_234 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_234, align 8
  %explicit_root_frame_pop_slot_335 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_335, align 8
  store ptr %explicit_root_frame_pop_prev31, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

refactor.st2:                                     ; preds = %resume_payload_st2
  %refactor_step_tmp26 = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp26, align 8
  %refactor_step_tag_gep27 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp26, i32 0, i32 0
  store i32 0, ptr %refactor_step_tag_gep27, align 4
  %refactor_step_storage_gep28 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp26, i32 0, i32 1
  store %scoop.refactor.StepComplete__fixtures_build_fixture_visibility_hiddenWorker undef, ptr %refactor_step_storage_gep28, align 1
  %refactor_step29 = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp26, align 8
  %explicit_root_frame_pop_prev_ptr36 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev37 = load ptr, ptr %explicit_root_frame_pop_prev_ptr36, align 8
  %explicit_root_frame_pop_slot_038 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_038, align 8
  %explicit_root_frame_pop_slot_139 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_139, align 8
  %explicit_root_frame_pop_slot_240 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_240, align 8
  %explicit_root_frame_pop_slot_341 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_341, align 8
  store ptr %explicit_root_frame_pop_prev37, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step29

refactor.st3:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr42 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev43 = load ptr, ptr %explicit_root_frame_pop_prev_ptr42, align 8
  %explicit_root_frame_pop_slot_044 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_044, align 8
  %explicit_root_frame_pop_slot_145 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_145, align 8
  %explicit_root_frame_pop_slot_246 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_246, align 8
  %explicit_root_frame_pop_slot_347 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_347, align 8
  store ptr %explicit_root_frame_pop_prev43, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_first:                                     ; preds = %entry
  %refactor_store_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 3
  store i1 true, ptr addrspace(1) %refactor_store_one_shot_gep, align 1
  %refactor_composed_callee_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 4
  %refactor_composed_callee = load ptr addrspace(1), ptr addrspace(1) %refactor_composed_callee_gep, align 8
  %refactor_composed_callee_is_null = icmp eq ptr addrspace(1) %refactor_composed_callee, null
  br i1 %refactor_composed_callee_is_null, label %resume_plain_dispatch, label %resume_composed_dispatch

resume_double:                                    ; preds = %entry
  %rt_alloc_refactor_cont = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_continuation_layout__fixtures_build_fixture_visibility_hiddenWorker__type_desc, i64 72)
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
  %refactor_cont_root_reload = load ptr addrspace(1), ptr %explicit_root_frame_slot_2, align 8
  %refactor_frame_root_reload = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  %refactor_cont_frame_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 1
  %gc_wb_slot_addr = addrspacecast ptr addrspace(1) %refactor_cont_frame_gep to ptr
  %gc_write_barrier = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr, ptr addrspace(1) %refactor_frame_root_reload)
  %refactor_cont_state_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 2
  store i32 %refactor_resume_state, ptr addrspace(1) %refactor_cont_state_gep, align 4
  %refactor_cont_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_one_shot_gep, align 1
  %refactor_cont_composed_callee_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 4
  %gc_wb_slot_addr1 = addrspacecast ptr addrspace(1) %refactor_cont_composed_callee_gep to ptr
  %gc_write_barrier2 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr1, ptr addrspace(1) null)
  %refactor_step_tmp = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp, align 8
  %refactor_step_tag_gep = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, i32 0, i32 0
  store i32 2, ptr %refactor_step_tag_gep, align 4
  %refactor_step_storage_gep = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, i32 0, i32 1
  %refactor_step_cont_insert = insertvalue %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case1 { %scoop.core.RuntimeError { i32 2, i64 0, ptr addrspace(1) null }, ptr addrspace(1) undef }, ptr addrspace(1) %refactor_cont_root_reload, 1
  store %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case1 %refactor_step_cont_insert, ptr %refactor_step_storage_gep, align 8
  %refactor_step = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, align 8
  %explicit_root_frame_pop_prev_ptr48 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev49 = load ptr, ptr %explicit_root_frame_pop_prev_ptr48, align 8
  %explicit_root_frame_pop_slot_050 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_050, align 8
  %explicit_root_frame_pop_slot_151 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_151, align 8
  %explicit_root_frame_pop_slot_252 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_252, align 8
  %explicit_root_frame_pop_slot_353 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_353, align 8
  store ptr %explicit_root_frame_pop_prev49, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step

resume_plain_dispatch:                            ; preds = %resume_first
  switch i32 %refactor_resume_state, label %resume_invalid_state [
    i32 2, label %resume_payload_st2
  ]

resume_composed_dispatch:                         ; preds = %resume_first
  %explicit_root_frame_pop_prev_ptr54 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev55 = load ptr, ptr %explicit_root_frame_pop_prev_ptr54, align 8
  %explicit_root_frame_pop_slot_056 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_056, align 8
  %explicit_root_frame_pop_slot_157 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_157, align 8
  %explicit_root_frame_pop_slot_258 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_258, align 8
  %explicit_root_frame_pop_slot_359 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_359, align 8
  store ptr %explicit_root_frame_pop_prev55, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_invalid_state:                             ; preds = %resume_plain_dispatch
  %explicit_root_frame_pop_prev_ptr60 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev61 = load ptr, ptr %explicit_root_frame_pop_prev_ptr60, align 8
  %explicit_root_frame_pop_slot_062 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_062, align 8
  %explicit_root_frame_pop_slot_163 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_163, align 8
  %explicit_root_frame_pop_slot_264 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_264, align 8
  %explicit_root_frame_pop_slot_365 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_365, align 8
  store ptr %explicit_root_frame_pop_prev61, ptr @__scoop_explicit_root_frame_top, align 8
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
  %refactor_resume_cont_root_reload = load ptr addrspace(1), ptr %explicit_root_frame_slot_1, align 8
  %refactor_load_frame_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 1
  %refactor_load_frame_gc = load ptr addrspace(1), ptr addrspace(1) %refactor_load_frame_gep, align 8
  store ptr addrspace(1) %refactor_load_frame_gc, ptr %explicit_root_frame_slot_0, align 8
  %refactor_resume_state_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 2
  %refactor_resume_state = load i32, ptr addrspace(1) %refactor_resume_state_gep, align 4
  %refactor_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 3
  %refactor_one_shot = load i1, ptr addrspace(1) %refactor_one_shot_gep, align 1
  br i1 %refactor_one_shot, label %resume_double, label %resume_first

refactor.st0:                                     ; No predecessors!
  %rt_alloc_refactor_cont3 = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_continuation_layout__fixtures_build_fixture_visibility_hiddenWorker__type_desc, i64 72)
  %refactor_cont_zero_field_14 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 1
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_14, align 8
  %refactor_cont_zero_field_25 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 2
  store i32 0, ptr addrspace(1) %refactor_cont_zero_field_25, align 4
  %refactor_cont_zero_field_36 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_zero_field_36, align 1
  %refactor_cont_zero_field_47 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 4
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_47, align 8
  %refactor_cont_zero_field_58 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 5
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_58, align 8
  %refactor_cont_zero_field_69 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 6
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_69, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_3, align 8
  store ptr addrspace(1) %rt_alloc_refactor_cont3, ptr %explicit_root_frame_slot_3, align 8
  %refactor_cont_root_reload11 = load ptr addrspace(1), ptr %explicit_root_frame_slot_3, align 8
  %refactor_frame_root_reload12 = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  %refactor_cont_frame_gep13 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload11, i32 0, i32 1
  %gc_wb_slot_addr14 = addrspacecast ptr addrspace(1) %refactor_cont_frame_gep13 to ptr
  %gc_write_barrier15 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr14, ptr addrspace(1) %refactor_frame_root_reload12)
  %refactor_cont_state_gep16 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload11, i32 0, i32 2
  store i32 2, ptr addrspace(1) %refactor_cont_state_gep16, align 4
  %refactor_cont_one_shot_gep17 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload11, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_one_shot_gep17, align 1
  %refactor_cont_composed_callee_gep18 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload11, i32 0, i32 4
  %gc_wb_slot_addr19 = addrspacecast ptr addrspace(1) %refactor_cont_composed_callee_gep18 to ptr
  %gc_write_barrier20 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr19, ptr addrspace(1) null)
  %refactor_step_tmp21 = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp21, align 8
  %refactor_step_tag_gep22 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp21, i32 0, i32 0
  store i32 1, ptr %refactor_step_tag_gep22, align 4
  %refactor_step_storage_gep23 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp21, i32 0, i32 1
  %refactor_step_cont_insert24 = insertvalue %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case0 undef, ptr addrspace(1) %refactor_cont_root_reload11, 0
  store %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case0 %refactor_step_cont_insert24, ptr %refactor_step_storage_gep23, align 8
  %refactor_step25 = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp21, align 8
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
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step25

refactor.st1:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr30 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev31 = load ptr, ptr %explicit_root_frame_pop_prev_ptr30, align 8
  %explicit_root_frame_pop_slot_032 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_032, align 8
  %explicit_root_frame_pop_slot_133 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_133, align 8
  %explicit_root_frame_pop_slot_234 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_234, align 8
  %explicit_root_frame_pop_slot_335 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_335, align 8
  store ptr %explicit_root_frame_pop_prev31, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

refactor.st2:                                     ; No predecessors!
  %refactor_step_tmp26 = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp26, align 8
  %refactor_step_tag_gep27 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp26, i32 0, i32 0
  store i32 0, ptr %refactor_step_tag_gep27, align 4
  %refactor_step_storage_gep28 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp26, i32 0, i32 1
  store %scoop.refactor.StepComplete__fixtures_build_fixture_visibility_hiddenWorker undef, ptr %refactor_step_storage_gep28, align 1
  %refactor_step29 = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp26, align 8
  %explicit_root_frame_pop_prev_ptr36 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev37 = load ptr, ptr %explicit_root_frame_pop_prev_ptr36, align 8
  %explicit_root_frame_pop_slot_038 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_038, align 8
  %explicit_root_frame_pop_slot_139 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_139, align 8
  %explicit_root_frame_pop_slot_240 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_240, align 8
  %explicit_root_frame_pop_slot_341 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_341, align 8
  store ptr %explicit_root_frame_pop_prev37, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step29

refactor.st3:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr42 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev43 = load ptr, ptr %explicit_root_frame_pop_prev_ptr42, align 8
  %explicit_root_frame_pop_slot_044 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_044, align 8
  %explicit_root_frame_pop_slot_145 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_145, align 8
  %explicit_root_frame_pop_slot_246 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_246, align 8
  %explicit_root_frame_pop_slot_347 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_347, align 8
  store ptr %explicit_root_frame_pop_prev43, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_first:                                     ; preds = %entry
  %refactor_store_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 3
  store i1 true, ptr addrspace(1) %refactor_store_one_shot_gep, align 1
  %refactor_composed_callee_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 4
  %refactor_composed_callee = load ptr addrspace(1), ptr addrspace(1) %refactor_composed_callee_gep, align 8
  %refactor_composed_callee_is_null = icmp eq ptr addrspace(1) %refactor_composed_callee, null
  br i1 %refactor_composed_callee_is_null, label %resume_plain_dispatch, label %resume_composed_dispatch

resume_double:                                    ; preds = %entry
  %rt_alloc_refactor_cont = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_continuation_layout__fixtures_build_fixture_visibility_hiddenWorker__type_desc, i64 72)
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
  %refactor_cont_root_reload = load ptr addrspace(1), ptr %explicit_root_frame_slot_2, align 8
  %refactor_frame_root_reload = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  %refactor_cont_frame_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 1
  %gc_wb_slot_addr = addrspacecast ptr addrspace(1) %refactor_cont_frame_gep to ptr
  %gc_write_barrier = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr, ptr addrspace(1) %refactor_frame_root_reload)
  %refactor_cont_state_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 2
  store i32 %refactor_resume_state, ptr addrspace(1) %refactor_cont_state_gep, align 4
  %refactor_cont_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_one_shot_gep, align 1
  %refactor_cont_composed_callee_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 4
  %gc_wb_slot_addr1 = addrspacecast ptr addrspace(1) %refactor_cont_composed_callee_gep to ptr
  %gc_write_barrier2 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr1, ptr addrspace(1) null)
  %refactor_step_tmp = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp, align 8
  %refactor_step_tag_gep = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, i32 0, i32 0
  store i32 2, ptr %refactor_step_tag_gep, align 4
  %refactor_step_storage_gep = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, i32 0, i32 1
  %refactor_step_cont_insert = insertvalue %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case1 { %scoop.core.RuntimeError { i32 2, i64 0, ptr addrspace(1) null }, ptr addrspace(1) undef }, ptr addrspace(1) %refactor_cont_root_reload, 1
  store %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case1 %refactor_step_cont_insert, ptr %refactor_step_storage_gep, align 8
  %refactor_step = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, align 8
  %explicit_root_frame_pop_prev_ptr48 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev49 = load ptr, ptr %explicit_root_frame_pop_prev_ptr48, align 8
  %explicit_root_frame_pop_slot_050 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_050, align 8
  %explicit_root_frame_pop_slot_151 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_151, align 8
  %explicit_root_frame_pop_slot_252 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_252, align 8
  %explicit_root_frame_pop_slot_353 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_353, align 8
  store ptr %explicit_root_frame_pop_prev49, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step

resume_plain_dispatch:                            ; preds = %resume_first
  switch i32 %refactor_resume_state, label %resume_invalid_state [
  ]

resume_composed_dispatch:                         ; preds = %resume_first
  %explicit_root_frame_pop_prev_ptr54 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev55 = load ptr, ptr %explicit_root_frame_pop_prev_ptr54, align 8
  %explicit_root_frame_pop_slot_056 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_056, align 8
  %explicit_root_frame_pop_slot_157 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_157, align 8
  %explicit_root_frame_pop_slot_258 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_258, align 8
  %explicit_root_frame_pop_slot_359 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_359, align 8
  store ptr %explicit_root_frame_pop_prev55, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_invalid_state:                             ; preds = %resume_plain_dispatch
  %explicit_root_frame_pop_prev_ptr60 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev61 = load ptr, ptr %explicit_root_frame_pop_prev_ptr60, align 8
  %explicit_root_frame_pop_slot_062 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_062, align 8
  %explicit_root_frame_pop_slot_163 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_163, align 8
  %explicit_root_frame_pop_slot_264 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_264, align 8
  %explicit_root_frame_pop_slot_365 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_365, align 8
  store ptr %explicit_root_frame_pop_prev61, ptr @__scoop_explicit_root_frame_top, align 8
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
  %refactor_resume_cont_root_reload = load ptr addrspace(1), ptr %explicit_root_frame_slot_1, align 8
  %refactor_load_frame_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 1
  %refactor_load_frame_gc = load ptr addrspace(1), ptr addrspace(1) %refactor_load_frame_gep, align 8
  store ptr addrspace(1) %refactor_load_frame_gc, ptr %explicit_root_frame_slot_0, align 8
  %refactor_resume_state_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 2
  %refactor_resume_state = load i32, ptr addrspace(1) %refactor_resume_state_gep, align 4
  %refactor_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 3
  %refactor_one_shot = load i1, ptr addrspace(1) %refactor_one_shot_gep, align 1
  br i1 %refactor_one_shot, label %resume_double, label %resume_first

refactor.st0:                                     ; No predecessors!
  %rt_alloc_refactor_cont3 = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_continuation_layout__fixtures_build_fixture_visibility_hiddenWorker__type_desc, i64 72)
  %refactor_cont_zero_field_14 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 1
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_14, align 8
  %refactor_cont_zero_field_25 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 2
  store i32 0, ptr addrspace(1) %refactor_cont_zero_field_25, align 4
  %refactor_cont_zero_field_36 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_zero_field_36, align 1
  %refactor_cont_zero_field_47 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 4
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_47, align 8
  %refactor_cont_zero_field_58 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 5
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_58, align 8
  %refactor_cont_zero_field_69 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 6
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_69, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_3, align 8
  store ptr addrspace(1) %rt_alloc_refactor_cont3, ptr %explicit_root_frame_slot_3, align 8
  %refactor_cont_root_reload11 = load ptr addrspace(1), ptr %explicit_root_frame_slot_3, align 8
  %refactor_frame_root_reload12 = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  %refactor_cont_frame_gep13 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload11, i32 0, i32 1
  %gc_wb_slot_addr14 = addrspacecast ptr addrspace(1) %refactor_cont_frame_gep13 to ptr
  %gc_write_barrier15 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr14, ptr addrspace(1) %refactor_frame_root_reload12)
  %refactor_cont_state_gep16 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload11, i32 0, i32 2
  store i32 2, ptr addrspace(1) %refactor_cont_state_gep16, align 4
  %refactor_cont_one_shot_gep17 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload11, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_one_shot_gep17, align 1
  %refactor_cont_composed_callee_gep18 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload11, i32 0, i32 4
  %gc_wb_slot_addr19 = addrspacecast ptr addrspace(1) %refactor_cont_composed_callee_gep18 to ptr
  %gc_write_barrier20 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr19, ptr addrspace(1) null)
  %refactor_step_tmp21 = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp21, align 8
  %refactor_step_tag_gep22 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp21, i32 0, i32 0
  store i32 1, ptr %refactor_step_tag_gep22, align 4
  %refactor_step_storage_gep23 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp21, i32 0, i32 1
  %refactor_step_cont_insert24 = insertvalue %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case0 undef, ptr addrspace(1) %refactor_cont_root_reload11, 0
  store %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case0 %refactor_step_cont_insert24, ptr %refactor_step_storage_gep23, align 8
  %refactor_step25 = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp21, align 8
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
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step25

refactor.st1:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr30 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev31 = load ptr, ptr %explicit_root_frame_pop_prev_ptr30, align 8
  %explicit_root_frame_pop_slot_032 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_032, align 8
  %explicit_root_frame_pop_slot_133 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_133, align 8
  %explicit_root_frame_pop_slot_234 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_234, align 8
  %explicit_root_frame_pop_slot_335 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_335, align 8
  store ptr %explicit_root_frame_pop_prev31, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

refactor.st2:                                     ; preds = %resume_payload_st2
  %refactor_step_tmp26 = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp26, align 8
  %refactor_step_tag_gep27 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp26, i32 0, i32 0
  store i32 0, ptr %refactor_step_tag_gep27, align 4
  %refactor_step_storage_gep28 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp26, i32 0, i32 1
  store %scoop.refactor.StepComplete__fixtures_build_fixture_visibility_hiddenWorker undef, ptr %refactor_step_storage_gep28, align 1
  %refactor_step29 = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp26, align 8
  %explicit_root_frame_pop_prev_ptr36 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev37 = load ptr, ptr %explicit_root_frame_pop_prev_ptr36, align 8
  %explicit_root_frame_pop_slot_038 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_038, align 8
  %explicit_root_frame_pop_slot_139 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_139, align 8
  %explicit_root_frame_pop_slot_240 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_240, align 8
  %explicit_root_frame_pop_slot_341 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_341, align 8
  store ptr %explicit_root_frame_pop_prev37, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step29

refactor.st3:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr42 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev43 = load ptr, ptr %explicit_root_frame_pop_prev_ptr42, align 8
  %explicit_root_frame_pop_slot_044 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_044, align 8
  %explicit_root_frame_pop_slot_145 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_145, align 8
  %explicit_root_frame_pop_slot_246 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_246, align 8
  %explicit_root_frame_pop_slot_347 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_347, align 8
  store ptr %explicit_root_frame_pop_prev43, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_first:                                     ; preds = %entry
  %refactor_store_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 3
  store i1 true, ptr addrspace(1) %refactor_store_one_shot_gep, align 1
  %refactor_composed_callee_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 4
  %refactor_composed_callee = load ptr addrspace(1), ptr addrspace(1) %refactor_composed_callee_gep, align 8
  %refactor_composed_callee_is_null = icmp eq ptr addrspace(1) %refactor_composed_callee, null
  br i1 %refactor_composed_callee_is_null, label %resume_plain_dispatch, label %resume_composed_dispatch

resume_double:                                    ; preds = %entry
  %rt_alloc_refactor_cont = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_continuation_layout__fixtures_build_fixture_visibility_hiddenWorker__type_desc, i64 72)
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
  %refactor_cont_root_reload = load ptr addrspace(1), ptr %explicit_root_frame_slot_2, align 8
  %refactor_frame_root_reload = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  %refactor_cont_frame_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 1
  %gc_wb_slot_addr = addrspacecast ptr addrspace(1) %refactor_cont_frame_gep to ptr
  %gc_write_barrier = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr, ptr addrspace(1) %refactor_frame_root_reload)
  %refactor_cont_state_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 2
  store i32 %refactor_resume_state, ptr addrspace(1) %refactor_cont_state_gep, align 4
  %refactor_cont_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_one_shot_gep, align 1
  %refactor_cont_composed_callee_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 4
  %gc_wb_slot_addr1 = addrspacecast ptr addrspace(1) %refactor_cont_composed_callee_gep to ptr
  %gc_write_barrier2 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr1, ptr addrspace(1) null)
  %refactor_step_tmp = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp, align 8
  %refactor_step_tag_gep = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, i32 0, i32 0
  store i32 2, ptr %refactor_step_tag_gep, align 4
  %refactor_step_storage_gep = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, i32 0, i32 1
  %refactor_step_cont_insert = insertvalue %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case1 { %scoop.core.RuntimeError { i32 2, i64 0, ptr addrspace(1) null }, ptr addrspace(1) undef }, ptr addrspace(1) %refactor_cont_root_reload, 1
  store %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case1 %refactor_step_cont_insert, ptr %refactor_step_storage_gep, align 8
  %refactor_step = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, align 8
  %explicit_root_frame_pop_prev_ptr48 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev49 = load ptr, ptr %explicit_root_frame_pop_prev_ptr48, align 8
  %explicit_root_frame_pop_slot_050 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_050, align 8
  %explicit_root_frame_pop_slot_151 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_151, align 8
  %explicit_root_frame_pop_slot_252 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_252, align 8
  %explicit_root_frame_pop_slot_353 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_353, align 8
  store ptr %explicit_root_frame_pop_prev49, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step

resume_plain_dispatch:                            ; preds = %resume_first
  switch i32 %refactor_resume_state, label %resume_invalid_state [
    i32 2, label %resume_payload_st2
  ]

resume_composed_dispatch:                         ; preds = %resume_first
  %explicit_root_frame_pop_prev_ptr54 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev55 = load ptr, ptr %explicit_root_frame_pop_prev_ptr54, align 8
  %explicit_root_frame_pop_slot_056 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_056, align 8
  %explicit_root_frame_pop_slot_157 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_157, align 8
  %explicit_root_frame_pop_slot_258 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_258, align 8
  %explicit_root_frame_pop_slot_359 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_359, align 8
  store ptr %explicit_root_frame_pop_prev55, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_invalid_state:                             ; preds = %resume_plain_dispatch
  %explicit_root_frame_pop_prev_ptr60 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev61 = load ptr, ptr %explicit_root_frame_pop_prev_ptr60, align 8
  %explicit_root_frame_pop_slot_062 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_062, align 8
  %explicit_root_frame_pop_slot_163 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_163, align 8
  %explicit_root_frame_pop_slot_264 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_264, align 8
  %explicit_root_frame_pop_slot_365 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_365, align 8
  store ptr %explicit_root_frame_pop_prev61, ptr @__scoop_explicit_root_frame_top, align 8
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
  %refactor_resume_cont_root_reload = load ptr addrspace(1), ptr %explicit_root_frame_slot_1, align 8
  %refactor_load_frame_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 1
  %refactor_load_frame_gc = load ptr addrspace(1), ptr addrspace(1) %refactor_load_frame_gep, align 8
  store ptr addrspace(1) %refactor_load_frame_gc, ptr %explicit_root_frame_slot_0, align 8
  %refactor_resume_state_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 2
  %refactor_resume_state = load i32, ptr addrspace(1) %refactor_resume_state_gep, align 4
  %refactor_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 3
  %refactor_one_shot = load i1, ptr addrspace(1) %refactor_one_shot_gep, align 1
  br i1 %refactor_one_shot, label %resume_double, label %resume_first

refactor.st0:                                     ; No predecessors!
  %rt_alloc_refactor_cont3 = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_continuation_layout__fixtures_build_fixture_visibility_hiddenWorker__type_desc, i64 72)
  %refactor_cont_zero_field_14 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 1
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_14, align 8
  %refactor_cont_zero_field_25 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 2
  store i32 0, ptr addrspace(1) %refactor_cont_zero_field_25, align 4
  %refactor_cont_zero_field_36 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_zero_field_36, align 1
  %refactor_cont_zero_field_47 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 4
  store ptr addrspace(1) null, ptr addrspace(1) %refactor_cont_zero_field_47, align 8
  %refactor_cont_zero_field_58 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 5
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_58, align 8
  %refactor_cont_zero_field_69 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %rt_alloc_refactor_cont3, i32 0, i32 6
  store ptr null, ptr addrspace(1) %refactor_cont_zero_field_69, align 8
  store ptr addrspace(1) null, ptr %explicit_root_frame_slot_3, align 8
  store ptr addrspace(1) %rt_alloc_refactor_cont3, ptr %explicit_root_frame_slot_3, align 8
  %refactor_cont_root_reload11 = load ptr addrspace(1), ptr %explicit_root_frame_slot_3, align 8
  %refactor_frame_root_reload12 = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  %refactor_cont_frame_gep13 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload11, i32 0, i32 1
  %gc_wb_slot_addr14 = addrspacecast ptr addrspace(1) %refactor_cont_frame_gep13 to ptr
  %gc_write_barrier15 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr14, ptr addrspace(1) %refactor_frame_root_reload12)
  %refactor_cont_state_gep16 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload11, i32 0, i32 2
  store i32 2, ptr addrspace(1) %refactor_cont_state_gep16, align 4
  %refactor_cont_one_shot_gep17 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload11, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_one_shot_gep17, align 1
  %refactor_cont_composed_callee_gep18 = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload11, i32 0, i32 4
  %gc_wb_slot_addr19 = addrspacecast ptr addrspace(1) %refactor_cont_composed_callee_gep18 to ptr
  %gc_write_barrier20 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr19, ptr addrspace(1) null)
  %refactor_step_tmp21 = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp21, align 8
  %refactor_step_tag_gep22 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp21, i32 0, i32 0
  store i32 1, ptr %refactor_step_tag_gep22, align 4
  %refactor_step_storage_gep23 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp21, i32 0, i32 1
  %refactor_step_cont_insert24 = insertvalue %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case0 undef, ptr addrspace(1) %refactor_cont_root_reload11, 0
  store %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case0 %refactor_step_cont_insert24, ptr %refactor_step_storage_gep23, align 8
  %refactor_step25 = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp21, align 8
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
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step25

refactor.st1:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr30 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev31 = load ptr, ptr %explicit_root_frame_pop_prev_ptr30, align 8
  %explicit_root_frame_pop_slot_032 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_032, align 8
  %explicit_root_frame_pop_slot_133 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_133, align 8
  %explicit_root_frame_pop_slot_234 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_234, align 8
  %explicit_root_frame_pop_slot_335 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_335, align 8
  store ptr %explicit_root_frame_pop_prev31, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

refactor.st2:                                     ; No predecessors!
  %refactor_step_tmp26 = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp26, align 8
  %refactor_step_tag_gep27 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp26, i32 0, i32 0
  store i32 0, ptr %refactor_step_tag_gep27, align 4
  %refactor_step_storage_gep28 = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp26, i32 0, i32 1
  store %scoop.refactor.StepComplete__fixtures_build_fixture_visibility_hiddenWorker undef, ptr %refactor_step_storage_gep28, align 1
  %refactor_step29 = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp26, align 8
  %explicit_root_frame_pop_prev_ptr36 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev37 = load ptr, ptr %explicit_root_frame_pop_prev_ptr36, align 8
  %explicit_root_frame_pop_slot_038 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_038, align 8
  %explicit_root_frame_pop_slot_139 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_139, align 8
  %explicit_root_frame_pop_slot_240 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_240, align 8
  %explicit_root_frame_pop_slot_341 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_341, align 8
  store ptr %explicit_root_frame_pop_prev37, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step29

refactor.st3:                                     ; No predecessors!
  %explicit_root_frame_pop_prev_ptr42 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev43 = load ptr, ptr %explicit_root_frame_pop_prev_ptr42, align 8
  %explicit_root_frame_pop_slot_044 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_044, align 8
  %explicit_root_frame_pop_slot_145 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_145, align 8
  %explicit_root_frame_pop_slot_246 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_246, align 8
  %explicit_root_frame_pop_slot_347 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_347, align 8
  store ptr %explicit_root_frame_pop_prev43, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_first:                                     ; preds = %entry
  %refactor_store_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 3
  store i1 true, ptr addrspace(1) %refactor_store_one_shot_gep, align 1
  %refactor_composed_callee_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_resume_cont_root_reload, i32 0, i32 4
  %refactor_composed_callee = load ptr addrspace(1), ptr addrspace(1) %refactor_composed_callee_gep, align 8
  %refactor_composed_callee_is_null = icmp eq ptr addrspace(1) %refactor_composed_callee, null
  br i1 %refactor_composed_callee_is_null, label %resume_plain_dispatch, label %resume_composed_dispatch

resume_double:                                    ; preds = %entry
  %rt_alloc_refactor_cont = call ptr addrspace(1) @scoop_alloc_typed(ptr @__scoop_refactor_continuation_layout__fixtures_build_fixture_visibility_hiddenWorker__type_desc, i64 72)
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
  %refactor_cont_root_reload = load ptr addrspace(1), ptr %explicit_root_frame_slot_2, align 8
  %refactor_frame_root_reload = load ptr addrspace(1), ptr %explicit_root_frame_slot_0, align 8
  %refactor_cont_frame_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 1
  %gc_wb_slot_addr = addrspacecast ptr addrspace(1) %refactor_cont_frame_gep to ptr
  %gc_write_barrier = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr, ptr addrspace(1) %refactor_frame_root_reload)
  %refactor_cont_state_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 2
  store i32 %refactor_resume_state, ptr addrspace(1) %refactor_cont_state_gep, align 4
  %refactor_cont_one_shot_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 3
  store i1 false, ptr addrspace(1) %refactor_cont_one_shot_gep, align 1
  %refactor_cont_composed_callee_gep = getelementptr inbounds nuw %scoop.refactor.Continuation__fixtures_build_fixture_visibility_hiddenWorker, ptr addrspace(1) %refactor_cont_root_reload, i32 0, i32 4
  %gc_wb_slot_addr1 = addrspacecast ptr addrspace(1) %refactor_cont_composed_callee_gep to ptr
  %gc_write_barrier2 = call ptr addrspace(1) @scoop_gc_write_barrier(ptr %gc_wb_slot_addr1, ptr addrspace(1) null)
  %refactor_step_tmp = alloca %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, align 8
  store %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker zeroinitializer, ptr %refactor_step_tmp, align 8
  %refactor_step_tag_gep = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, i32 0, i32 0
  store i32 2, ptr %refactor_step_tag_gep, align 4
  %refactor_step_storage_gep = getelementptr inbounds nuw %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, i32 0, i32 1
  %refactor_step_cont_insert = insertvalue %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case1 { %scoop.core.RuntimeError { i32 2, i64 0, ptr addrspace(1) null }, ptr addrspace(1) undef }, ptr addrspace(1) %refactor_cont_root_reload, 1
  store %scoop.refactor.StepCase__fixtures_build_fixture_visibility_hiddenWorker__case1 %refactor_step_cont_insert, ptr %refactor_step_storage_gep, align 8
  %refactor_step = load %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker, ptr %refactor_step_tmp, align 8
  %explicit_root_frame_pop_prev_ptr48 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev49 = load ptr, ptr %explicit_root_frame_pop_prev_ptr48, align 8
  %explicit_root_frame_pop_slot_050 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_050, align 8
  %explicit_root_frame_pop_slot_151 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_151, align 8
  %explicit_root_frame_pop_slot_252 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_252, align 8
  %explicit_root_frame_pop_slot_353 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_353, align 8
  store ptr %explicit_root_frame_pop_prev49, ptr @__scoop_explicit_root_frame_top, align 8
  ret %scoop.refactor.Step__fixtures_build_fixture_visibility_hiddenWorker %refactor_step

resume_plain_dispatch:                            ; preds = %resume_first
  switch i32 %refactor_resume_state, label %resume_invalid_state [
  ]

resume_composed_dispatch:                         ; preds = %resume_first
  %explicit_root_frame_pop_prev_ptr54 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev55 = load ptr, ptr %explicit_root_frame_pop_prev_ptr54, align 8
  %explicit_root_frame_pop_slot_056 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_056, align 8
  %explicit_root_frame_pop_slot_157 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_157, align 8
  %explicit_root_frame_pop_slot_258 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_258, align 8
  %explicit_root_frame_pop_slot_359 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_359, align 8
  store ptr %explicit_root_frame_pop_prev55, ptr @__scoop_explicit_root_frame_top, align 8
  unreachable

resume_invalid_state:                             ; preds = %resume_plain_dispatch
  %explicit_root_frame_pop_prev_ptr60 = getelementptr inbounds nuw %scoop.runtime.ScoopRootFrameHeader, ptr %explicit_root_frame_storage, i32 0, i32 0
  %explicit_root_frame_pop_prev61 = load ptr, ptr %explicit_root_frame_pop_prev_ptr60, align 8
  %explicit_root_frame_pop_slot_062 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 16
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_062, align 8
  %explicit_root_frame_pop_slot_163 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 24
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_163, align 8
  %explicit_root_frame_pop_slot_264 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 32
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_264, align 8
  %explicit_root_frame_pop_slot_365 = getelementptr inbounds i8, ptr %explicit_root_frame_storage, i64 40
  store ptr addrspace(1) null, ptr %explicit_root_frame_pop_slot_365, align 8
  store ptr %explicit_root_frame_pop_prev61, ptr @__scoop_explicit_root_frame_top, align 8
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
