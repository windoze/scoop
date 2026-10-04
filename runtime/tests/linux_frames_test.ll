; Exercise the actual LLVM 22.1 stack map and SysV entry ABI at O0 and O2.
declare void @scoop_rt_safepoint()
declare void @scoop_rt_box_zst(i64)
declare void @scoop_rt_box_value(i64, i64)
declare void @scoop_rt_array_clone(i64, i64, i64)
declare token @llvm.experimental.gc.statepoint.p0(i64 immarg, i32 immarg, ptr, i32 immarg, i32 immarg, ...)
declare ptr addrspace(1) @llvm.experimental.gc.relocate.p1(token, i32 immarg, i32 immarg)

define ptr addrspace(1) @probe0(ptr addrspace(1) %root) #0 gc "statepoint-example" {
  %t = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 1, i32 0, ptr elementtype(void ()) @scoop_rt_safepoint, i32 0, i32 0, i32 0, i32 0) [ "gc-live"(ptr addrspace(1) %root) ]
  %r = call coldcc ptr addrspace(1) @llvm.experimental.gc.relocate.p1(token %t, i32 0, i32 0)
  ret ptr addrspace(1) %r
}

define ptr addrspace(1) @probe1(ptr addrspace(1) %root) #0 gc "statepoint-example" {
  %t = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 2, i32 0, ptr elementtype(void (i64)) @scoop_rt_box_zst, i32 1, i32 0, i64 11, i32 0, i32 0) [ "gc-live"(ptr addrspace(1) %root) ]
  %r = call coldcc ptr addrspace(1) @llvm.experimental.gc.relocate.p1(token %t, i32 0, i32 0)
  ret ptr addrspace(1) %r
}

define ptr addrspace(1) @probe2(ptr addrspace(1) %root) #0 gc "statepoint-example" {
  %t = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 3, i32 0, ptr elementtype(void (i64, i64)) @scoop_rt_box_value, i32 2, i32 0, i64 11, i64 22, i32 0, i32 0) [ "gc-live"(ptr addrspace(1) %root) ]
  %r = call coldcc ptr addrspace(1) @llvm.experimental.gc.relocate.p1(token %t, i32 0, i32 0)
  ret ptr addrspace(1) %r
}

define ptr addrspace(1) @probe3(ptr addrspace(1) %root) #0 gc "statepoint-example" {
  %t = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 4, i32 0, ptr elementtype(void (i64, i64, i64)) @scoop_rt_array_clone, i32 3, i32 0, i64 11, i64 22, i64 33, i32 0, i32 0) [ "gc-live"(ptr addrspace(1) %root) ]
  %r = call coldcc ptr addrspace(1) @llvm.experimental.gc.relocate.p1(token %t, i32 0, i32 0)
  ret ptr addrspace(1) %r
}

define ptr addrspace(1) @outer(ptr addrspace(1) %root) #0 gc "statepoint-example" {
  %t = call token (i64, i32, ptr, i32, i32, ...) @llvm.experimental.gc.statepoint.p0(i64 5, i32 0, ptr elementtype(ptr addrspace(1) (ptr addrspace(1))) @probe0, i32 1, i32 0, ptr addrspace(1) %root, i32 0, i32 0) [ "gc-live"(ptr addrspace(1) %root) ]
  %r = call coldcc ptr addrspace(1) @llvm.experimental.gc.relocate.p1(token %t, i32 0, i32 0)
  ret ptr addrspace(1) %r
}

%CharResult = type { i64, i32, i32 }
%BoundsResult = type { i64, i64, i64 }
declare void @scoop_rt_string_get(ptr sret(%CharResult) align 8, ptr, i64)
declare void @scoop_rt_string_slice_bounds(ptr sret(%BoundsResult) align 8, ptr, i64, i64)

define i64 @string_results(ptr %string) #0 {
  %char = alloca %CharResult, align 8
  %bounds = alloca %BoundsResult, align 8
  call void @scoop_rt_string_get(ptr sret(%CharResult) align 8 %char, ptr %string, i64 1)
  call void @scoop_rt_string_slice_bounds(ptr sret(%BoundsResult) align 8 %bounds, ptr %string, i64 1, i64 2)
  %tag0 = load i64, ptr %char
  %valuep = getelementptr %CharResult, ptr %char, i32 0, i32 1
  %value = load i32, ptr %valuep
  %tag1 = load i64, ptr %bounds
  %startp = getelementptr %BoundsResult, ptr %bounds, i32 0, i32 1
  %endp = getelementptr %BoundsResult, ptr %bounds, i32 0, i32 2
  %start = load i64, ptr %startp
  %end = load i64, ptr %endp
  %ok0 = icmp eq i64 %tag0, 0
  %ok1 = icmp eq i64 %tag1, 0
  %ok2 = icmp eq i32 %value, 20013
  %ok3 = icmp eq i64 %start, 1
  %ok4 = icmp eq i64 %end, 4
  %a = and i1 %ok0, %ok1
  %b = and i1 %ok2, %ok3
  %c = and i1 %a, %b
  %d = and i1 %c, %ok4
  %result = zext i1 %d to i64
  ret i64 %result
}

attributes #0 = { noinline noredzone "frame-pointer"="all" "disable-tail-calls"="true" }
