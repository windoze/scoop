%Descriptor = type { i64, i64 }

$owned_odr = comdat any
@owned = hidden global i64 11
@owned_odr = weak_odr hidden constant [2 x i64] [i64 9, i64 7], comdat
@projection = weak_odr hidden alias i64, getelementptr ([2 x i64], ptr @owned_odr, i64 0, i64 1)
@dead_external = external hidden constant %Descriptor
@dead_planned = external hidden constant %Descriptor
@dead_direct = external hidden constant i64
@live_external = external hidden constant %Descriptor

declare hidden i64 @dead_callee()
declare hidden i64 @live_callee(i64)

define hidden i32 @main() {
entry:
  %take_dead = icmp eq i64 0, 1
  br i1 %take_dead, label %dead, label %live
dead:
  %a = load volatile i64, ptr getelementptr (%Descriptor, ptr @dead_external, i64 0, i32 1)
  %b = load volatile i64, ptr getelementptr (%Descriptor, ptr @dead_planned, i64 0, i32 1)
  %c = load volatile i64, ptr @dead_direct
  %d = call i64 @dead_callee()
  %sum = add i64 %a, %b
  %extra = add i64 %c, %d
  %value = add i64 %sum, %extra
  %dead_result = trunc i64 %value to i32
  ret i32 %dead_result
live:
  %field = load i64, ptr getelementptr (%Descriptor, ptr @live_external, i64 0, i32 1)
  %result = call i64 @live_callee(i64 %field)
  %status = sub i64 %result, 42
  %exit = trunc i64 %status to i32
  ret i32 %exit
}
