%I = type {ptr addrspace(1), ptr}
declare %I @produce() #0
declare void @poll()
declare void @llvm.fake.use(...)
declare i32 @personality(...)

define %I @f(i1 %choose, i1 %slow, i32 %count, ptr %output) gc "statepoint-example" personality ptr @personality {
entry:
  br i1 %choose, label %left, label %right
left:
  %from.invoke = invoke %I @produce() to label %join unwind label %catch
right:
  %from.call = call %I @produce()
  br label %join
join:
  %start = phi %I [%from.invoke, %left], [%from.call, %right]
  br label %loop
loop:
  %current = phi %I [%start, %join], [%next, %back]
  %remaining = phi i32 [%count, %join], [%less, %back]
  br i1 %slow, label %collect, label %after
collect:
  %object = extractvalue %I %current, 0
  call void @poll() #1
  call void (...) @llvm.fake.use(ptr addrspace(1) %object) #2, !scoop.statepoint-root-identity !0
  br label %after
after:
  store %I %current, ptr %output
  %next = select i1 %choose, %I %current, %I zeroinitializer
  %done = icmp eq i32 %remaining, 0
  br i1 %done, label %exit, label %back
back:
  %less = sub i32 %remaining, 1
  br label %loop
exit:
  ret %I %current
catch:
  %exception = landingpad {ptr, i32} cleanup
  resume {ptr, i32} %exception
}

attributes #0 = { "gc-leaf-function" }
attributes #1 = { noduplicate "statepoint-id"="7" }
attributes #2 = { noduplicate }
!0 = !{i64 7, i64 2, i64 0, i64 0}
