; Real LLVM invoke/landingpad emission, shared by both Windows machine triples.
declare void @throw_probe()
declare void @cleanup_probe()
declare void @caught_probe(ptr, i32)
declare i32 @scoop_eh_personality_seh(...)

define void @cleanup_frame() noinline uwtable personality ptr @scoop_eh_personality_seh {
entry:
  invoke void @throw_probe() to label %normal unwind label %cleanup
normal:
  ret void
cleanup:
  %exception = landingpad { ptr, i32 } cleanup
  call void @cleanup_probe()
  resume { ptr, i32 } %exception
}

define i32 @catch_probe() noinline uwtable personality ptr @scoop_eh_personality_seh {
entry:
  invoke void @cleanup_frame() to label %normal unwind label %caught
normal:
  ret i32 0
caught:
  %exception = landingpad { ptr, i32 } catch ptr null
  %record = extractvalue { ptr, i32 } %exception, 0
  %selector = extractvalue { ptr, i32 } %exception, 1
  call void @caught_probe(ptr %record, i32 %selector)
  ret i32 42
}
