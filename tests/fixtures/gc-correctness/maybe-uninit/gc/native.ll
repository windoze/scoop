declare void @m34_maybe_copy_gc(ptr, ptr, i64)
declare i8 @m34_maybe_is_zero(ptr)

define void @m34_maybe_collect(ptr sret([16 x i8]) align 8 %result, ptr byval([16 x i8]) align 8 %value) nounwind {
    call void @m34_maybe_copy_gc(ptr %result, ptr %value, i64 8)
    ret void
}

define void @m34_maybe_collect_interface(ptr sret([16 x i8]) align 8 %result, ptr byval([16 x i8]) align 8 %value) nounwind {
    call void @m34_maybe_copy_gc(ptr %result, ptr %value, i64 0)
    ret void
}

define i1 @m34_maybe_zero(ptr byval([16 x i8]) align 8 %value) nounwind {
    %byte = call i8 @m34_maybe_is_zero(ptr %value)
    %value_is_zero = icmp ne i8 %byte, 0
    ret i1 %value_is_zero
}
