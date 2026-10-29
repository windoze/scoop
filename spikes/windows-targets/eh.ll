; Real LLVM invoke/landingpad emission, shared by both Windows machine triples.
declare void @throw_probe()
declare void @cleanup_probe()
declare void @caught_probe(ptr, i32)
declare i32 @scoop_eh_personality_seh(...)
declare token @llvm.experimental.gc.statepoint.p0(i64 immarg, i32 immarg, ptr, i32 immarg, i32 immarg, ...)

define void @cleanup_frame() noinline uwtable gc "statepoint-example" personality ptr @scoop_eh_personality_seh {
entry:
  %token = invoke token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 1, i32 0, ptr elementtype(void ()) @throw_probe, i32 0, i32 0, i32 0, i32 0)
      to label %normal unwind label %cleanup
normal:
  ret void
cleanup:
  %exception = landingpad { ptr, i32 } cleanup
  call void @cleanup_probe()
  resume { ptr, i32 } %exception
}

define i32 @catch_probe() noinline uwtable gc "statepoint-example" personality ptr @scoop_eh_personality_seh {
entry:
  %token = invoke token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 2, i32 0, ptr elementtype(void ()) @cleanup_frame, i32 0, i32 0, i32 0, i32 0)
      to label %normal unwind label %caught
normal:
  ret i32 0
caught:
  %exception = landingpad { ptr, i32 } catch ptr null
  %record = extractvalue { ptr, i32 } %exception, 0
  %selector = extractvalue { ptr, i32 } %exception, 1
  call void @caught_probe(ptr %record, i32 %selector)
  ret i32 42
}
