# 使用配套库与 API

先查询 `scoop example --search "关键词"`，读取相关主题，再扩展到当前版本的真实 API。不要仅因教程没有相应主题就认为某个库不存在。

## 查找顺序

1. `<SYSROOT>/src/<cone>/examples/README.md` 列出完整库示例；存在库根 `README.md` 时阅读其 API 概览与使用条件。
2. `<SYSROOT>/docs/index.html` 是随包离线 API 入口，选择应用目标；声明页给出签名、说明、类型链接和源码。自己的多 Cone 文档可由 `scoop doc` 生成。
3. 对尚不确定的类型、重载或行为，用 `rg` 在所选库的源码和测试中定位实际声明。例如 `rg -n 'fun decodeFromString|fun encodeToString' <SYSROOT>/src/scoop.json`。同时查看 package、可见性、泛型条件、默认参数和 suspend 标记。
4. 从库 `Cone.toml` 取得准确 coordinate 和版本，在应用中声明实际直接使用的库并添加所需 import。源码依赖则使用项目指定的源码版本。

编辑器已有 Scoop LSP 时，利用诊断、补全、签名、跳转定义、引用和重命名核对项目语义。LSP 不代替正式 CLI 构建／测试，也不负责安装工具链。

## 按需求找库

下面是配套库的查找入口，不是固定 API 清单；以当前安装根实际文件为准。

| 需求 | Cone 与材料 |
| --- | --- |
| Option、值类型、基本容器、StringBuilder、资源协议 | `scoop.core`；`language/*`、`text/strings`、`collections/list` |
| 集合算法、字典、字节／文本、日期与 Duration | `scoop.std`；`collections/map` 和库 `examples/` |
| 文件、目录、系统资源、线程 | `scoop.platform`；`io/files`、`io/resources` |
| 事件循环、任务、异步 I/O | `scoop.asyncio`；`async/tasks` |
| JSON 与类型 codec | `scoop.json`；`json/typed` |
| HTTP 请求、服务端、路由及流式交互 | `scoop.http.client`、`scoop.http.server`；`http/get` 及各库 README |
| HTTP 协议与异步传输适配 | `scoop.http`、`scoop.http.asyncio`；仅在需要协议层时使用 |
| 命令行参数与测试 | `scoop.cli`、`scoop.test`；`cli/options`、`test/assertions` |
| 正则、URL、IDNA、字符集与压缩 | `scoop.regex`、`scoop.url`、`scoop.idna`、`scoop.charset`、`scoop.compression` 的 README／示例 |
| 密码与 TLS | `scoop.crypto`、`scoop.tls`、`scoop.tls.asyncio` 的示例与 API 文档 |
| 数学、大整数和十进制数 | `scoop.math` 的 README／示例 |
| 日志与控制台 provider | `scoop.log`、`scoop.log.console` 的 README／示例 |
| 终端、异步终端与交互行编辑 | `scoop.terminal`、`scoop.terminal.asyncio`、`scoop.readline` 的示例 |

## JSON 与 DTO

从 `json/typed` 复制实际的 companion 声明：

```scoop
struct User(val name: String, val note: String? = None) {
    public companion object : ValueEncoder<User>, ValueDecoder<User>
}
```

`Json.encodeToString`／`Json.decodeFromString<User>` 使用公开 companion 能力；其他入口也可能接受显式 codec。struct 本身不意味着任意序列化入口都可用。增改字段时确认字段类型具有相应能力，并为缺字段行为提供真实默认值。JSON null、缺失字段和 Scoop 的 Option 分别遵循对应 API 合同。

## 异步与网络

`async/tasks` 展示普通 main 创建 `PlatformEventLoop()` 并调用 `loop.run suspend { scope -> ... }`。复用此入口及结构化任务作用域；需要返回值用 `async`／`await`，无业务结果用 `launch`／`join`。不要发明 `runBlocking` 或全局后台任务入口。

`http/get` 同时提供本机服务端与客户端。保持 `withHttpClient`、`withResponse` 等生命周期作用域，等服务开始监听后再运行客户端。读取正文遵守一次消费和读取上限的实际 API；例如示例的 `body.text(1_048_576L)`。这种 I/O 上限不代表所有业务请求都应使用同一数值。

更复杂的请求、路由、TLS、WebSocket 或取消处理按库 README 和具体声明实现。不要从同步函数名推导挂起版本，也不要将旧示例中的类型名移植到另一版工具链。

## 文件、终端与资源

从 `io/files` 查 `Path`、`OsString` 和文本／字节操作；从 `io/resources` 查正常与异常路径的关闭。路径表示、编码和字符串不是可任意互换的类型。只清理当前任务实际创建的临时资源。

终端／行编辑示例可能需要真实 TTY，异步 HTTP 示例需要活动事件循环与服务，线程共享状态需要同步。用符合各自前提的方式验证，不能把无 TTY 或服务未启动造成的失败归为语言错误。
