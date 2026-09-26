declare i64 @check()
define i32 @main() {
entry:
  %result = call i64 @check()
  %failed = icmp ne i64 %result, 83
  %status = zext i1 %failed to i32
  ret i32 %status
}
