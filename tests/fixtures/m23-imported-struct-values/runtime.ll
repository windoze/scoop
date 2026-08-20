; Explicit fixture bindings use the published typed callable symbols.
%Wide = type { i64, i64, i64 }
@input = private constant %Wide { i64 17, i64 -29, i64 53 }, align 8

declare void @f0()
declare void @f1(ptr sret(%Wide) align 8, ptr byval(%Wide) align 8)
declare void @f2()
declare void @f3(ptr sret(%Wide) align 8, ptr byval(%Wide) align 8)
declare void @f4()
declare void @f5(ptr sret(%Wide) align 8, ptr byval(%Wide) align 8)

define i32 @m23_struct_abi_check() {
entry:
  %a = alloca %Wide, align 8
  %b = alloca %Wide, align 8
  %c = alloca %Wide, align 8
  call void @f0()
  call void @f1(ptr sret(%Wide) align 8 %a, ptr byval(%Wide) align 8 @input)
  call void @f2()
  call void @f3(ptr sret(%Wide) align 8 %b, ptr byval(%Wide) align 8 %a)
  call void @f4()
  call void @f5(ptr sret(%Wide) align 8 %c, ptr byval(%Wide) align 8 %b)
  %p1 = getelementptr %Wide, ptr %c, i32 0, i32 1
  %p2 = getelementptr %Wide, ptr %c, i32 0, i32 2
  %v0 = load i64, ptr %c, align 8
  %v1 = load i64, ptr %p1, align 8
  %v2 = load i64, ptr %p2, align 8
  %ok0 = icmp eq i64 %v0, 17
  %ok1 = icmp eq i64 %v1, -29
  %ok2 = icmp eq i64 %v2, 53
  %pair = and i1 %ok0, %ok1
  %all = and i1 %pair, %ok2
  %failed = xor i1 %all, true
  %status = zext i1 %failed to i32
  ret i32 %status
}
