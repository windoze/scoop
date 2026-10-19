declare void @yes(i64) "gc-leaf-function"
declare void @no(i64) "gc-leaf-function"

define void @prefix(ptr %data, i64 %count) {
entry:
  %has = icmp sgt i64 %count, 1
  br i1 %has, label %loop, label %exit
loop:
  %i = phi i64 [1, %entry], [%next, %loop]
  %previous = sub i64 %i, 1
  %source = getelementptr i64, ptr %data, i64 %previous
  %value = load i64, ptr %source, align 8
  %sum = add i64 %value, %i
  %destination = getelementptr i64, ptr %data, i64 %i
  store i64 %sum, ptr %destination, align 8
  %next = add i64 %i, 1
  %more = icmp slt i64 %next, %count
  br i1 %more, label %loop, label %exit
exit:
  ret void
}

define void @small(ptr %data) {
entry:
  br label %loop
loop:
  %i = phi i64 [0, %entry], [%next, %loop]
  %address = getelementptr i64, ptr %data, i64 %i
  %value = load i64, ptr %address, align 8
  %shift = shl i64 %value, 5
  %mixed = xor i64 %value, %shift
  store i64 %mixed, ptr %address, align 8
  %next = add i64 %i, 1
  %more = icmp slt i64 %next, 4
  br i1 %more, label %loop, label %exit
exit:
  ret void
}

define void @unswitch(i1 %flag, i64 %count) {
entry:
  %has = icmp sgt i64 %count, 0
  br i1 %has, label %loop, label %exit
loop:
  %i = phi i64 [0, %entry], [%next, %latch]
  br i1 %flag, label %true, label %false
true:
  call void @yes(i64 %i)
  br label %latch
false:
  call void @no(i64 %i)
  br label %latch
latch:
  %next = add i64 %i, 1
  %more = icmp slt i64 %next, %count
  br i1 %more, label %loop, label %exit
exit:
  ret void
}
