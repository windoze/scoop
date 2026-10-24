# scoop.readline 示例

先完成[运行准备](../../../README.md#运行准备)，以下命令在本目录执行。

| 文件 | 内容 |
| --- | --- |
| [repl.scoop](repl.scoop) | 内存历史、历史提示、重复读取、EOF 与 Ctrl+C |
| [multiline.scoop](multiline.scoop) | Vi 编辑、文件名补全、括号校验和多行输入 |

```sh
scoop run --release repl.scoop
scoop run --release multiline.scoop
```

REPL 输入 `:quit` 或按 Ctrl+D 退出，上箭头回看本次运行的历史；Ctrl+C 取消当前输入并继续。多行示例输入 `(` 后回车，再输入 `Scoop)`，括号平衡后返回完整文本并结束。历史只保存在内存，不读写用户历史文件。

也可验证重定向输入；这种模式没有交互按键编辑：

```sh
printf 'hello\n:quit\n' | scoop run --release repl.scoop
printf '(\nScoop)\n' | scoop run --release multiline.scoop
```

前者包含 `You entered: hello`，后者在 `Accepted:` 后输出两行括号内容。
