# scoop.idna

独立 IDNA／Punycode Cone，依赖 core、std。执行 UTS #46 non-transitional、STD3、连字符、Bidi、ContextJ 与 DNS 长度检查；同步内存计算，没有 DNS、文件、线程或 suspend 入口。使用固定 Rust idna 1.1.0 和 ICU4X 2.3.0 后端，Unicode 数据为 17.0.0；不依赖 ICU4C 或 C++。

```scoop
import scoop.idna.*

val ascii = Idna.toAscii("bücher.example")
val display = Idna.toUnicode(ascii)
val name = Idna.parse("bücher.example")
val invalid = Idna.toAsciiOrNone("a_b.example")
val encoded = Punycode.encode("bücher")
```

`Idna` 的转换验证整个域名；`labelToAscii`／`labelToUnicode` 拒绝分隔符。`Punycode` 只执行 RFC 3492 原始编码，不代表结果是合法域名。异常区分正常格式失败和后端失败，OrNone 只转换前者。IDNA 数据版本可用 `Idna.unicodeVersion` 查询，后端版本见 `backendVersion`。

需要同时保留传输与展示表示时使用 `Idna.parse`，返回的 `IdnaName.ascii`／`unicode` 复用一次验证结果，且不持有需要关闭的 native 状态。

准备同目标 native 资源后可直接运行本 Cone 的 `scoop test`。完整合同见 [IDNA 规范](../../../docs/specs/SCOOP-IDNA-SPEC.md)，构建及许可见 [vendor 说明](../../vendor/text_backend/README.md)。

M61 接受 UTS #46 允许的符号域名（如 `☃.example`）和 ContextO 情形（如 `a·b.cat`），不声明完整严格 IDNA2008 校验。上游不能细分的有效性错误统一为 InvalidLabel，code／labelIndex 没有实际值时为 None。

原始 Punycode 保留空串、ASCII 末尾分隔符、内嵌 NUL 与标点；它不提供域名有效性保证。Rust 内部不可恢复 OOM／panic 会终止进程，不保证转成 Backend 异常；普通格式失败始终通过返回状态处理。
