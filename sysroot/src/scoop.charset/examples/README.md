# scoop.charset 示例

先完成[运行准备](../../../README.md#运行准备)，以下命令在本目录执行。

| 文件 | 内容 |
| --- | --- |
| [encoding.scoop](encoding.scoop) | GBK、Windows-1252、严格 ASCII 失败和显式有损转换 |
| [unicode.scoop](unicode.scoop) | NFC 规范化与扩展字素簇迭代 |

```sh
scoop run --release encoding.scoop
scoop run --release unicode.scoop
```

转码示例输出 `d6d0cec4`、`中文`、`café`，报告严格 ASCII 不能表示 `é`，最后输出 `caf?`。Unicode 示例把分解形式的 José 从 5 个标量规范化为 4 个，再逐项输出 `A`、`👩‍💻` 和组合形式的 `é`。NFC 与字素处理是显式操作，不隐含在编码转换中。
