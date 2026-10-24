# scoop.terminal 示例

先完成[运行准备](../../../README.md#运行准备)，以下命令在本目录执行。

| 文件 | 内容 |
| --- | --- |
| [styles.scoop](styles.scoop) | ContentStyle、颜色、粗体、命令输出与样式恢复 |
| [parse_input.scoop](parse_input.scoop) | 不依赖 TTY 的输入解析、方向键和 Escape 等待期限 |

```sh
scoop run --release styles.scoop
scoop run --release parse_input.scoop
```

样式示例在 ANSI 终端中显示彩色标题，末行恢复普通文本；重定向输出时仍可能包含控制序列。解析示例只处理内存中的字节，依次输出 `character=a`、`up`、`escape`；显式推进时间使单独的 Escape 与未完成转义序列区分开。
