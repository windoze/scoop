declare {ptr addrspace(1), ptr} @produce(ptr addrspace(1), ptr)
declare void @tick()
declare i32 @personality(...)
declare void @llvm.fake.use(...)
declare token @llvm.experimental.gc.statepoint.p0(i64 immarg, i32 immarg, ptr, i32 immarg, i32 immarg, ...)
declare {ptr addrspace(1), ptr} @llvm.experimental.gc.result.sl_p1p0s(token)

define {ptr addrspace(1), ptr} @f(ptr addrspace(1) %object, ptr %itab) #0 gc "statepoint-example" {
entry:
  %storage = alloca {ptr addrspace(1), ptr}, align 8
  %result = call {ptr addrspace(1), ptr} @produce(ptr addrspace(1) %object, ptr %itab) #1
  call void (...) @llvm.fake.use(ptr addrspace(1) %object), !scoop.statepoint-root-identity !0
  store {ptr addrspace(1), ptr} %result, ptr %storage, align 8
  %returned.object = extractvalue {ptr addrspace(1), ptr} %result, 0
  %returned.itab = extractvalue {ptr addrspace(1), ptr} %result, 1
  call void @tick() #2
  call void (...) @llvm.fake.use(ptr addrspace(1) %returned.object), !scoop.statepoint-root-identity !1
  store volatile ptr addrspace(1) %returned.object, ptr %storage, align 8
  %updated = load volatile ptr addrspace(1), ptr %storage, align 8
  %first = insertvalue {ptr addrspace(1), ptr} zeroinitializer, ptr addrspace(1) %updated, 0
  %complete = insertvalue {ptr addrspace(1), ptr} %first, ptr %returned.itab, 1
  ret {ptr addrspace(1), ptr} %complete
}

define {ptr addrspace(1), ptr} @g(ptr addrspace(1) %object, ptr %itab) #0 gc "statepoint-example" personality ptr @personality {
entry:
  %storage = alloca {ptr addrspace(1), ptr}, align 8
  %token = invoke token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 9, i32 0, ptr elementtype({ptr addrspace(1), ptr} (ptr addrspace(1), ptr)) @produce, i32 2, i32 0, ptr addrspace(1) %object, ptr %itab, i32 0, i32 0)
      to label %normal unwind label %exception
normal:
  %result = call {ptr addrspace(1), ptr} @llvm.experimental.gc.result.sl_p1p0s(token %token)
  store {ptr addrspace(1), ptr} %result, ptr %storage, align 8
  %returned.object = extractvalue {ptr addrspace(1), ptr} %result, 0
  %returned.itab = extractvalue {ptr addrspace(1), ptr} %result, 1
  call void @tick() #4
  call void (...) @llvm.fake.use(ptr addrspace(1) %returned.object), !scoop.statepoint-root-identity !2
  store volatile ptr addrspace(1) %returned.object, ptr %storage, align 8
  %updated = load volatile ptr addrspace(1), ptr %storage, align 8
  %first = insertvalue {ptr addrspace(1), ptr} zeroinitializer, ptr addrspace(1) %updated, 0
  %complete = insertvalue {ptr addrspace(1), ptr} %first, ptr %returned.itab, 1
  ret {ptr addrspace(1), ptr} %complete
exception:
  %failure = landingpad {ptr, i32} cleanup
  resume {ptr, i32} %failure
}

define {ptr addrspace(1), ptr} @h(ptr addrspace(1) %a, ptr %a.itab, ptr addrspace(1) %b, ptr %b.itab, i1 %choose) #0 gc "statepoint-example" {
entry:
  %storage = alloca {ptr addrspace(1), ptr}, align 8
  %a.first = insertvalue {ptr addrspace(1), ptr} zeroinitializer, ptr addrspace(1) %a, 0
  %a.complete = insertvalue {ptr addrspace(1), ptr} %a.first, ptr %a.itab, 1
  %b.first = insertvalue {ptr addrspace(1), ptr} zeroinitializer, ptr addrspace(1) %b, 0
  %b.complete = insertvalue {ptr addrspace(1), ptr} %b.first, ptr %b.itab, 1
  br i1 %choose, label %left, label %right
left:
  br label %merge
right:
  br label %merge
merge:
  %selected = phi {ptr addrspace(1), ptr} [%a.complete, %left], [%b.complete, %right]
  store {ptr addrspace(1), ptr} %selected, ptr %storage, align 8
  %selected.object = extractvalue {ptr addrspace(1), ptr} %selected, 0
  %selected.itab = extractvalue {ptr addrspace(1), ptr} %selected, 1
  call void @tick() #5
  call void (...) @llvm.fake.use(ptr addrspace(1) %selected.object), !scoop.statepoint-root-identity !3
  store volatile ptr addrspace(1) %selected.object, ptr %storage, align 8
  %updated = load volatile ptr addrspace(1), ptr %storage, align 8
  %first = insertvalue {ptr addrspace(1), ptr} zeroinitializer, ptr addrspace(1) %updated, 0
  %complete = insertvalue {ptr addrspace(1), ptr} %first, ptr %selected.itab, 1
  ret {ptr addrspace(1), ptr} %complete
}

attributes #0 = { noredzone "disable-tail-calls"="true" "frame-pointer"="all" }
attributes #1 = { "statepoint-id"="7" }
attributes #2 = { "statepoint-id"="8" }
attributes #4 = { "statepoint-id"="10" }
attributes #5 = { "statepoint-id"="11" }
!0 = !{i64 7, i64 0, i64 0, i64 0}
!1 = !{i64 8, i64 2, i64 0, i64 0}
!2 = !{i64 10, i64 2, i64 0, i64 0}
!3 = !{i64 11, i64 2, i64 0, i64 0}
