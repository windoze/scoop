use super::*;

/// 一个极简、可复现的伪随机数生成器（避免引入 `rand` 依赖）。
#[derive(Clone)]
struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        // xorshift 在 0 种子下会卡住；这里做一次扰动。
        let seed = if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        };
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        // https://en.wikipedia.org/wiki/Xorshift
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn gen_usize(&mut self, upper_exclusive: usize) -> usize {
        if upper_exclusive == 0 {
            return 0;
        }
        (self.next_u64() as usize) % upper_exclusive
    }
}

fn gen_source(rng: &mut XorShift64, max_len: usize) -> String {
    // parser 会先走 lexer，所以字符集同样偏向 token/分隔符/空白等。
    const CHARS: &[char] = &[
        ' ', '\t', '\n', '\r', '_', '@', '.', ',', ':', ';', '(', ')', '{', '}', '[', ']', '+',
        '-', '*', '/', '%', '=', '<', '>', '!', '?', '&', '|', '"', '\\', 'a', 'b', 'c', 'x', 'y',
        'z', 'A', 'B', 'C', '0', '1', '2', '3', '9', '中', 'é',
    ];

    let len = rng.gen_usize(max_len + 1);
    let mut s = String::with_capacity(len);
    for _ in 0..len {
        let ch = CHARS[rng.gen_usize(CHARS.len())];
        s.push(ch);
    }
    s
}

#[test]
fn parse_minimal_file() {
    let src = SourceFile::new_virtual("<mem>", "package a.b\n\nfun main() { val x = 1 }");
    let ast = parse_file(&src).unwrap();
    assert!(ast.package.is_some());
    assert_eq!(ast.imports.len(), 0);
    assert_eq!(ast.items.len(), 1);
}

#[test]
fn parse_import_alias_decl() {
    let src = SourceFile::new_virtual(
        "<mem>",
        "package a\nimport foo.bar.Baz as Qux\nfun main() {}",
    );
    let ast = parse_file(&src).unwrap();
    assert_eq!(ast.imports.len(), 1);

    let import = &ast.imports[0];
    assert_eq!(import.path.len(), 3);
    assert!(!import.has_star);

    let alias = import
        .alias
        .as_ref()
        .expect("alias import 应当记录 alias 名");
    assert_eq!(src.slice(alias.span), "Qux");
}

#[test]
fn parse_type_decls() {
    let src = SourceFile::new_virtual(
        "<mem>",
        "open class A(x: Int) : B(x)\nstruct P(val x: Int)\nenum E { A, B }\nfun main() {}",
    );
    let ast = parse_file(&src).unwrap();
    assert_eq!(ast.items.len(), 4);
}

#[test]
fn parse_annotation_class_decl() {
    let src = SourceFile::new_virtual(
        "<mem>",
        "package a\nannotation class Deprecated(val message: String = \"\")\nclass C {}\n",
    );
    let file = parse_file(&src).unwrap();
    assert_eq!(file.items.len(), 2);

    let ast::Item::Type(t) = &file.items[0] else {
        panic!("期望第一个 item 为类型声明");
    };

    assert_eq!(t.kind, ast::TypeKind::Class);
    assert!(t.modifiers.contains(&ast::Modifier::Annotation));
}

#[test]
fn parse_async_fun_decl() {
    let src = SourceFile::new_virtual("<mem>", "package a\nasync fun f(): Int { return 1 }\n");
    let file = parse_file(&src).unwrap();
    assert_eq!(file.items.len(), 1);

    let ast::Item::Fun(f) = &file.items[0] else {
        panic!("期望顶层第一个 item 为函数声明");
    };

    assert_eq!(src.slice(f.name.span), "f");
    assert!(f.modifiers.contains(&ast::Modifier::Async));
}

#[test]
fn parse_fun_decl_with_annotations() {
    let src = SourceFile::new_virtual(
        "<mem>",
        "package a\n@Unsafe\n@Extern(\"c_name\")\nfun f() {}\n",
    );
    let file = parse_file(&src).unwrap();

    let ast::Item::Fun(f) = &file.items[0] else {
        panic!("期望顶层第一个 item 为函数声明");
    };

    assert_eq!(src.slice(f.name.span), "f");
    assert_eq!(f.annotations.len(), 2);

    assert_eq!(src.slice(f.annotations[0].path[0].span), "Unsafe");
    assert!(f.annotations[0].args.is_empty());

    assert_eq!(src.slice(f.annotations[1].path[0].span), "Extern");
    assert_eq!(f.annotations[1].args.len(), 1);
    assert!(f.annotations[1].args[0].name.is_none());
    assert!(matches!(
        f.annotations[1].args[0].value.kind,
        ast::ExprKind::StringLit
    ));
    assert_eq!(src.slice(f.annotations[1].args[0].value.span), "\"c_name\"");
}

#[test]
fn parse_top_level_val_var() {
    let src = SourceFile::new_virtual(
        "<mem>",
        "package a\nval x: Int = 1\nvar y = x\nfun main() {}",
    );
    let ast = parse_file(&src).unwrap();
    assert_eq!(ast.items.len(), 3);
}

#[test]
fn parse_top_level_typealias() {
    let src = SourceFile::new_virtual("<mem>", "package a\ntypealias Byte = UInt8\n");
    let file = parse_file(&src).unwrap();
    assert_eq!(file.items.len(), 1);

    let ast::Item::TypeAlias(ta) = &file.items[0] else {
        panic!("期望顶层第一个 item 为 typealias 声明");
    };
    assert_eq!(src.slice(ta.name.span), "Byte");
    assert!(ta.modifiers.is_empty());

    let ast::TypeRef::Path(p) = &ta.ty else {
        panic!("期望 typealias RHS 为路径类型引用");
    };
    assert_eq!(src.slice(p.segments[0].span), "UInt8");
}

#[test]
fn parse_call_named_args() {
    let src = SourceFile::new_virtual("<mem>", "package a\nval v = f(x = 1, y = 2)\n");
    let file = parse_file(&src).unwrap();

    let ast::Item::Val(v) = &file.items[0] else {
        panic!("期望顶层第一个 item 为 val 声明");
    };
    let init = v.init.as_ref().expect("val 应当包含 initializer");

    let ast::ExprKind::Call { args, .. } = &init.kind else {
        panic!("期望 initializer 为调用表达式");
    };
    assert_eq!(args.len(), 2);

    let ast::ExprKind::NamedArg {
        name,
        eq_span,
        value,
    } = &args[0].kind
    else {
        panic!("期望第一个实参为命名参数");
    };
    assert_eq!(src.slice(name.span), "x");
    assert_eq!(src.slice(*eq_span), "=");
    assert!(matches!(&value.kind, ast::ExprKind::IntLit));

    let ast::ExprKind::NamedArg {
        name,
        eq_span,
        value,
    } = &args[1].kind
    else {
        panic!("期望第二个实参为命名参数");
    };
    assert_eq!(src.slice(name.span), "y");
    assert_eq!(src.slice(*eq_span), "=");
    assert!(matches!(&value.kind, ast::ExprKind::IntLit));
}

#[test]
fn parse_comptime_syntax_and_splice() {
    let src = SourceFile::new_virtual(
        "<mem>",
        r#"
package a

const fun add(a: Int, b: Int): Int { return a + b }

fun f(value: T) {
    comptime {
        comptime for (field in fieldsOf()) {
            val x = value.[field]
        }

        comptime if (cond) {
            val y = 1
        } else comptime if (cond2) {
            val y = 2
        } else {
            val y = 3
        }
    }
}
"#,
    );

    let file = parse_file(&src).unwrap();
    assert_eq!(file.items.len(), 2);

    let ast::Item::Fun(f) = &file.items[1] else {
        panic!("期望第二个 item 为 fun 声明");
    };
    let ast::FunBody::Block(body) = &f.body else {
        panic!("期望 fun body 为 block");
    };

    assert_eq!(body.stmts.len(), 1);
    let ast::StmtKind::ComptimeBlock { body: cbody, .. } = &body.stmts[0].kind else {
        panic!("期望函数体内第一条语句为 comptime block");
    };

    assert_eq!(cbody.stmts.len(), 2);

    // 1) `comptime for`
    let ast::StmtKind::ComptimeFor(cf) = &cbody.stmts[0].kind else {
        panic!("期望 comptime block 内第一条语句为 comptime for");
    };
    assert_eq!(src.slice(cf.binder.span), "field");
    let ast::ExprKind::Call { .. } = &cf.iter.kind else {
        panic!("期望 comptime for 的 iter 为调用表达式");
    };

    assert_eq!(cf.body.stmts.len(), 1);
    let ast::StmtKind::Val(v) = &cf.body.stmts[0].kind else {
        panic!("期望 comptime for body 内第一条语句为 val 声明");
    };
    let init = v.init.as_ref().expect("val 应当包含 initializer");
    let ast::ExprKind::SpliceField { receiver, field } = &init.kind else {
        panic!("期望 val initializer 为 splice 表达式");
    };
    assert!(matches!(&receiver.kind, ast::ExprKind::Ident(_)));
    assert!(matches!(&field.kind, ast::ExprKind::Ident(_)));

    // 2) `comptime if` + `else comptime if`
    let ast::StmtKind::ComptimeIf(ci) = &cbody.stmts[1].kind else {
        panic!("期望 comptime block 内第二条语句为 comptime if");
    };
    assert!(matches!(&ci.cond.kind, ast::ExprKind::Ident(_)));
    let Some(else_branch) = ci.else_branch.as_deref() else {
        panic!("期望 comptime if 包含 else 分支");
    };
    let ast::ComptimeIfElse::If(nested) = else_branch else {
        panic!("期望 else 分支为 `else comptime if ...`");
    };
    assert!(nested.else_branch.is_some());
}

/// 崩溃防线：确保 parser 对“任意输入”都不会 panic。
///
/// 由于 parser 内部大量逻辑依赖 token cursor 的边界行为，这类测试能尽早发现
/// “越界/空 vec unwrap/无 EOF token”等内部 bug。
#[test]
fn parser_random_inputs_do_not_panic() {
    // 先跑一组固定 corpus，确保常见“坏输入”都不会触发 panic。
    const CORPUS: &[&str] = &[
        "",
        " ",
        "\n\n\n",
        "/*",
        "*/",
        "//",
        "\"",
        "\"\\",
        "\"\n",
        "\"\"\"",
        "package",
        "package a.b\nimport x.y.*\nfun f() {}",
        "fun f( {",
        "open class A(x: Int) : B(x)\nfun main() {",
    ];

    for (idx, &text) in CORPUS.iter().enumerate() {
        let src = SourceFile::new_virtual("<corpus>", text);
        let res = std::panic::catch_unwind(|| {
            let _ = parse_file(&src);
        });
        if res.is_err() {
            panic!("parser panic（corpus#{idx}）: {text:?}");
        }
    }

    // 再跑一轮可复现的伪随机输入。
    let mut rng = XorShift64::new(0xD15E_A5E0_1234_5678);
    for i in 0..1_000usize {
        let text = gen_source(&mut rng, 512);
        let src = SourceFile::new_virtual("<fuzz>", text.clone());
        let res = std::panic::catch_unwind(|| {
            let _ = parse_file(&src);
        });
        if res.is_err() {
            panic!("parser panic（iter={i}）: {text:?}");
        }
    }
}
