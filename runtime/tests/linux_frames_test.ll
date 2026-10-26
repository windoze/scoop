; Exercise the actual LLVM 22.1 stack map and SysV entry ABI at O0 and O2.
declare void @scoop_rt_safepoint()
declare void @scoop_rt_context_ensure_root(i64)
declare void @scoop_rt_string_concat(i64, i64)
declare void @scoop_rt_context_push(i64, i64, i64)
declare token @llvm.experimental.gc.statepoint.p0(i64 immarg, i32 immarg, ptr, i32 immarg, i32 immarg, ...)
declare ptr addrspace(1) @llvm.experimental.gc.relocate.p1(token, i32 immarg, i32 immarg)

define ptr addrspace(1) @probe0(ptr addrspace(1) %root) #0 gc "statepoint-example" {
  %t = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 1, i32 0, ptr elementtype(void ()) @scoop_rt_safepoint, i32 0, i32 0, i32 0, i32 0) [ "gc-live"(ptr addrspace(1) %root) ]
  %r = call coldcc ptr addrspace(1) @llvm.experimental.gc.relocate.p1(token %t, i32 0, i32 0)
  ret ptr addrspace(1) %r
}

define ptr addrspace(1) @probe1(ptr addrspace(1) %root) #0 gc "statepoint-example" {
  %t = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 2, i32 0, ptr elementtype(void (i64)) @scoop_rt_context_ensure_root, i32 1, i32 0, i64 11, i32 0, i32 0) [ "gc-live"(ptr addrspace(1) %root) ]
  %r = call coldcc ptr addrspace(1) @llvm.experimental.gc.relocate.p1(token %t, i32 0, i32 0)
  ret ptr addrspace(1) %r
}

define ptr addrspace(1) @probe2(ptr addrspace(1) %root) #0 gc "statepoint-example" {
  %t = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 3, i32 0, ptr elementtype(void (i64, i64)) @scoop_rt_string_concat, i32 2, i32 0, i64 11, i64 22, i32 0, i32 0) [ "gc-live"(ptr addrspace(1) %root) ]
  %r = call coldcc ptr addrspace(1) @llvm.experimental.gc.relocate.p1(token %t, i32 0, i32 0)
  ret ptr addrspace(1) %r
}

define ptr addrspace(1) @probe3(ptr addrspace(1) %root) #0 gc "statepoint-example" {
  %t = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 4, i32 0, ptr elementtype(void (i64, i64, i64)) @scoop_rt_context_push, i32 3, i32 0, i64 11, i64 22, i64 33, i32 0, i32 0) [ "gc-live"(ptr addrspace(1) %root) ]
  %r = call coldcc ptr addrspace(1) @llvm.experimental.gc.relocate.p1(token %t, i32 0, i32 0)
  ret ptr addrspace(1) %r
}

define ptr addrspace(1) @outer(ptr addrspace(1) %root) #0 gc "statepoint-example" {
  %t = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 5, i32 0, ptr elementtype(ptr addrspace(1) (ptr addrspace(1))) @probe0, i32 1, i32 0, ptr addrspace(1) %root, i32 0, i32 0) [ "gc-live"(ptr addrspace(1) %root) ]
  %r = call coldcc ptr addrspace(1) @llvm.experimental.gc.relocate.p1(token %t, i32 0, i32 0)
  ret ptr addrspace(1) %r
}

attributes #0 = { noinline noredzone "frame-pointer"="all" "disable-tail-calls"="true" }
