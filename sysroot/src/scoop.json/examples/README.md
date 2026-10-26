# scoop.json 示例

先完成[运行准备](../../../README.md#运行准备)，以下命令在本目录执行。

| 文件 | 内容 |
| --- | --- |
| [typed.scoop](typed.scoop) | companion 生成的编码/解码能力、数组和可选字段 |
| [value_tree.scoop](value_tree.scoop) | 不可变 JSON 值树、缺失与 null、数字 token 和 builder |

```sh
scoop run --release typed.scoop
scoop run --release value_tree.scoop
```

类型示例的完整说明见 `scoop example json/typed`（[正文](../../../share/scoop/examples/topics/json/typed.md)）；单文件依赖组织见 `scoop example start/single-file`。

值树示例先输出 `Scoop`、`3` 和两个 `true`，再输出包含原对象与 `[null]` 的新对象；缺失字段为 None，显式 JSON null 则是包含 Null 值的 Some。
