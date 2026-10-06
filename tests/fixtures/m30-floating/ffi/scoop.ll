%Pair = type { float, double }

define void @scoop_pair(ptr sret(%Pair) align 8 %result, ptr byval(%Pair) align 8 %source) nounwind {
entry:
    %value = load %Pair, ptr %source, align 8
    store %Pair %value, ptr %result, align 8
    ret void
}
