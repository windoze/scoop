# scoop.std 示例

先完成[运行准备](../../../README.md#运行准备)，以下命令在本目录执行。单文件默认依赖 std，具体名称仍需显式 import。

| 文件 | 内容 |
| --- | --- |
| [collections.scoop](collections.scoop) | 筛选、排序、连接文本和保持插入顺序的词频表 |
| [text_and_bytes.scoop](text_and_bytes.scoop) | trim、Base64、十六进制、大端整数写入与可失败的数字解析 |
| [dates.scoop](dates.scoop) | 月末日期调整、闰年、日期差和 Duration 运算 |

```sh
scoop run --release collections.scoop
scoop run --release text_and_bytes.scoop
scoop run --release dates.scoop
```

集合示例输出 `6, 8, 9`、`red=2`、`blue=1`。字节示例输出 `U2Nvb3A=`、`Scoop`、`02014f4b`、`123`、`0`。日期示例从 2024-01-31 加一个月得到 2024-02-29，随后一天为 2024-03-01；最后的时长为 150 秒。

按问题阅读可查询 `scoop example collections/map`（[词频正文](../../../share/scoop/examples/topics/collections/map.md)）、`text/strings` 和 `language/errors`；这些聚焦程序与此处混合示例分别维护。
