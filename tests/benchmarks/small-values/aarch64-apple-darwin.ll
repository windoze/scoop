define [2 x i64] @pair_step([2 x i64] %value) nounwind {
    %first = extractvalue [2 x i64] %value, 0
    %next = add i64 %first, 1
    %result = insertvalue [2 x i64] %value, i64 %next, 0
    ret [2 x i64] %result
}
