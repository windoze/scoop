; Symbols come from the completed provider and consumer artifacts.
%Wide = type { i64, i64, i64 }
@input = private constant %Wide { i64 17, i64 -29, i64 53 }, align 8

declare void @f7()
declare void @f8(ptr sret(%Wide) align 8, ptr byval(%Wide) align 8)
declare i64 @f9(ptr byval(%Wide) align 8)

define i32 @main() {
entry:
  %value = alloca %Wide, align 8
  call void @f7()
  call void @f8(ptr sret(%Wide) align 8 %value, ptr byval(%Wide) align 8 @input)
  %sum = call i64 @f9(ptr byval(%Wide) align 8 %value)
  %p1 = getelementptr %Wide, ptr %value, i32 0, i32 1
  %p2 = getelementptr %Wide, ptr %value, i32 0, i32 2
  %v0 = load i64, ptr %value, align 8
  %v1 = load i64, ptr %p1, align 8
  %v2 = load i64, ptr %p2, align 8
  %ok0 = icmp eq i64 %v0, 17
  %ok1 = icmp eq i64 %v1, -29
  %ok2 = icmp eq i64 %v2, 53
  %ok3 = icmp eq i64 %sum, 48
  %pair0 = and i1 %ok0, %ok1
  %pair1 = and i1 %ok2, %ok3
  %all = and i1 %pair0, %pair1
  %failed = xor i1 %all, true
  %status = zext i1 %failed to i32
  ret i32 %status
}
