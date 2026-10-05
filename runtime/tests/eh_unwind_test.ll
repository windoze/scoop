; Exercise the real two-phase unwinder through a cleanup-only middle frame.
declare i32 @scoop_eh_personality(...)
declare void @test_throw()
declare void @test_cleanup()
declare void @test_caught(ptr)

define void @test_cleanup_frame() uwtable "frame-pointer"="all"
    personality ptr @scoop_eh_personality {
entry:
  invoke void @test_throw() to label %done unwind label %cleanup
done:
  ret void
cleanup:
  %exception = landingpad { ptr, i32 } cleanup
  call void @test_cleanup()
  resume { ptr, i32 } %exception
}

define void @test_catch_frame() uwtable "frame-pointer"="all"
    personality ptr @scoop_eh_personality {
entry:
  invoke void @test_cleanup_frame() to label %done unwind label %catch
done:
  ret void
catch:
  %exception = landingpad { ptr, i32 } catch ptr null
  %record = extractvalue { ptr, i32 } %exception, 0
  call void @test_caught(ptr %record)
  ret void
}
