%Wide = type { i64, i64, i64 }
declare void @primary(ptr sret(%Wide) align 8)
declare i64 @check()
define i32 @main() {
entry:
  %value = alloca %Wide, align 8
  call void @primary(ptr sret(%Wide) align 8 %value)
  %p1 = getelementptr %Wide, ptr %value, i32 0, i32 1
  %p2 = getelementptr %Wide, ptr %value, i32 0, i32 2
  %v0 = load i64, ptr %value, align 8
  %v1 = load i64, ptr %p1, align 8
  %v2 = load i64, ptr %p2, align 8
  %sum = call i64 @check()
  %ok0 = icmp eq i64 %v0, 17
  %ok1 = icmp eq i64 %v1, -29
  %ok2 = icmp eq i64 %v2, 53
  %ok3 = icmp eq i64 %sum, 83
  %a = and i1 %ok0, %ok1
  %b = and i1 %ok2, %ok3
  %all = and i1 %a, %b
  %failed = xor i1 %all, true
  %status = zext i1 %failed to i32
  ret i32 %status
}
