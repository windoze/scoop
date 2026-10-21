declare void @abort() noreturn
declare void @scoop_runtime_gc_collect()

define [2 x i64] @small_mixed([2 x i64] %value) nounwind {
    ret [2 x i64] %value
}

define [2 x float] @small_singles([2 x float] %value) nounwind {
    ret [2 x float] %value
}

define i24 @small_three(i64 %value) nounwind {
    %padding = lshr i64 %value, 24
    %valid = icmp eq i64 %padding, 0
    br i1 %valid, label %ok, label %bad
ok:
    %result = trunc i64 %value to i24
    ret i24 %result
bad:
    call void @abort()
    unreachable
}

define [2 x i64] @small_padded([2 x i64] %value) nounwind {
    %first = extractvalue [2 x i64] %value, 0
    %padding = lshr i64 %first, 8
    %valid = icmp eq i64 %padding, 0
    br i1 %valid, label %ok, label %bad
ok:
    ret [2 x i64] %value
bad:
    call void @abort()
    unreachable
}

define [2 x i64] @small_registers(i64 %a, i64 %b, i64 %c, i64 %d, i64 %e, i64 %f, [2 x i64] %value, double %tail) nounwind {
    %ab = add i64 %a, %b
    %cd = add i64 %c, %d
    %ef = add i64 %e, %f
    %abcd = add i64 %ab, %cd
    %sum = add i64 %abcd, %ef
    %head_ok = icmp eq i64 %sum, 21
    %tail_ok = fcmp oeq double %tail, 2.5
    %valid = and i1 %head_ok, %tail_ok
    br i1 %valid, label %ok, label %bad
ok:
    ret [2 x i64] %value
bad:
    call void @abort()
    unreachable
}

define [2 x i64] @small_collect([2 x i64] %value) {
    call void @scoop_runtime_gc_collect()
    ret [2 x i64] %value
}
