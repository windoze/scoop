# scoop.idna

独立 IDNA／Punycode Cone，依赖 core、std。默认执行 UTS #46 non-transitional、IDNA2008 检查与 DNS hostname 约束；同步内存计算，没有 DNS、文件、线程或 suspend 入口。使用固定 libidn2／libunistring 后端，不依赖 ICU。

```scoop
import scoop.idna.*

val ascii = Idna.toAscii("bücher.example")
val display = Idna.toUnicode(ascii)
val name = Idna.parse("bücher.example")
val invalid = Idna.toAsciiOrNone("a_b.example")
val encoded = Punycode.encode("bücher")
```

`Idna` 的转换验证整个域名；`labelToAscii`／`labelToUnicode` 拒绝分隔符。`Punycode` 只执行 RFC 3492 原始编码，不代表结果是合法域名。异常区分正常格式失败和后端失败，OrNone 只转换前者。IDNA 数据版本可用 `Idna.unicodeVersion` 查询，后端与补丁见 `backendVersion`。

需要同时保留传输与展示表示时使用 `Idna.parse`，返回的 `IdnaName.ascii`／`unicode` 复用一次验证结果，且不持有需要关闭的 native 状态。

准备同目标 native 资源后可直接运行本 Cone 的 `scoop test`。完整合同见 [IDNA 规范](../../../docs/specs/SCOOP-IDNA-SPEC.md)，构建及许可见 [vendor 说明](../../vendor/libidn2/README.md)。
