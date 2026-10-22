# Unicode conformance data

固定 Unicode 17.0.0 官方测试数据，用于普通 scoop.test 测试 Cone
`sysroot/native/text_backend/tests`，不作为运行时数据读取。

- [IdnaTestV2.txt](https://www.unicode.org/Public/17.0.0/idna/IdnaTestV2.txt)：non-transitional、STD3、CheckHyphens、Bidi、ContextJ。Scoop 允许末尾根点；只有符合该长度规则的尾部空标签调整 A4_2。独立根点按公开合同处理。无法表示成 Scoop String 的 surrogate 输入按上游说明跳过，非法 UTF-8 另由 C ABI 用例覆盖。
- [NormalizationTest.txt](https://www.unicode.org/Public/17.0.0/ucd/NormalizationTest.txt)：每行检查全部五个 NFC 不变量。
- 原版权与许可保留在文件头及本目录 `LICENSE` 中。

执行时设置 `SCOOP_SYSROOT` 指向已经准备的 sysroot；测试通过公开 Scoop API 消费这些文件。
