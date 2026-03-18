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
fn parse_type_decls() {
    let src = SourceFile::new_virtual(
        "<mem>",
        "open class A(x: Int) : B(x)\nstruct P(val x: Int)\nenum E { A, B }\nfun main() {}",
    );
    let ast = parse_file(&src).unwrap();
    assert_eq!(ast.items.len(), 4);
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
