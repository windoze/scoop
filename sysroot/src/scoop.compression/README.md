# scoop.compression

内存、增量、同步流和异步流使用同一个 Cone，支持 gzip、zlib、raw DEFLATE、Brotli 和 zstd。通用压缩默认 zstd level 3；实现使用固定的 Zstandard 1.5.7、zlib-ng 2.3.3 和 Brotli 1.2.0。

## 依赖与内存操作

```toml
[dependencies]
"scoop:scoop.core" = "0.1.0"
"scoop:scoop.std" = "0.1.0"
"scoop:scoop.compression" = "0.1.0"
```

```scoop
import scoop.compression.*

val original = "hello 中文".toByteArray()
val encoded = compress(original)
val decoded = decompress(encoded)

val gzip = compress(original, CompressionOptions.Gzip(GzipOptions()))
val text = String.fromUtf8(decompress(gzip, DecompressionOptions(CompressionFormat.Gzip)))
```

解压格式由 `DecompressionOptions.format` 明确指定，缺省为 zstd。`DecompressionMode.Automatic` 控制所选格式的连续 frame／member 处理，并不猜测压缩格式；`Single` 用于单个 frame／member 的增量解码。完整内存操作和流适配拒绝未消费的尾随数据。

`maxOutputBytes` 可限制解压后的字节数；zstd 另支持 `maxWindowBytes` 和 `CompressionDictionary`。字典及窗口参数只接受 zstd，不能对其他格式设置。gzip 元数据、DEFLATE 窗口／策略、Brotli quality／mode 和 zstd level／checksum 均有各自配置。

## 同步文件流

文件示例还需要声明 `"scoop:scoop.platform" = "0.1.0"`，并使用正常文件 I/O 接口：

```scoop
import scoop.compression.*
import scoop.platform.fs.*
import scoop.platform.io.*

fun gzipFile(input: Path, output: Path) {
    val writing = OpenOptions(read = false, write = true, create = true, truncate = true)
    File.open(input).use { source ->
        File.open(output, writing).use { destination ->
            CompressionSink(destination, CompressionOptions.Gzip(GzipOptions())).use { sink ->
                source.copyTo(sink, 8192L)
                sink.finish()
            }
        }
    }
}
```

四个适配器为 `CompressionSource`、`CompressionSink`、`DecompressionSource` 和 `DecompressionSink`。默认借用上下游，示例通过外层 `use` 关闭文件；需要把端点的关闭责任交给适配器时，使用相应的 `takeOwnership`。`compressTo`／`decompressTo` 可以完成一次完整复制并返回实际输入／输出计数。

## 异步文件与网络流

异步示例再声明 `"scoop:scoop.asyncio" = "0.1.0"`。`scoop.compression.asyncio` 是同一个压缩 Cone 内的 package：

```scoop
import scoop.asyncio.*
import scoop.compression.*
import scoop.compression.asyncio.*
import scoop.platform.fs.*

suspend fun gzipFileAsync(loop: EventLoop, input: Path, output: Path) {
    val writing = OpenOptions(read = false, write = true, create = true, truncate = true)
    loop.openFile(input).useAsync suspend { source ->
        loop.openFile(output, writing).useAsync suspend { destination ->
            AsyncCompressionSink(destination, CompressionOptions.Gzip(GzipOptions())).useAsync suspend { sink ->
                source.copyTo(sink, 8192L)
                sink.finish()
            }
        }
    }
}
```

异步 Source／Sink 与同步接口对应，使用相同的格式配置和增量 codec。读写等待上游或下游时真正挂起，并以有限缓冲保持背压；CPU 编码／解码仍是本地计算。取消、并发关闭和失败清理会等待真实的 I/O 与 native borrow 完成，借用的端点保留自己的关闭责任。

## finish、flush 与 close

- `flush` 推进当前输出和格式内同步点，之后仍可继续输入。
- `finish` 在编码时写出格式结束信息，在解码时确认输入完整，并排空尚未交付的输出；正常完成 Sink 时显式调用，或使用 `compressTo`／`decompressTo` 完成复制。
- `close` 释放 codec 和已接管的端点，不替未完成的流补写结束标记。取消路径使用 `close` 收尾，不在 `finally` 中继续读取来源或强行 `finish`。

Source 读到 EOF 时完成相应的格式终结与检查；提前关闭 Source 只结束消费，不能据此声称整个压缩内容已经验证。短读写和单字节缓冲均受支持，`totals` 记录实际接受和交付的字节数，错误保留格式、阶段与进度。

需要自行管理输入／输出缓冲时，使用 `CompressionEncoder`／`CompressionDecoder` 的增量接口，并按 `NeedInput`、`NeedOutput`、`Finished` 推进。WebSocket 使用 `DeflateMessageEncoder`／`DeflateMessageDecoder` 表达 RFC 7692 消息边界、flush、窗口与 context takeover；协议库通过此公共接口组合压缩消息。

完整合同见 [压缩规范](../../../docs/specs/SCOOP-COMPRESSION-SPEC.md)。可运行的内存、真实文件、TCP、取消和独立格式互操作组合见 `tests/fixtures/m54-compression`、`m54-compression-streams`、`m54-compression-async` 和 `m54-deflate-messages`；实际三目标状态见 [M54 进度](../../../docs/milestone54/PROGRESS.md)。
