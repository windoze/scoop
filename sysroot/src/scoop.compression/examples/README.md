# scoop.compression 示例

先完成[运行准备](../../../README.md#运行准备)，以下命令在本目录执行。

| 文件 | 内容 |
| --- | --- |
| [memory.scoop](memory.scoop) | 默认 zstd 与显式 gzip 压缩/解压、解压输出上限 |
| [file_stream.scoop](file_stream.scoop) | 文件流经 CompressionSink 写 gzip，再经 DecompressionSource 读回 |

```sh
scoop run --release memory.scoop
scoop run --release file_stream.scoop
```

内存示例两次输出 `Hello, compressed Scoop!`。文件示例输出 `A file compressed through streams.`，并在结束时清理临时目录。写入端显式调用 `finish()` 完成 gzip 内容；`close()` 只负责清理，不代替格式终结。适配器默认借用文件，文件由外层 `use` 关闭。
