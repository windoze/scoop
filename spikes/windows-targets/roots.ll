; Win64 roots must use the SEH-reconstructed RSP/RBP, not SysV frame geometry.
declare void @relocate_probe(ptr)
declare token @llvm.experimental.gc.statepoint.p0(i64 immarg, i32 immarg, ptr, i32 immarg, i32 immarg, ...)
declare ptr addrspace(1) @llvm.experimental.gc.relocate.p1(token, i32 immarg, i32 immarg)

define ptr addrspace(1) @roots_probe(ptr addrspace(1) %object) #0 gc "statepoint-example" {
entry:
  %large = alloca [16384 x i8], align 16
  store volatile i8 42, ptr %large
  %token = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 100, i32 0, ptr elementtype(void (ptr)) @relocate_probe, i32 1, i32 0, ptr %large, i32 0, i32 0) ["gc-live"(ptr addrspace(1) %object)]
  %updated = call ptr addrspace(1) @llvm.experimental.gc.relocate.p1(token %token, i32 0, i32 0)
  ret ptr addrspace(1) %updated
}

attributes #0 = { noinline uwtable noredzone "disable-tail-calls"="true" "frame-pointer"="all" }
