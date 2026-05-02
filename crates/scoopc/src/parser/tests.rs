use super::*;
use crate::syntax::token::Symbol;
use crate::syntax::token::TokenKind;

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
        '-', '*', '/', '%', '=', '<', '>', '!', '?', '&', '|', '"', '\'', '\\', 'a', 'b', 'c', 'x',
        'y', 'z', 'A', 'B', 'C', '0', '1', '2', '3', '9', '中', 'é',
    ];

    let len = rng.gen_usize(max_len + 1);
    let mut s = String::with_capacity(len);
    for _ in 0..len {
        let ch = CHARS[rng.gen_usize(CHARS.len())];
        s.push(ch);
    }
    s
}

fn top_level_val_init(file: &ast::File, idx: usize) -> &ast::Expr {
    let ast::Item::Val(val) = &file.items[idx] else {
        panic!("期望第 {idx} 个顶层 item 为 val 声明");
    };
    val.init.as_ref().expect("val 应当包含 initializer")
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
fn parse_target_annotation_with_enum_values() {
    let src = SourceFile::new_virtual(
        "<mem>",
        "package a\n@Target(AnnotationTarget.Field)\nannotation class Column\n",
    );
    let file = parse_file(&src).unwrap();

    let ast::Item::Type(t) = &file.items[0] else {
        panic!("期望顶层第一个 item 为类型声明");
    };
    assert!(t.modifiers.contains(&ast::Modifier::Annotation));
    assert_eq!(src.slice(t.name.span), "Column");

    assert_eq!(t.annotations.len(), 1);
    let ann = &t.annotations[0];
    assert_eq!(src.slice(ann.path[0].span), "Target");
    assert_eq!(ann.args.len(), 1);

    let v = &ann.args[0].value;
    assert!(matches!(v.kind, ast::ExprKind::MemberAccess { .. }));
}

#[test]
fn parse_unsafe_block_expr() {
    let src = SourceFile::new_virtual("<mem>", "package a\nfun f() { @Unsafe do { 1 } }\n");
    let file = parse_file(&src).unwrap();

    let ast::Item::Fun(f) = &file.items[0] else {
        panic!("期望顶层第一个 item 为函数声明");
    };

    let ast::FunBody::Block(b) = &f.body else {
        panic!("期望函数体为 block");
    };

    let ast::StmtKind::Expr(e) = &b.stmts[0].kind else {
        panic!("期望第一条语句为表达式语句");
    };

    let ast::ExprKind::UnsafeBlock {
        at_unsafe_span,
        body,
    } = &e.kind
    else {
        panic!("期望表达式为 unsafe block");
    };

    assert_eq!(src.slice(*at_unsafe_span), "@Unsafe");
    assert_eq!(body.stmts.len(), 1);
    assert!(matches!(body.stmts[0].kind, ast::StmtKind::Expr(_)));
}

#[test]
fn parse_safe_annotated_closure_expr() {
    let src = SourceFile::new_virtual("<mem>", "package a\nval f: () -> Int = @Safe { 1 }\n");
    let file = parse_file(&src).unwrap();

    let ast::Item::Val(v) = &file.items[0] else {
        panic!("期望顶层第一个 item 为 val");
    };
    let init = v.init.as_ref().expect("val 应有 initializer");

    let ast::ExprKind::Lambda(lam) = &init.kind else {
        panic!("期望 initializer 为 annotated lambda");
    };

    let at_safe_span = lam.at_safe_span.expect("@Safe closure 应记录注解 span");
    assert_eq!(src.slice(at_safe_span), "@Safe");
    assert!(lam.params.is_empty());
    assert!(matches!(lam.body.kind, ast::ExprKind::IntLit));
}

#[test]
fn parse_unsafe_block_requires_do() {
    let src = SourceFile::new_virtual("<mem>", "package a\nfun f() { @Unsafe { 1 } }\n");
    let err = parse_file(&src).expect_err("裸 `@Unsafe { ... }` 应报错");
    assert!(matches!(err, ParseError::UnsafeBlockRequiresDo { .. }));
}

#[test]
fn parse_char_literal_expr_and_when_pattern() {
    let src = SourceFile::new_virtual(
        "<mem>",
        "package a\nval plain = 'A'\nval choice = when (c) { 'x' -> 1 else -> 2 }\n",
    );
    let file = parse_file(&src).unwrap();
    assert_eq!(file.items.len(), 2);

    let ast::Item::Val(plain) = &file.items[0] else {
        panic!("期望第一个 item 为 val");
    };
    let plain_init = plain.init.as_ref().expect("plain 应有 initializer");
    assert!(matches!(plain_init.kind, ast::ExprKind::CharLit));
    assert_eq!(src.slice(plain_init.span), "'A'");

    let ast::Item::Val(choice) = &file.items[1] else {
        panic!("期望第二个 item 为 val");
    };
    let choice_init = choice.init.as_ref().expect("choice 应有 initializer");
    let ast::ExprKind::When { arms, .. } = &choice_init.kind else {
        panic!("期望 choice initializer 为 when 表达式");
    };
    assert!(matches!(arms[0].pat, ast::WhenPat::CharLit { .. }));
    assert_eq!(src.slice(arms[0].pat.span()), "'x'");
}

#[test]
fn parse_when_variant_payload_or_pattern() {
    let src = SourceFile::new_virtual(
        "<mem>",
        "package a\nval choice = when (x) { Hit(0) | Miss() -> 1 else -> 2 }\n",
    );
    let file = parse_file(&src).unwrap();

    let ast::Item::Val(choice) = &file.items[0] else {
        panic!("期望第一个 item 为 val");
    };
    let choice_init = choice.init.as_ref().expect("choice 应有 initializer");
    let ast::ExprKind::When { arms, .. } = &choice_init.kind else {
        panic!("期望 choice initializer 为 when 表达式");
    };
    let ast::WhenPat::Or { pats, .. } = &arms[0].pat else {
        panic!("期望首个 when pattern 为 or-pattern");
    };
    assert_eq!(pats.len(), 2);

    let ast::WhenPat::Variant { path, args, .. } = &pats[0] else {
        panic!("期望第一个 or 分支为 variant pattern");
    };
    assert_eq!(path.segments.len(), 1);
    assert_eq!(src.slice(path.segments[0].span), "Hit");
    assert_eq!(args.len(), 1);
    assert!(matches!(args[0], ast::WhenPat::IntLit { .. }));

    let ast::WhenPat::Variant { path, args, .. } = &pats[1] else {
        panic!("期望第二个 or 分支为 variant pattern");
    };
    assert_eq!(path.segments.len(), 1);
    assert_eq!(src.slice(path.segments[0].span), "Miss");
    assert!(args.is_empty());
}

#[test]
fn parse_when_bare_variant_or_pattern_keeps_zero_arity_variants() {
    let src = SourceFile::new_virtual(
        "<mem>",
        "package a\nval choice = when (x) { Hit | Miss -> 1 else -> 2 }\n",
    );
    let file = parse_file(&src).unwrap();

    let ast::Item::Val(choice) = &file.items[0] else {
        panic!("期望第一个 item 为 val");
    };
    let choice_init = choice.init.as_ref().expect("choice 应有 initializer");
    let ast::ExprKind::When { arms, .. } = &choice_init.kind else {
        panic!("期望 choice initializer 为 when 表达式");
    };
    let ast::WhenPat::Or { pats, .. } = &arms[0].pat else {
        panic!("期望首个 when pattern 为 or-pattern");
    };
    assert_eq!(pats.len(), 2);

    let ast::WhenPat::Variant { path, args, .. } = &pats[0] else {
        panic!("期望第一个 or 分支为 bare variant pattern");
    };
    assert_eq!(path.segments.len(), 1);
    assert_eq!(src.slice(path.segments[0].span), "Hit");
    assert!(
        args.is_empty(),
        "bare variant pattern 不应被 parser 扩成 payload wildcard"
    );

    let ast::WhenPat::Variant { path, args, .. } = &pats[1] else {
        panic!("期望第二个 or 分支为 bare variant pattern");
    };
    assert_eq!(path.segments.len(), 1);
    assert_eq!(src.slice(path.segments[0].span), "Miss");
    assert!(
        args.is_empty(),
        "bare variant pattern 不应被 parser 扩成 payload wildcard"
    );
}

#[test]
fn parse_when_qualified_variant_patterns() {
    let src = SourceFile::new_virtual(
        "<mem>",
        "package a\nval choice = when (x) { TaskStep.Ready(1) | TaskStep.Pending -> 1 else -> 2 }\n",
    );
    let file = parse_file(&src).unwrap();

    let ast::Item::Val(choice) = &file.items[0] else {
        panic!("期望第一个 item 为 val");
    };
    let choice_init = choice.init.as_ref().expect("choice 应有 initializer");
    let ast::ExprKind::When { arms, .. } = &choice_init.kind else {
        panic!("期望 choice initializer 为 when 表达式");
    };
    let ast::WhenPat::Or { pats, .. } = &arms[0].pat else {
        panic!("期望首个 when pattern 为 or-pattern");
    };
    assert_eq!(pats.len(), 2);

    let ast::WhenPat::Variant { path, args, .. } = &pats[0] else {
        panic!("期望第一个 or 分支为 qualified variant pattern");
    };
    assert_eq!(path.segments.len(), 2);
    assert_eq!(src.slice(path.segments[0].span), "TaskStep");
    assert_eq!(src.slice(path.segments[1].span), "Ready");
    assert_eq!(args.len(), 1);
    assert!(matches!(args[0], ast::WhenPat::IntLit { .. }));

    let ast::WhenPat::Variant { path, args, .. } = &pats[1] else {
        panic!("期望第二个 or 分支为 qualified bare variant pattern");
    };
    assert_eq!(path.segments.len(), 2);
    assert_eq!(src.slice(path.segments[0].span), "TaskStep");
    assert_eq!(src.slice(path.segments[1].span), "Pending");
    assert!(args.is_empty());
}

#[test]
fn do_block_basic() {
    let src = SourceFile::new_virtual("<mem>", "fun f() { val x = do { 1 }; return x }");
    let file = parse_file(&src).unwrap();

    let ast::Item::Fun(f) = &file.items[0] else {
        panic!("期望顶层第一个 item 为函数声明");
    };
    let ast::FunBody::Block(b) = &f.body else {
        panic!("期望函数体为 block");
    };
    let ast::StmtKind::Val(val) = &b.stmts[0].kind else {
        panic!("期望第一条语句为 val 声明");
    };
    let init = val.init.as_ref().expect("val 应有 initializer");
    let ast::ExprKind::DoBlock { body, .. } = &init.kind else {
        panic!("期望 initializer 为 DoBlock，实际为 {:?}", init.kind);
    };
    assert_eq!(body.stmts.len(), 1);
    assert!(matches!(body.stmts[0].kind, ast::StmtKind::Expr(_)));
}

#[test]
fn bare_brace_is_lambda_not_do_block() {
    let src = SourceFile::new_virtual("<mem>", "val f = { 1 }");
    let file = parse_file(&src).unwrap();

    let ast::Item::Val(v) = &file.items[0] else {
        panic!("期望顶层第一个 item 为 val");
    };
    let init = v.init.as_ref().expect("val 应有 initializer");
    assert!(
        matches!(init.kind, ast::ExprKind::Lambda(_)),
        "期望裸 {{}} 被解析为 lambda，实际为 {:?}",
        init.kind
    );
}

#[test]
fn safe_do_block() {
    let src = SourceFile::new_virtual("<mem>", "fun f() { @Safe do { 1 } }");
    let file = parse_file(&src).unwrap();

    let ast::Item::Fun(f) = &file.items[0] else {
        panic!("期望函数声明");
    };
    let ast::FunBody::Block(b) = &f.body else {
        panic!("期望函数体为 block");
    };
    let ast::StmtKind::Expr(e) = &b.stmts[0].kind else {
        panic!("期望表达式语句");
    };
    assert!(
        matches!(e.kind, ast::ExprKind::SafeBlock { .. }),
        "期望 @Safe do {{}} 被解析为 SafeBlock，实际为 {:?}",
        e.kind
    );
}

#[test]
fn unsafe_do_block() {
    let src = SourceFile::new_virtual("<mem>", "fun f() { @Unsafe do { 1 } }");
    let file = parse_file(&src).unwrap();

    let ast::Item::Fun(f) = &file.items[0] else {
        panic!("期望函数声明");
    };
    let ast::FunBody::Block(b) = &f.body else {
        panic!("期望函数体为 block");
    };
    let ast::StmtKind::Expr(e) = &b.stmts[0].kind else {
        panic!("期望表达式语句");
    };
    assert!(
        matches!(e.kind, ast::ExprKind::UnsafeBlock { .. }),
        "期望 @Unsafe do {{}} 被解析为 UnsafeBlock，实际为 {:?}",
        e.kind
    );
}

#[test]
fn annotated_local_val_decl_basic() {
    let src = SourceFile::new_virtual(
        "<mem>",
        "fun f() { @Suppress(\"deprecated\") val x = oldAdd() }",
    );
    let file = parse_file(&src).unwrap();

    let ast::Item::Fun(f) = &file.items[0] else {
        panic!("期望函数声明");
    };
    let ast::FunBody::Block(body) = &f.body else {
        panic!("期望函数体为 block");
    };
    let ast::StmtKind::Val(val) = &body.stmts[0].kind else {
        panic!("期望第一条语句为局部 val 声明");
    };
    assert_eq!(val.annotations.len(), 1);
    assert_eq!(src.slice(val.annotations[0].path[0].span), "Suppress");
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
    assert!(ta.type_params.is_empty());

    let ast::TypeRef::Path(p) = &ta.ty else {
        panic!("期望 typealias RHS 为路径类型引用");
    };
    assert_eq!(src.slice(p.segments[0].span), "UInt8");
}

#[test]
fn parse_top_level_generic_typealias() {
    let src = SourceFile::new_virtual("<mem>", "package a\ntypealias Handler<T> = (T) -> Unit\n");
    let file = parse_file(&src).unwrap();
    assert_eq!(file.items.len(), 1);

    let ast::Item::TypeAlias(ta) = &file.items[0] else {
        panic!("期望顶层第一个 item 为 typealias 声明");
    };
    assert_eq!(src.slice(ta.name.span), "Handler");
    assert_eq!(ta.type_params.len(), 1);
    assert_eq!(src.slice(ta.type_params[0].name.span), "T");

    let ast::TypeRef::Function(fun) = &ta.ty else {
        panic!("期望 typealias RHS 为函数类型引用");
    };
    assert_eq!(fun.params.len(), 1);
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
fn parse_resume_member_call_as_plain_call_shape() {
    let src = SourceFile::new_virtual("<mem>", "package a\nval resumed = k.resume(x)\n");
    let file = parse_file(&src).unwrap();

    let init = top_level_val_init(&file, 0);
    let ast::ExprKind::Call { callee, args } = &init.kind else {
        panic!("期望 initializer 为调用表达式");
    };
    assert_eq!(args.len(), 1, "`k.resume(x)` 不应在 AST 中改写 arity");
    assert!(matches!(args[0].kind, ast::ExprKind::Ident(_)));
    assert_eq!(src.slice(args[0].span), "x");

    let ast::ExprKind::MemberAccess { receiver, member } = &callee.kind else {
        panic!("期望调用 callee 为普通 member access");
    };
    assert!(matches!(receiver.kind, ast::ExprKind::Ident(_)));
    assert_eq!(src.slice(receiver.span), "k");
    assert_eq!(src.slice(member.span), "resume");
}

#[test]
fn parse_zero_arg_resume_member_call_without_unit_desugar() {
    let src = SourceFile::new_virtual("<mem>", "package a\nval resumed = k.resume()\n");
    let file = parse_file(&src).unwrap();

    let init = top_level_val_init(&file, 0);
    let ast::ExprKind::Call { callee, args } = &init.kind else {
        panic!("期望 initializer 为调用表达式");
    };
    assert!(
        args.is_empty(),
        "`k.resume()` 不应在 AST 中自动补 `UnitLit`"
    );

    let ast::ExprKind::MemberAccess { receiver, member } = &callee.kind else {
        panic!("期望调用 callee 为普通 member access");
    };
    assert!(matches!(receiver.kind, ast::ExprKind::Ident(_)));
    assert_eq!(src.slice(receiver.span), "k");
    assert_eq!(src.slice(member.span), "resume");
}

#[test]
fn parse_zero_arg_and_explicit_unit_calls_as_distinct_shapes() {
    let src = SourceFile::new_virtual("<mem>", "package a\nval zero = f()\nval explicit = f(())\n");
    let file = parse_file(&src).unwrap();

    let zero = top_level_val_init(&file, 0);
    let ast::ExprKind::Call { callee, args } = &zero.kind else {
        panic!("期望 `f()` initializer 为调用表达式");
    };
    assert!(matches!(callee.kind, ast::ExprKind::Ident(_)));
    assert_eq!(src.slice(callee.span), "f");
    assert!(args.is_empty(), "`f()` 必须保持零参数调用形状");

    let explicit = top_level_val_init(&file, 1);
    let ast::ExprKind::Call { callee, args } = &explicit.kind else {
        panic!("期望 `f(())` initializer 为调用表达式");
    };
    assert!(matches!(callee.kind, ast::ExprKind::Ident(_)));
    assert_eq!(src.slice(callee.span), "f");
    assert_eq!(args.len(), 1, "`f(())` 必须保留显式单参数调用形状");
    assert!(matches!(args[0].kind, ast::ExprKind::UnitLit));
    assert_eq!(src.slice(args[0].span), "()");
}

#[test]
fn parse_prefixed_int_literals_as_single_tokens() {
    let src = SourceFile::new_virtual("<mem>", "package a\nval hex = 0xFF\nval bin = 0b1010\n");
    let file = parse_file(&src).unwrap();

    let ast::Item::Val(hex) = &file.items[0] else {
        panic!("期望第一个 item 为 val 声明");
    };
    let hex_init = hex.init.as_ref().expect("hex initializer 应存在");
    assert!(matches!(hex_init.kind, ast::ExprKind::IntLit));
    assert_eq!(src.slice(hex_init.span), "0xFF");

    let ast::Item::Val(bin) = &file.items[1] else {
        panic!("期望第二个 item 为 val 声明");
    };
    let bin_init = bin.init.as_ref().expect("bin initializer 应存在");
    assert!(matches!(bin_init.kind, ast::ExprKind::IntLit));
    assert_eq!(src.slice(bin_init.span), "0b1010");
}

#[test]
fn parse_float_literals_and_int_member_call() {
    let src = SourceFile::new_virtual(
        "<mem>",
        "package a\nval plain = 2.75\nval sci = 1.5e3\nval f32 = 0.5f\nval call = 1.toString()\n",
    );
    let file = parse_file(&src).unwrap();

    let ast::Item::Val(plain) = &file.items[0] else {
        panic!("期望第一个 item 为 val 声明");
    };
    let plain_init = plain.init.as_ref().expect("plain initializer 应存在");
    assert!(matches!(plain_init.kind, ast::ExprKind::FloatLit));
    assert_eq!(src.slice(plain_init.span), "2.75");

    let ast::Item::Val(sci) = &file.items[1] else {
        panic!("期望第二个 item 为 val 声明");
    };
    let sci_init = sci.init.as_ref().expect("sci initializer 应存在");
    assert!(matches!(sci_init.kind, ast::ExprKind::FloatLit));
    assert_eq!(src.slice(sci_init.span), "1.5e3");

    let ast::Item::Val(f32_) = &file.items[2] else {
        panic!("期望第三个 item 为 val 声明");
    };
    let f32_init = f32_.init.as_ref().expect("f32 initializer 应存在");
    assert!(matches!(f32_init.kind, ast::ExprKind::FloatLit));
    assert_eq!(src.slice(f32_init.span), "0.5f");

    let ast::Item::Val(call) = &file.items[3] else {
        panic!("期望第四个 item 为 val 声明");
    };
    let call_init = call.init.as_ref().expect("call initializer 应存在");
    let ast::ExprKind::Call { callee, .. } = &call_init.kind else {
        panic!("期望 `1.toString()` 解析为调用表达式");
    };
    let ast::ExprKind::MemberAccess { receiver, .. } = &callee.kind else {
        panic!("期望调用的 callee 为 member access");
    };
    assert!(matches!(receiver.kind, ast::ExprKind::IntLit));
    assert_eq!(src.slice(receiver.span), "1");
}

#[test]
fn when_float_pattern_reports_single_parse_error() {
    let src = SourceFile::new_virtual(
        "<mem>",
        r#"
fun main() {
    val x: Float64 = 1.5
    val y = when (x) {
        1.5 -> 1
        else -> 2
    }
    println(y)
}
"#,
    );

    let err = parse_file(&src).expect_err("Float when pattern 应报 parser 错误");
    match err {
        ParseError::Expected {
            expected,
            found: TokenKind::FloatLiteral,
            ..
        } => {
            assert!(expected.contains("Float 字面量"));
        }
        other => panic!("期望单个 FloatLiteral parser 错误，实际为: {other:?}"),
    }
}

#[test]
fn parse_top_level_val_destructuring_with_type_annotation() {
    let src = SourceFile::new_virtual(
        "<mem>",
        r#"
package a
import scoop.core.*

struct Point(val x: Int, val y: Int)

enum MaybeInt {
    Some(val value: Int),
    None,
}

val (a, b): (Int, Int) = (1, 2)
val Point { x, y }: Point = Point { x: 3, y: 4 }
val Some(total): MaybeInt = Some(5)
"#,
    );

    let file = parse_file(&src).unwrap();

    let ast::Item::Val(tuple_decl) = &file.items[2] else {
        panic!("期望第三个 item 为 tuple destructuring 顶层 val");
    };
    assert!(matches!(tuple_decl.binding, ast::ValBinding::Pattern(_)));
    assert!(tuple_decl.ty.is_some());

    let ast::Item::Val(struct_decl) = &file.items[3] else {
        panic!("期望第四个 item 为 struct destructuring 顶层 val");
    };
    assert!(matches!(struct_decl.binding, ast::ValBinding::Pattern(_)));
    assert!(struct_decl.ty.is_some());

    let ast::Item::Val(variant_decl) = &file.items[4] else {
        panic!("期望第五个 item 为 variant destructuring 顶层 val");
    };
    assert!(matches!(variant_decl.binding, ast::ValBinding::Pattern(_)));
    assert!(variant_decl.ty.is_some());
}

#[test]
fn top_level_var_destructuring_is_rejected() {
    let src = SourceFile::new_virtual("<mem>", "package a\nvar (a, b): (Int, Int) = (1, 2)\n");
    let err = parse_file(&src).expect_err("顶层 `var` destructuring 应报错");
    match err {
        ParseError::Expected {
            expected,
            found: TokenKind::Symbol(Symbol::LParen),
            ..
        } => {
            assert!(expected.contains("变量名"));
        }
        other => panic!("期望顶层 `var` destructuring 产生变量名语法错误，实际为: {other:?}"),
    }
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
