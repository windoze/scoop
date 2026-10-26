# scoop.core 示例

先完成[运行准备](../../../README.md#运行准备)，以下命令在本目录执行。这些程序只使用 core API；单文件入口仍按默认规则依赖 core 和 std。

| 文件 | 内容 |
| --- | --- |
| [values.scoop](values.scoop) | struct、带载荷 enum、嵌套模式匹配、Option、安全访问和默认值 |
| [collections.scoop](collections.scoop) | ArrayList、数组快照、Unicode 标量/字节长度、StringBuilder |

```sh
scoop run --release values.scoop
scoop run --release collections.scoop
```

值类型概览的完整说明见 `scoop example language/overview`（[正文](../../../share/scoop/examples/topics/language/overview.md)）。聚焦用法可查询 `language/option`、`language/struct`、`collections/list` 和 `text/strings`。

collections.scoop 先问候快照中的 Ada 和 Lin，再输出修改后的 Grace；`Scoop 中文` 有 8 个 Unicode 标量、12 个 UTF-8 字节。
