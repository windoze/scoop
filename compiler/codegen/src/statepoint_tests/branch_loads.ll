declare void @callee(i64)
declare void @llvm.fake.use(...)

define void @f(ptr %source, ptr %result, ptr addrspace(1) %replacement, i1 %condition) #0 gc "statepoint-example" {
entry:
  %first = load ptr addrspace(1), ptr %source, align 8
  br i1 %condition, label %left, label %right
left:
  store ptr addrspace(1) %replacement, ptr %source, align 8
  br label %merge
right:
  br label %merge
merge:
  %width = phi i64 [ 1, %left ], [ 2, %right ]
  %second = load ptr addrspace(1), ptr %source, align 8
  call void @callee(i64 %width) #1
  call void (...) @llvm.fake.use(ptr addrspace(1) %first), !scoop.statepoint-root-identity !0
  store volatile ptr addrspace(1) %first, ptr %source, align 8
  call void (...) @llvm.fake.use(ptr addrspace(1) %second), !scoop.statepoint-root-identity !1
  store volatile ptr addrspace(1) %second, ptr %result, align 8
  ret void
}

attributes #0 = { "disable-tail-calls"="true" "frame-pointer"="all" }
attributes #1 = { "statepoint-id"="7" }
!0 = !{i64 7, i64 1, i64 0, i64 0}
!1 = !{i64 7, i64 1, i64 1, i64 0}
