define void @f(ptr %source, ptr %destination, i64 %count) {
entry:
  br label %condition
condition:
  %index = phi i64 [0, %entry], [%next, %body]
  %more = icmp slt i64 %index, %count
  br i1 %more, label %body, label %exit
body:
  %from = getelementptr i32, ptr %source, i64 %index
  %value = load i32, ptr %from, align 4
  %scaled = mul i32 %value, 3
  %result = add i32 %scaled, 7
  %to = getelementptr i32, ptr %destination, i64 %index
  store i32 %result, ptr %to, align 4
  %next = add i64 %index, 1
  br label %condition
exit:
  ret void
}
