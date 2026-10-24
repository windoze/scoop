# scoop.idna 示例

先完成[运行准备](../../../README.md#运行准备)，以下命令在本目录执行。

| 文件 | 内容 |
| --- | --- |
| [domain_names.scoop](domain_names.scoop) | 一次解析保留 ASCII/Unicode 域名、OrNone 处理非法名称 |
| [punycode.scoop](punycode.scoop) | 原始 Punycode 编解码与 IDNA 标签校验的区别 |

```sh
scoop run --release domain_names.scoop
scoop run --release punycode.scoop
```

域名示例输出 `xn--bcher-kva.example`、`bücher.example`、`中国.example` 和 `invalid domain`。原始编码示例的前两行为 `bcher-kva` 和 `bücher`；Punycode 可编码 `a_b`，但它不是合法的 IDNA 域名标签。两例均不进行 DNS 查询。
