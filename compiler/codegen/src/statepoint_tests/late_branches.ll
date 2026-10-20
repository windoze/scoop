declare void @tick()
declare void @llvm.fake.use(...)
define i64 @f(ptr addrspace(1) %root, ptr %fields) #0 gc "statepoint-example" {
entry:
  %slot = alloca ptr addrspace(1), align 8
  %a = load i64, ptr %fields
  %valid = icmp sge i64 %a, 0
  br i1 %valid, label %guard, label %outside
guard:
  %other = getelementptr i64, ptr %fields, i64 1
  store i64 9, ptr %other
  %b = load i64, ptr %fields
  %bad = icmp slt i64 %b, 0
  br i1 %bad, label %failure, label %success
success:
  call void @tick() #1
  call void (...) @llvm.fake.use(ptr addrspace(1) %root), !scoop.statepoint-root-identity !0
  store volatile ptr addrspace(1) %root, ptr %slot
  %restored = load volatile ptr addrspace(1), ptr %slot
  %value = load i64, ptr addrspace(1) %restored
  %sum = add i64 %value, %b
  ret i64 %sum
failure:
  call void @tick() #2
  call void (...) @llvm.fake.use(ptr addrspace(1) %root), !scoop.statepoint-root-identity !1
  store volatile ptr addrspace(1) %root, ptr %slot
  ret i64 -9
outside:
  ret i64 -1
}
attributes #0 = { "disable-tail-calls"="true" "frame-pointer"="all" }
attributes #1 = { "statepoint-id"="7" }
attributes #2 = { "statepoint-id"="8" }
!0 = !{i64 7, i64 0, i64 0, i64 0}
!1 = !{i64 8, i64 0, i64 0, i64 0}
