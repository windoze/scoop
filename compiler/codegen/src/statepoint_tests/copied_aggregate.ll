declare void @fill(ptr) "gc-leaf-function"
declare void @consume(ptr)
declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1 immarg)
declare void @llvm.fake.use(...)

define ptr addrspace(1) @f() #0 gc "statepoint-example" {
entry:
  %source = alloca {ptr addrspace(1)}, align 8
  %copy = alloca {ptr addrspace(1)}, align 8
  call void @fill(ptr %source)
  call void @llvm.memcpy.p0.p0.i64(ptr %copy, ptr %source, i64 8, i1 false)
  %root = load ptr addrspace(1), ptr %copy, align 8
  call void @consume(ptr byval({ptr addrspace(1)}) align 8 %copy) #1
  call void (...) @llvm.fake.use(ptr addrspace(1) %root), !scoop.statepoint-root-identity !0
  store volatile ptr addrspace(1) %root, ptr %copy, align 8
  %updated = load volatile ptr addrspace(1), ptr %copy, align 8
  ret ptr addrspace(1) %updated
}

attributes #0 = { noredzone "disable-tail-calls"="true" "frame-pointer"="all" }
attributes #1 = { "statepoint-id"="7" }
!0 = !{i64 7, i64 1, i64 0, i64 0}
