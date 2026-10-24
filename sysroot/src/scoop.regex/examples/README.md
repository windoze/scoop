# scoop.regex 示例

先完成[运行准备](../../../README.md#运行准备)，以下命令在本目录执行。

| 文件 | 内容 |
| --- | --- |
| [captures.scoop](captures.scoop) | 命名捕获、惰性搜索与完整匹配 |
| [replace_and_split.scoop](replace_and_split.scoop) | 模板替换、字面替换和分割 |

```sh
scoop run --release captures.scoop
scoop run --release replace_and_split.scoop
```

捕获示例输出 `name: Scoop`、`target: native`、`true`、`false`；日期模式只检查形状，不验证日历合法性。替换示例先输出 `width: 80 height: 24`，再输出字面值 `$1 $1`，最后逐行输出三个颜色。模板引用仅由 `replaceTemplate` 解释，普通 `replace` 的字符串是字面文本；所有 Regex 都在 `use` 中释放。
