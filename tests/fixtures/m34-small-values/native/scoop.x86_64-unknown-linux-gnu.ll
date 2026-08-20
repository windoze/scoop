%Mixed = type { double, i64 }
declare void @abort() noreturn
declare void @scoop_runtime_gc_collect()

define %Mixed @small_mixed(double %number, i64 %tag) nounwind {
    %first = insertvalue %Mixed zeroinitializer, double %number, 0
    %result = insertvalue %Mixed %first, i64 %tag, 1
    ret %Mixed %result
}

define <2 x float> @small_singles(<2 x float> %value) nounwind {
    ret <2 x float> %value
}

define i24 @small_three(i24 %value) nounwind {
    ret i24 %value
}

define { i64, i64 } @small_padded(i64 %first, i64 %second) nounwind {
    %padding = lshr i64 %first, 8
    %valid = icmp eq i64 %padding, 0
    br i1 %valid, label %ok, label %bad
ok:
    %a = insertvalue { i64, i64 } zeroinitializer, i64 %first, 0
    %b = insertvalue { i64, i64 } %a, i64 %second, 1
    ret { i64, i64 } %b
bad:
    call void @abort()
    unreachable
}

define %Mixed @small_registers(i64 %a, i64 %b, i64 %c, i64 %d, i64 %e, i64 %f, ptr byval(%Mixed) align 8 %value, double %tail) nounwind {
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
    %result = load %Mixed, ptr %value, align 8
    ret %Mixed %result
bad:
    call void @abort()
    unreachable
}

define %Mixed @small_collect(double %number, i64 %tag) {
    call void @scoop_runtime_gc_collect()
    %first = insertvalue %Mixed zeroinitializer, double %number, 0
    %result = insertvalue %Mixed %first, i64 %tag, 1
    ret %Mixed %result
}
