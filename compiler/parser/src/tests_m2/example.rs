use super::*;

// --- the M2 design example end to end --------------------------------------

#[test]
fn design_example() {
    let source = "struct Point(val x: Int, val y: Int)\n\
\n\
fun main() {\n\
\u{20}   val p = Point(1, 2)\n\
\u{20}   var q = (1, \"hello\")\n\
\u{20}   val x = p.x\n\
\u{20}   var n = 0\n\
\u{20}   while (n < 3) {\n\
\u{20}       n = n + 1\n\
\u{20}   }\n\
\u{20}   if (p == Point(1, 2) && x > 0) {\n\
\u{20}       println(\"ok\")\n\
\u{20}   } else {\n\
\u{20}       println(\"ng\")\n\
\u{20}   }\n\
\u{20}   val u = ()\n\
\u{20}   val s = (42,)\n\
}\n";
    let file = ok(source);
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  struct Point\n    field x: Int\n    field y: Int\n  fun main()\n    val p\n      Call Point\n        IntLiteral 1\n        IntLiteral 2\n    var q\n      TupleLiteral\n        IntLiteral 1\n        StringLiteral \"hello\"\n    val x\n      FieldAccess x\n        Var p\n    var n\n      IntLiteral 0\n    while\n      Binary Lt\n        Var n\n        IntLiteral 3\n      assign n\n        Binary Add\n          Var n\n          IntLiteral 1\n    if\n      Binary And\n        Binary Eq\n          Var p\n          Call Point\n            IntLiteral 1\n            IntLiteral 2\n        Binary Gt\n          Var x\n          IntLiteral 0\n      Call println\n        StringLiteral \"ok\"\n    else\n      Call println\n        StringLiteral \"ng\"\n    val u\n      UnitLiteral\n    val s\n      TupleLiteral\n        IntLiteral 42\n"
    );
}
