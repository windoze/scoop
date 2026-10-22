declare i1 @tick()
declare void @observe(ptr addrspace(1), i64) "gc-leaf-function"
declare void @llvm.fake.use(...)
define void @f(ptr addrspace(1) %root, ptr %fields) #0 gc "statepoint-example" {
entry:
  %slot = alloca ptr addrspace(1), align 8
  %empty = icmp eq ptr addrspace(1) %root, null
  br i1 %empty, label %null.path, label %join
null.path:
  %stop = call i1 @tick() #1
  call void (...) @llvm.fake.use(ptr addrspace(1) null) #2, !scoop.statepoint-root-identity !0
  store volatile ptr addrspace(1) null, ptr %slot
  br i1 %stop, label %exit, label %join
join:
  %field = load volatile i64, ptr %fields
  br i1 %empty, label %exit, label %use
use:
  call void @observe(ptr addrspace(1) %root, i64 %field)
  br label %exit
exit:
  ret void
}
attributes #0 = { "disable-tail-calls"="true" "frame-pointer"="all" }
attributes #1 = { noduplicate "statepoint-id"="7" }
attributes #2 = { noduplicate }
!0 = !{i64 7, i64 0, i64 0, i64 0}
