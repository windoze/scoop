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

第一例依次输出 `point=(3, 4)`、`point missing`、`Ada`、`guest`、`3`。第二例先问候快照中的 Ada 和 Lin，再输出修改后的 Grace，展示快照与可变列表的区别；`Scoop 中文` 有 8 个 Unicode 标量、12 个 UTF-8 字节。
