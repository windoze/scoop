source_filename = "metadata.scoop"

@value = global i64 23, align 8, !dbg !0
@zero = global [4 x i64] zeroinitializer, align 32
@tls = thread_local global i64 37, align 8
@external = external global i8
@pointers = constant [4 x ptr] [ptr @value, ptr @external, ptr @callee, ptr getelementptr (i8, ptr @zero, i64 8)]
@alias = alias i64, ptr @value
declare void @callee()

!llvm.dbg.cu = !{!2}
!llvm.module.flags = !{!5, !6}
!0 = !DIGlobalVariableExpression(var: !1, expr: !DIExpression())
!1 = distinct !DIGlobalVariable(name: "value", scope: !2, file: !3, line: 1, type: !4, isLocal: false, isDefinition: true)
!2 = distinct !DICompileUnit(language: DW_LANG_C, file: !3, producer: "scoop", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug, globals: !7)
!3 = !DIFile(filename: "metadata.scoop", directory: ".")
!4 = !DIBasicType(name: "Long", size: 64, encoding: DW_ATE_signed)
!5 = !{i32 2, !"Dwarf Version", i32 5}
!6 = !{i32 2, !"Debug Info Version", i32 3}
!7 = !{!0}
