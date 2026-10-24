# scoop.url 示例

先完成[运行准备](../../../README.md#运行准备)，以下命令在本目录执行。

| 文件 | 内容 |
| --- | --- |
| [resolve.scoop](resolve.scoop) | 相对 URL、URI/IRI、HTTP request target 和组件构建 |
| [query_and_form.scoop](query_and_form.scoop) | 重复参数、无值/空值、普通 query 与表单的空格规则 |

```sh
scoop run --release resolve.scoop
scoop run --release query_and_form.scoop
```

第一个示例将 `a/b` 作为单个路径段添加，结果包含 `a%2Fb`。第二个示例保留两个 `tag`，把 `flag` 显示为 `<no value>`，而 `empty` 显示为空串；普通 query 编码为 `q=a%20b%2Bc`，表单编码为 `q=a+b%2Bc`，解码恢复 `a b+c`。这些操作只处理内存中的 URL，不发送请求。
