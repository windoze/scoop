%I = type {ptr addrspace(1), ptr}
declare %I @produce(i32) #0
declare void @poll()
declare void @consume(ptr addrspace(1), ptr) #0
declare void @llvm.fake.use(...)

define %I @f(i1 %slow, i32 %count, ptr %output) #1 gc "statepoint-example" {
entry:
  %a = call %I @produce(i32 1)
  %a.object = extractvalue %I %a, 0
  %b = call %I @produce(i32 2)
  br label %loop
loop:
  %current = phi %I [%a, %entry], [%next, %back]
  %object = phi ptr addrspace(1) [%a.object, %entry], [%next.object, %back]
  %next = phi %I [%b, %entry], [%current, %back]
  %remaining = phi i32 [%count, %entry], [%less, %back]
  %next.object = extractvalue %I %next, 0
  br i1 %slow, label %collect, label %after
collect:
  call void @poll() #2
  call void (...) @llvm.fake.use(ptr addrspace(1) %object) #3, !scoop.statepoint-root-identity !0
  br label %after
after:
  store %I %current, ptr %output
  %itab = extractvalue %I %current, 1
  call void @consume(ptr addrspace(1) %object, ptr %itab)
  %done = icmp eq i32 %remaining, 0
  br i1 %done, label %exit, label %back
back:
  %less = sub i32 %remaining, 1
  br label %loop
exit:
  ret %I %current
}

attributes #0 = { "gc-leaf-function" }
attributes #1 = { "disable-tail-calls"="true" "frame-pointer"="all" }
attributes #2 = { noduplicate "statepoint-id"="7" }
attributes #3 = { noduplicate }
!0 = !{i64 7, i64 2, i64 0, i64 0}
