mod support;
use support::checked;

#[test]
fn declarations_and_blocks() {
    assert_eq!(
        checked("fun add(a:Int,b:Int):Int {return a+b}"),
        "fun add(a: Int, b: Int): Int {\n    return a + b\n}\n"
    );
    assert_eq!(
        checked("package example\nimport a.B\nimport c.D\nfun f(){}\nfun g(){}"),
        "package example\n\nimport a.B\nimport c.D\n\nfun f() {}\n\nfun g() {}\n"
    );
    assert_eq!(checked(" \t\r\n"), "");
}

#[test]
fn whitespace_sensitive_expressions() {
    for source in [
        "fun f(){ return\n value\n call\n (x)\n array\n [1,2]\n array[0] }",
        "fun f(){ val x=- -1; val y=+ +2; val z=!!flag; a++; --a; val v=a!!; val b=v !is T; val c=v as? T }",
        "fun f(){ outer@ while(true){return@outer value} }",
        "fun f(){ if(a) if(b) foo() else bar()\n do call() while(ready) }",
        "fun <T> f(a: Box<T?>): Box<Box<T>> = call<T>(a)",
        "fun f(){ val x=thing.{a:1,b:2}; val y=items[0,1]; call(*items) { x -> x+1 } }",
        "fun f(){ when(x) { 1 -> yes()\n 2 -> no()\n else -> other() } }",
    ] {
        checked(source);
    }
}

#[test]
fn comments_strings_and_newlines_are_verbatim() {
    for source in [
        "#!/usr/bin/env scoop\n/// doc\n@Tag public fun f(){ // tail\n val s=\"a  b\"\n /** unattached */ s\n }\n/** eof */",
        "fun f(){ val s=\"\"\"a\r\n  b\n c\"\"\"; val t=f\"text ${1+2} tail\" }",
        "fun f() = f\"${a}${b}\"",
        "/** comment\r\n  * raw\n*/\nfun f(){return 1}\n",
        "fun f(){\r\nreturn 1\r\n}\r\n",
        "fun f(){\nreturn 1\r\n}\n",
    ] {
        checked(source);
    }
}

#[test]
fn lists_break_at_unicode_scalar_width() {
    let name = "参数".repeat(10);
    let source = format!("fun f({name}: Int, {name}b: Int, {name}c: Int) {{}}");
    let formatted = checked(&source);
    assert_eq!(formatted.lines().count(), 1, "{formatted}");
    let source = format!("fun longer({name}: Int, {name}b: Int, {name}c: Int, {name}d: Int) {{}}");
    let formatted = checked(&source);
    assert!(formatted.contains("(\n    "), "{formatted}");
    assert!(formatted.contains("\n) {}"), "{formatted}");
}

#[test]
fn callable_bodies_annotations_and_control_layout() {
    for (source, expected) in [
        ("fun f()=apply {x->x+1}", "fun f() = apply { x -> x + 1 }\n"),
        (
            "@First @Second public fun f()=1",
            "@First\n@Second\npublic fun f() = 1\n",
        ),
        (
            "fun f(){if(x)call() else other()}",
            "fun f() {\n    if (x)\n        call()\n    else\n        other()\n}\n",
        ),
        (
            "fun f(){when(x){1->a()\nelse->b()}}",
            "fun f() {\n    when (x) {\n        1 -> a()\n        else -> b()\n    }\n}\n",
        ),
        ("fun f(){return\n  1}", "fun f() {\n    return\n    1\n}\n"),
        ("fun f() =\n 1+2", "fun f() = 1 + 2\n"),
    ] {
        assert_eq!(checked(source), expected);
    }
}

#[test]
fn long_expressions_and_comments_keep_grammar_boundaries() {
    for source in [
        "fun f() = veryLongValueName + anotherLongValueName + yetAnotherLongValueName + finalLongValueName + lastLongValueName",
        "fun f() = items and (if (ready) 63 else 7)",
        "fun f(a: Int, // parameter\n b: Int) { call(a, // argument\n b) }",
        "class C { fun first(){}\nfun second(){} }",
        "fun f(){ call(1,2)\nval tuple=(1,2,)\nval array=[1,2] }",
        "fun f() = apply { x -> first(x); second(x) }",
        "fun f() = apply { x -> ({ y -> x + y }) }",
        "/// first\n/** second */\nfun f(/** parameter */ value: Int) {}\n/** unbound */",
    ] {
        checked(source);
    }
    assert_eq!(
        checked("fun f(){\r\nreturn 1\r\n}\r\n"),
        "fun f() {\r\n    return 1\r\n}\r\n"
    );
}

#[test]
fn block_imports_stay_in_place_and_preserve_statement_separators() {
    for source in [
        "fun f(){import api.pick as selected;selected(); {import api.*;pick()}; selected()}",
        "fun f()=apply { x -> import api.*; x.convert() }",
        "fun f(){call();\n// local\nimport api.pick\npick()}",
    ] {
        let formatted = checked(source);
        assert!(formatted.starts_with("fun f("), "{formatted}");
        assert!(formatted.contains("import api."), "{formatted}");
    }
}
