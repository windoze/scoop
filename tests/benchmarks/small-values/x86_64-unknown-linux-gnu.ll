define { i64, i64 } @pair_step(i64 %first, i64 %second) nounwind {
    %next = add i64 %first, 1
    %a = insertvalue { i64, i64 } zeroinitializer, i64 %next, 0
    %result = insertvalue { i64, i64 } %a, i64 %second, 1
    ret { i64, i64 } %result
}
