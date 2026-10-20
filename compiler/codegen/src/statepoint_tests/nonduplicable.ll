declare void @callee(i1, ptr addrspace(1))
declare void @left(ptr addrspace(1))
declare void @right(ptr addrspace(1))
declare void @llvm.fake.use(...)

define void @f(ptr addrspace(1) %root, i1 %first, i64 %number, i64 %count) #0 gc "statepoint-example" {
entry:
  %slot = alloca ptr addrspace(1), align 8
  br i1 %first, label %check, label %start
check:
  %second = icmp eq i64 %number, 1
  br label %start
start:
  %condition = phi i1 [ false, %entry ], [ %second, %check ]
  br i1 %condition, label %yes, label %no
yes:
  br label %merge
no:
  br label %merge
merge:
  %expected = phi i64 [ 0, %yes ], [ 1, %no ]
  %equal = icmp eq i64 %count, %expected
  call void @callee(i1 %equal, ptr addrspace(1) %root) #1
  call void (...) @llvm.fake.use(ptr addrspace(1) %root), !scoop.statepoint-root-identity !0
  store volatile ptr addrspace(1) %root, ptr %slot, align 8
  br i1 %condition, label %a, label %b
a:
  call void @left(ptr addrspace(1) %root) #2
  ret void
b:
  call void @right(ptr addrspace(1) %root) #2
  ret void
}

attributes #0 = { "disable-tail-calls"="true" "frame-pointer"="all" }
attributes #1 = { "statepoint-id"="7" }
attributes #2 = { "gc-leaf-function" }
!0 = !{i64 7, i64 0, i64 0, i64 0}
