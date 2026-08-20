declare void @tick()
declare void @llvm.fake.use(...)

define ptr addrspace(1) @f(ptr addrspace(1) %base, ptr addrspace(1) %expected, ptr addrspace(1) %value) #0 gc "statepoint-example" {
entry:
  call void @tick() #1
  call void (...) @llvm.fake.use(ptr addrspace(1) %base), !scoop.statepoint-root-identity !0
  call void (...) @llvm.fake.use(ptr addrspace(1) %expected), !scoop.statepoint-root-identity !1
  call void (...) @llvm.fake.use(ptr addrspace(1) %value), !scoop.statepoint-root-identity !2
  %address = getelementptr i8, ptr addrspace(1) %base, i64 16
  ATOMIC_OPERATION
  call void @tick() #2
  call void (...) @llvm.fake.use(ptr addrspace(1) %base), !scoop.statepoint-root-identity !3
  call void (...) @llvm.fake.use(ptr addrspace(1) %expected), !scoop.statepoint-root-identity !4
  call void (...) @llvm.fake.use(ptr addrspace(1) %value), !scoop.statepoint-root-identity !5
  call void (...) @llvm.fake.use(ptr addrspace(1) OBSERVED_VALUE), !scoop.statepoint-root-identity !6
  ret ptr addrspace(1) OBSERVED_VALUE
}

attributes #0 = { noredzone "disable-tail-calls"="true" "frame-pointer"="all" }
attributes #1 = { "statepoint-id"="7" }
attributes #2 = { "statepoint-id"="8" }
!0 = !{i64 7, i64 0, i64 0, i64 0}
!1 = !{i64 7, i64 0, i64 1, i64 0}
!2 = !{i64 7, i64 0, i64 2, i64 0}
!3 = !{i64 8, i64 0, i64 0, i64 0}
!4 = !{i64 8, i64 0, i64 1, i64 0}
!5 = !{i64 8, i64 0, i64 2, i64 0}
!6 = !{i64 8, i64 1, i64 0, i64 0}
