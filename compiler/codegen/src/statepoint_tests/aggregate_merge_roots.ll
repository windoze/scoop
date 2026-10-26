%I = type {ptr addrspace(1), ptr}
declare %I @produce(i32) #0
declare void @left() #0
declare void @right() #0
declare void @poll()
declare void @consume(ptr addrspace(1), ptr) #0
declare void @llvm.fake.use(...)

define %I @f(i1 %choose, ptr %object.output, ptr %value.output) #1 gc "statepoint-example" {
entry:
  %a = call %I @produce(i32 1)
  %a.object = extractvalue %I %a, 0
  %b = call %I @produce(i32 2)
  %b.object = extractvalue %I %b, 0
  br i1 %choose, label %left, label %right
left:
  call void @left()
  br label %join
right:
  call void @right()
  br label %join
join:
  %value = phi %I [%a, %left], [%b, %right]
  %object = phi ptr addrspace(1) [%a.object, %left], [%b.object, %right]
  call void @poll() #2
  call void (...) @llvm.fake.use(ptr addrspace(1) %object) #3, !scoop.statepoint-root-identity !0
  store ptr addrspace(1) %object, ptr %object.output
  store %I %value, ptr %value.output
  %current.object = extractvalue %I %value, 0
  %itab = extractvalue %I %value, 1
  call void @consume(ptr addrspace(1) %current.object, ptr %itab)
  ret %I %value
}

attributes #0 = { "gc-leaf-function" }
attributes #1 = { "disable-tail-calls"="true" "frame-pointer"="all" }
attributes #2 = { noduplicate "statepoint-id"="7" }
attributes #3 = { noduplicate }
!0 = !{i64 7, i64 2, i64 0, i64 0}
