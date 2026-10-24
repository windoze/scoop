# Sysroot 示例

每个正式库 Cone 的 `examples/` 都包含可运行示例和中文说明。示例使用公开 API；单文件头部的 `//! dep:` 声明额外依赖，core 和 std 由单文件入口默认提供。父库默认只编译 `src/`，这些示例不会进入库的生产源码。

## 运行准备

使用已安装的 Scoop 时，直接使用配套的 `scoop` 和 sysroot。在本仓库中，先按[准备脚本说明](../scripts/README.md)构建配套工具并准备开发 sysroot，再从仓库根目录执行：

```sh
export PATH="$PWD/target/release:$PATH"
export SCOOP_SYSROOT="$PWD/sysroot"
mkdir -p "$PWD/target/examples/scratch"
export TMPDIR="$PWD/target/examples/scratch"
```

进入所选 Cone 的 `examples/` 后，按照其 README 运行，例如：

```sh
cd sysroot/src/scoop.json/examples
scoop run --release typed.scoop
```

所有示例使用 release。单文件消费预编译库，不会自动编译相邻的父 Cone；库源码更新后需重新准备 sysroot。单文件示例的编译输出默认放在当前目录的 `target/`，也可用 `--target-dir` 指定位置。`scoop.test` 的示例是独立测试 Cone，使用 `scoop test --release`。

## 示例索引

| Cone | 示例主题 | 运行条件 |
| --- | --- | --- |
| [scoop.core](src/scoop.core/examples/README.md) | 值类型、模式匹配、Option、数组与字符串 | 独立运行 |
| [scoop.std](src/scoop.std/examples/README.md) | 集合、文本与字节、日期与时长 | 独立运行 |
| [scoop.cli](src/scoop.cli/examples/README.md) | 位置参数、选项、帮助、枚举和重复选项 | 按说明传参 |
| [scoop.test](src/scoop.test/examples/README.md) | 断言、异常、测试前后置钩子 | `scoop test` |
| [scoop.platform](src/scoop.platform/examples/README.md) | 临时文件、目录、线程与原子计数 | 临时文件自动清理 |
| [scoop.asyncio](src/scoop.asyncio/examples/README.md) | 并发任务、取消、本机 TCP echo | 使用系统分配的回环端口 |
| [scoop.charset](src/scoop.charset/examples/README.md) | 严格/有损转码、NFC、字素簇 | 独立运行 |
| [scoop.idna](src/scoop.idna/examples/README.md) | 国际化域名、原始 Punycode | 独立运行 |
| [scoop.url](src/scoop.url/examples/README.md) | 相对解析、组件构建、query 与表单 | 独立运行 |
| [scoop.json](src/scoop.json/examples/README.md) | 类型 codec、不可变值树 | 独立运行 |
| [scoop.regex](src/scoop.regex/examples/README.md) | 捕获、匹配、替换、分割 | 独立运行 |
| [scoop.crypto](src/scoop.crypto/examples/README.md) | SHA-256、增量摘要、HMAC | 独立运行 |
| [scoop.compression](src/scoop.compression/examples/README.md) | zstd/gzip 内存操作、文件流 | 临时文件自动清理 |
| [scoop.tls](src/scoop.tls/examples/README.md) | 同步 TLS 服务端与客户端 | 临时证书、两个终端 |
| [scoop.tls.asyncio](src/scoop.tls.asyncio/examples/README.md) | 异步 TLS、CA 文件加载、关闭握手 | 临时证书、两个终端 |
| [scoop.http](src/scoop.http/examples/README.md) | HTTP/1 内存编解码、增量 SSE | 独立运行 |
| [scoop.http.asyncio](src/scoop.http.asyncio/examples/README.md) | TCP 上的 HTTP/1 请求与响应 | 两个终端 |
| [scoop.http.client](src/scoop.http.client/examples/README.md) | GET、类型化 JSON POST | 与 server 示例配对 |
| [scoop.http.server](src/scoop.http.server/examples/README.md) | 路由、JSON、SSE、SPA | 前台服务，Ctrl+C 结束 |
| [scoop.terminal](src/scoop.terminal/examples/README.md) | ANSI 样式、输入协议解析 | 样式需支持 ANSI 的终端 |
| [scoop.terminal.asyncio](src/scoop.terminal.asyncio/examples/README.md) | 异步按键、尺寸、定时输出 | 真实 TTY |
| [scoop.readline](src/scoop.readline/examples/README.md) | 历史提示、Vi、多行输入和补全 | 交互终端或重定向输入 |

网络示例默认只使用本机回环地址，不需要外网。TLS 示例现场生成短期证书，不附带固定私钥。嵌套的 `consumer-tests`、`test-native` 和测试数据 Cone 是测试辅助项目，不属于这里的正式库清单。
