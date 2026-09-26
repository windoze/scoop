; Symbols come from the completed provider and consumer artifacts.
%Wide = type { i64, i64, i64 }
%Nested = type { %Wide }
@input = private constant %Wide { i64 17, i64 -29, i64 53 }, align 8

declare void @f7()
declare void @f8(ptr sret(%Wide) align 8, ptr byval(%Wide) align 8)
declare i64 @f9(ptr byval(%Wide) align 8)
declare i64 @f10(ptr byval(%Wide) align 8)
declare void @f11(ptr byval(%Nested) align 8)
declare i64 @f12(ptr byval(%Nested) align 8)
declare i64 @f13(ptr byval(%Wide) align 8)
declare i64 @f14(ptr byval(%Wide) align 8)

define i32 @main() {
entry:
  %value = alloca %Wide, align 8
  call void @f7()
  call void @f8(ptr sret(%Wide) align 8 %value, ptr byval(%Wide) align 8 @input)
  %sum = call i64 @f9(ptr byval(%Wide) align 8 %value)
  %first = call i64 @f10(ptr byval(%Wide) align 8 %value)
  call void @f11(ptr byval(%Nested) align 8 @input)
  %nested = call i64 @f12(ptr byval(%Nested) align 8 @input)
  %fieldSum = call i64 @f13(ptr byval(%Wide) align 8 %value)
  %computedTotal = call i64 @f14(ptr byval(%Wide) align 8 %value)
  %p1 = getelementptr %Wide, ptr %value, i32 0, i32 1
  %p2 = getelementptr %Wide, ptr %value, i32 0, i32 2
  %v0 = load i64, ptr %value, align 8
  %v1 = load i64, ptr %p1, align 8
  %v2 = load i64, ptr %p2, align 8
  %ok0 = icmp eq i64 %v0, 17
  %ok1 = icmp eq i64 %v1, -29
  %ok2 = icmp eq i64 %v2, 53
  %ok3 = icmp eq i64 %sum, 48
  %ok4 = icmp eq i64 %first, 17
  %ok5 = icmp eq i64 %nested, 17
  %ok6 = icmp eq i64 %fieldSum, 48
  %ok7 = icmp eq i64 %computedTotal, 48
  %pair0 = and i1 %ok0, %ok1
  %pair1 = and i1 %ok2, %ok3
  %old = and i1 %pair0, %pair1
  %fields = and i1 %ok4, %ok5
  %sums = and i1 %ok6, %ok7
  %properties = and i1 %fields, %sums
  %all = and i1 %old, %properties
  %failed = xor i1 %all, true
  %status = zext i1 %failed to i32
  ret i32 %status
}
