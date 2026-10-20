declare void @callee()
declare void @failure()
declare void @llvm.fake.use(...)
define i64 @f(ptr addrspace(1) %root, i64 %i, i64 %n, i64 %m) #0 gc "statepoint-example" {
entry:
  %slot = alloca ptr addrspace(1), align 8
  %first = icmp slt i64 %i, %n
  br i1 %first, label %next, label %outside
next:
  %second = icmp sle i64 %n, %m
  br i1 %second, label %check, label %outside
check:
  %redundant = icmp slt i64 %i, %m
  br i1 %redundant, label %success, label %failure
success:
  call void @callee() #1
  call void (...) @llvm.fake.use(ptr addrspace(1) %root), !scoop.statepoint-root-identity !0
  store volatile ptr addrspace(1) %root, ptr %slot
  %restored = load volatile ptr addrspace(1), ptr %slot
  %value = load i64, ptr addrspace(1) %restored
  ret i64 %value
failure:
  call void @failure() #2
  call void (...) @llvm.fake.use(ptr addrspace(1) %root), !scoop.statepoint-root-identity !1
  store volatile ptr addrspace(1) %root, ptr %slot
  ret i64 -2
outside:
  ret i64 -1
}
attributes #0 = { "disable-tail-calls"="true" "frame-pointer"="all" }
attributes #1 = { "statepoint-id"="7" }
attributes #2 = { "statepoint-id"="8" }
!0 = !{i64 7, i64 0, i64 0, i64 0}
!1 = !{i64 8, i64 0, i64 0, i64 0}
