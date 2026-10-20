declare void @tick()
declare void @llvm.fake.use(...)

define i32 @f(ptr addrspace(1) %root, i32 %count) #0 gc "statepoint-example" {
entry:
  %slot = alloca ptr addrspace(1), align 8
  call void @tick() #1
  call void (...) @llvm.fake.use(ptr addrspace(1) %root), !scoop.statepoint-root-identity !0
  store volatile ptr addrspace(1) %root, ptr %slot, align 8
  br label %outer
outer:
  %start = phi i32 [0, %entry], [%next, %inner]
  %initial = phi i32 [0, %entry], [%accumulated, %inner]
  %before = load ptr addrspace(1), ptr %slot, align 8
  call void @tick() #2
  call void (...) @llvm.fake.use(ptr addrspace(1) %before), !scoop.statepoint-root-identity !1
  store volatile ptr addrspace(1) %before, ptr %slot, align 8
  %more = icmp slt i32 %start, %count
  br i1 %more, label %chunk, label %exit
chunk:
  br label %inner
inner:
  %left = phi i32 [64, %chunk], [%remaining, %inner]
  %i = phi i32 [%start, %chunk], [%next, %inner]
  %sum = phi i32 [%initial, %chunk], [%accumulated, %inner]
  %scaled = mul i32 %i, 1664525
  %biased = add i32 %scaled, 1013904223
  %shifted = lshr i32 %i, 8
  %mixed = xor i32 %biased, %shifted
  %accumulated = add i32 %sum, %mixed
  %next = add i32 %i, 1
  %remaining = sub i32 %left, 1
  %again = icmp ne i32 %remaining, 0
  br i1 %again, label %inner, label %outer
exit:
  %restored = load volatile ptr addrspace(1), ptr %slot, align 8
  %field = load i32, ptr addrspace(1) %restored, align 4
  %result = add i32 %initial, %field
  ret i32 %result
}
attributes #0 = { "disable-tail-calls"="true" "frame-pointer"="all" }
attributes #1 = { "statepoint-id"="7" }
attributes #2 = { "statepoint-id"="8" }
!0 = !{i64 7, i64 0, i64 0, i64 0}
!1 = !{i64 8, i64 0, i64 0, i64 0}
