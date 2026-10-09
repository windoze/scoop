%Pair = type { i64, i64 }
define void @pair_step(ptr sret(%Pair) align 8 %result, ptr byval(%Pair) align 8 %source) nounwind {
    %value = load %Pair, ptr %source, align 8
    %first = extractvalue %Pair %value, 0
    %next = add i64 %first, 1
    %updated = insertvalue %Pair %value, i64 %next, 0
    store %Pair %updated, ptr %result, align 8
    ret void
}
