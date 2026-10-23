# Scoop HTTP Server 设计草案

日期：2026-10-24

状态：仓库内总体设计，正在分步实现；已交付范围见[实施记录](PROGRESS.md)，具体公共 API 合同见 [server 规范](../specs/SCOOP-HTTP-SERVER-SPEC.md)。

范围：在现有 Scoop HTTP、异步 I/O、TLS 和 Context 能力上建立应用层 HTTP server 库。本文不要求改编译器，不引入反射或通用 DI 容器。

本文根据当前工作区的规范和相关实现核对能力边界。尚未交付章节中的 scoop.http.server 类型、方法及其示例仍为拟议 API；业务侧 UserRepository、AuthContext、DatabaseSession 等是示例类型。示例用于说明契约，已实现 API 以库规范和源码为准。已有库能力与建议补齐的能力在下文分别标明。

本文迁入 `docs/http-server/` 时保留原设计主线，并在第 15 节规划 SSE 输出、multipart 上传、H1 WebSocket、响应压缩、请求解压、静态文件与条件请求、DTO query 与静态提取适配器。服务端 mTLS／动态 SNI 和 HTTP/3 保留原有范围说明，不在本次展开；第 12 节已有的基础 TLS 接入仍保留。第 16 节给出各增量的依赖、规范落点和交付标准。

## 1. 核心决策

建立普通 Cone **scoop.http.server**，以显式 HttpContext 和挂起 handler 为核心。借鉴 axum 的路由、提取和结果转换，使用接近 Vert.x Web 的显式应用组合方式；请求作用域和响应执行边界参考 ASP.NET Core，取消及流式生命周期参考 Armeria。

| 问题 | 决策 |
| --- | --- |
| Handler 入口 | suspend (HttpContext) -> HttpResult |
| 中间件 | suspend (HttpContext, HttpNext) -> Unit；next() 等到下游响应执行完毕 |
| 全局依赖 | 应用启动时显式构造，通过 handler 构造参数或闭包持有 |
| 请求依赖 | 每请求创建普通引用对象；显式传参，或由中间件使用 Scoop Context 绑定 |
| HTTP 上下文 | 每个请求一个 HttpContext；H1 每条请求独立，H2 每个 stream 独立 |
| 参数提取 | 显式 path/query/header/cookie/body API 和静态 codec；不扫描任意函数参数 |
| 返回值 | HttpResult 描述状态、字段、响应体；框架在请求管线内执行 |
| 认证结果 | 应用定义 AuthContext / Principal；HTTP 核心不固定业务用户模型 |
| 资源释放 | 显式所有权和 useAsync / finally；Context 只负责绑定与传播 |
| 底层协议 | 复用 scoop.http.asyncio；不再实现 HTTP parser、HPACK、TLS 或事件循环 |

“没有反射”不等于“不能注入依赖”：构造函数注入、闭包组合、显式工厂和静态生成都可行。这里不采用自动发现、运行期构造对象图和任意 handler 签名推断，因为它们不是完成 HTTP server 的必要条件。

首个稳定版本包括 HTTP/1.0、HTTP/1.1、HTTP/2、TLS、路由、中间件、请求 Context、基本提取、Cookie 和流式响应。实现过程先打通 H1 闭环，再接 H2。WebSocket、SSE 编码便利层、multipart 便利层等按独立增量交付，不成为基础管线的前置条件。

## 2. 参考框架及取舍

| 框架 | 借鉴内容 | 对 Scoop 的处理 |
| --- | --- | --- |
| axum | Router、类型化提取、结果转换、中间件组合、单次 body 消费 | 保留语义与使用体验；不照搬 Rust blanket impl、Tower Service、Poll 或固定数量 handler trait 实现 |
| ASP.NET Core | HttpContext、应用与请求寿命、RequestDelegate、结果执行、认证和授权分离 | 用普通对象和作用域表达寿命；不复制 ServiceProvider 和基于运行期元数据的参数绑定 |
| Vert.x Web | 每请求 RoutingContext、显式注册 handler、共享请求数据、失败和完成边界 | 使用挂起函数表达顺序；不复制 callback 的 next() 返回语义或字符串 Object 数据袋 |
| Javalin | 少量核心 API、直接 Context、手动注册路由、有类型应用数据入口 | 让简单应用直接可写；JSON 使用 Scoop codec，不假设 Java mapper 的反射能力 |
| Armeria | 流式响应、请求取消、Context 传播、明确的终结通知、装饰器 | 复用 Scoop Task-local Context；返回结果对象与完成响应必须分开定义 |

Vert.x Core 的执行 Context 与 Vert.x Web 的 RoutingContext 不是同一个寿命：前者与执行环境相关，后者属于请求。Scoop 的 EventLoop Context 也不能用来存放当前用户、当前 Cookie 或请求事务。

Vert.x 的 handler 调用 next() 后可以返回，异步工作继续发生。本设计采用挂起 next()，返回意味着下游结果已经执行完成。因此不能直接照搬它的 callback 生命周期。

ASP.NET Core 的请求参数绑定也不只是 DI：HttpContext、Request、Response、ClaimsPrincipal、取消信息和请求数据有各自规则。需要借鉴的是这些来源及其寿命，而不是统一从容器“按类型找任何东西”。

## 3. 当前 Scoop 已有的基础

| 层次 | 已有能力 | 对本设计的意义 |
| --- | --- | --- |
| 语言与 core | 泛型单态化、挂起函数、函数值、绑定方法引用、普通接口、结构化协程、Task-local Context | 能直接实现 handler、中间件、显式服务对象及请求绑定 |
| 条件接口 | 普通类型和 companion 可在原始声明内按正向约束实现接口；M57 已完成 | 泛型结果包装和 codec 可以按实际能力声明接口，不需要为了 HTTP 新增语言能力 |
| scoop.asyncio | TCP listener/stream、EventLoop、异步文件、sleep、NativeByteBuffer、AsyncCloseable | 提供接入、传输、计时和确定性清理 |
| scoop.http | HTTP/1 与 HTTP/2 消息及协议状态机、字段、Cookie、协商工具 | 保留原始语义与协议校验结果 |
| scoop.http.asyncio | 服务端 H1/H2 connection、单次 body、H2 多路复用、流控、超时、GOAWAY | 高层只需建立请求分派与寿命管理 |
| scoop.tls.asyncio | 服务端 TLS、证书配置、ALPN、异步握手 | HTTPS 无需自建 TLS 后端 |
| scoop.url | 有序 query、重复字段、百分号编解码、form-urlencoded | 参数层复用解析语义 |
| scoop.json 与 core codec | 显式 ValueEncoder/ValueDecoder、companion codec、JSON 编解码 | JSON 请求和响应不依赖反射 |
| 其他 HTTP 扩展 | multipart 底层编解码、SSE 接收、H1 WebSocket 握手及异步 session | 可在基础管线上逐项加便利层 |

现有异步库把 native 操作投递到所属 loop。外线程可以提交，第三方挂起点也可能在其他线程恢复；Task-local Context 随逻辑任务恢复。不能据此假设同一个可变业务对象没有数据竞争，也不能把 suspend 当成自动进入 worker pool。

两个语言边界直接影响 API：

1. interface 方法不能声明自己的类型参数；泛型虚方法同样不支持。因此泛型 json<T>() 等便利方法放在 final class 或顶层函数上。需要协议时可以使用宿主泛型接口，例如 RequestDecoder<T>。
2. Context requirement 不进入函数值类型。框架拿到 HttpHandler 后，不能仅凭其类型检查它是否要求 AuthContext、Logger 等绑定。

条件接口支持也不等于可以对任意已有函数类型追加一个通用 Handler 实现。首版继续使用固定 handler 签名；有限参数的类型化适配器可在存在实际调用需求时增加。

## 4. 分层与依赖

基础依赖关系：

~~~text
应用的服务对象、业务 handler、认证和授权
                    |
             scoop.http.server
       路由 / HttpContext / 管线 / 结果执行
                    |
             scoop.http.asyncio
       H1 connection / H2 connection 与 stream
                    |
      scoop.asyncio + 可选 scoop.tls.asyncio
                    |
                 TCP / TLS
~~~

scoop.http.server 可以依赖 scoop.url 和 scoop.json，提供常用应用 API；multipart 文本提取复用 scoop.charset。公共类型复用 HttpField、HttpRequestHead、HttpSetCookie、Duration 等现有类型，避免为同一个概念再建一套格式或校验器。

职责边界：

- scoop.http 负责 HTTP 字节和消息合法性、定界、状态机、字段解析与协议协商。
- scoop.http.asyncio 负责协议驱动、真实 I/O、流控、操作取消、连接与 stream 的本地清理。
- scoop.http.server 负责路由、应用状态组合、请求作用域、提取、错误映射、结果执行和 server 停机。
- 应用负责业务模型、认证凭证验证、授权规则、数据库事务和外部服务配置。

若高层需求暴露出 H2 头部时限等真实缺口，应在所属层补一个最小接口并同步该层规范，不让 server 读取底层 private 状态。也不为潜在的多种后端先建立通用插件体系。

## 5. 公共 API 的骨架

函数类型：

~~~scoop
public typealias HttpHandler = suspend (HttpContext) -> HttpResult
public typealias HttpNext = suspend () -> Unit
public typealias HttpMiddleware =
    suspend (HttpContext, HttpNext) -> Unit
~~~

HttpNext 已绑定当前请求，调用者不传另一个 HttpContext。它不是 Java SAM 接口，Scoop lambda 直接使用函数类型。

| 类型 | 主要内容和约束 |
| --- | --- |
| RouterBuilder | 注册方法、路径、分组、中间件和 fallback；build 时检查路由冲突并冻结 |
| RouteGroup | 组合前缀和组内中间件；配置期对象，不是运行期 scope |
| HttpApplication | 不可变路由表及管线；可供一个或多个 server 使用，闭包内依赖的并发规则由应用保证 |
| HttpContext | request、response、route、pathParams、connection、requestId、requestJob；以及挂起 respond(result) |
| HttpRequest | 原始 head、解析后的 path/query/cookies、单次 HttpRequestBody |
| HttpRequestBody | openStream()、bytes(maxBytes)、text(maxBytes)、json(codec, maxBytes) 等互斥消费入口 |
| HttpResponse | 提交前的字段修改、Cookie 写入、beforeCommit、isCommitted；handler 通常不直接写网络 |
| HttpResult | 普通响应保存状态码、字段和 HttpBody；WebSocket 增量另有明确的升级结果，见 7.5、15.3 |
| HttpRejection | 携带明确 HTTP 结果的请求拒绝异常；提取器及应用可用它表达预期失败 |
| HttpBody | 基础版含空体、字节快照和挂起 producer；静态文件增量增加延迟打开的 File，见 15.6 |
| HttpBodyWriter | 受框架管理的 AsyncByteSink 视图；支持短写、flush；最终 finish 由框架执行 |
| HttpServer | 拥有 listener 和接管的连接；提供 awaitTermination、shutdownGracefully、close |

不公开泛型 ServiceProvider、GetService<T>()、自动控制器扫描或注解路由。

启动使用一个明确接管所有权的入口。拟议签名如下，仅展示 API 表面：

~~~scoop
HttpServer.takeOwnership(
    listener: AsyncTcpListener,
    application: HttpApplication,
    scope: CoroutineScope,
    options: HttpServerOptions
): HttpServer

server.awaitTermination(): Unit
server.shutdownGracefully(grace: Duration): Unit
server.close(): Unit
~~~

上述四个操作均声明 suspend。工厂完成配置检查、活动 Job 检查并成功登记 server 所有者之后接管 listener；登记前失败由调用方负责，登记后失败由工厂收尾。返回表示接受循环已建立。

只由工厂启动一次接受循环，不再另设一个容易与它重复启动的 run()。awaitTermination 只等待；shutdownGracefully 停止接入并等待已有请求；close 是强制本地关闭。调用方持有的数据库、HttpClient、配置对象不会因为被 handler 捕获就转交给 server 关闭。

## 6. 全局 state 与请求 Context

### 6.1 应用级依赖

“global state”指一个应用实例共享的对象，不是进程内 static 全局变量。

~~~scoop
class UsersApi(private val users: UserRepository) {
    suspend fun list(ctx: HttpContext): HttpResult {
        val page = ctx.request.query.optionalLong("page") ?: 1L
        return json(users.list(page))
    }
}

fun createApplication(users: UserRepository): HttpApplication {
    val api = UsersApi(users)
    val routes = RouterBuilder()
    routes.get("/users", api::list)
    return routes.build()
}
~~~

每个测试可以构造自己的 UserRepository；同一进程可以运行两个不同配置的应用。构造函数注入不需要反射，也不需要容器。

默认不把 AppState<T> 泛型传播到 Router、HttpContext、HttpResult 和所有中间件。只有业务代码需要知道应用状态的具体类型；库级中间件通常只处理 HTTP。

若一个大应用确实需要聚合依赖，可以自行定义 ApplicationServices 普通 class。聚合对象仍由应用启动代码显式创建。

### 6.2 请求级数据

以下数据属于不同寿命：

| 数据 | 创建和释放 | 传递方式 |
| --- | --- | --- |
| 配置、DB pool、共享 HTTP client | 应用启动与应用停机 | 构造函数或闭包 |
| HttpContext | 每条请求进入与终结 | 显式 handler 参数；框架同时绑定精确 HttpContext Context |
| AuthContext、tenant、request logger | 每条请求由相应中间件创建 | 显式对象或应用自定义 Context |
| DB session / unit of work | 实际需要它的请求作用域 | 显式工厂、useAsync 和 Context |
| HTTP session | 通过 Cookie 等跨请求关联的独立存储 | 可选应用组件，不等同于请求属性 |
| TCP 连接信息 | 一条连接 | 请求只持有只读视图，不存业务请求数据 |

首版不强制把认证结果设计成 HttpContext.user: Any。应用可以定义 AuthContext，包含它实际需要的 Principal、角色、租户和认证来源。这样无需运行期类型搜索，也不把 JWT、OAuth、数据库 User 模型固定进 HTTP 核心。

首版也不增加字符串到 Any 的请求属性袋。若今后出现两个独立库必须通过开放扩展数据协作的实际需求，可增加按对象身份区分的 AttributeKey<T>，明确只做数据存取、不构造服务。它不是当前实现的前置条件。

HTTP 核心不固定 Principal，不妨碍应用在自己的 AuthContext 上提供统一的 user、roles 和 requireUser。未来若多个认证库确需互操作，可先约定一个小的普通 Principal 接口，无需引入身份服务容器。

### 6.3 Scoop Context 的绑定

~~~scoop
fun authentication(authenticator: Authenticator): HttpMiddleware =
    suspend { ctx, next ->
        val auth: AuthContext = authenticator.authenticate(ctx.request)
        context(auth) {
            next()
        }
    }

context(auth: AuthContext)
suspend fun currentUser(ctx: HttpContext): HttpResult {
    val user = auth.requireUser()
    val locale = ctx.request.cookies.getSingle("locale") ?: "zh-CN"
    return json(user.profile(locale))
}
~~~

此例中的 authenticate 由应用定义：没有凭证时可以返回匿名 AuthContext，非法凭证通过该应用的认证中间件产生明确的 HTTP 拒绝。requireUser 同样是应用的授权便利方法，缺少已认证用户时产生约定的 401，而不是 MissingContextException。

组合时，认证先建立请求数据，随后按路由需要打开 DB session，再调用使用这些 Context 的 handler：

~~~scoop
fun createUserApplication(
    users: UserRepository,
    authenticator: Authenticator,
    database: DatabasePool
): HttpApplication {
    val api = UsersApi(users)
    val routes = RouterBuilder()
    routes.get("/users", api::list)
    routes.group("/account") { group ->
        group.use(authentication(authenticator))
        group.use(requestDatabase(database))
        group.get("/me", ::currentUser)
    }
    return routes.build()
}
~~~

这里 /account/me 的认证结果与 DB session 每请求创建；UsersApi 捕获的 UserRepository 是应用级共享依赖。需要在打开 DB 前拒绝匿名访问时，在 authentication 与 requestDatabase 之间安装相应授权 middleware。

框架在执行全局管线前绑定 HttpContext。业务 Context 由中间件按其具体作用域绑定。多项绑定使用嵌套 context(value) 块。

必须保持下列语言语义：

- 精确类型匹配。LoggerImpl 不满足 Logger 的 key；应先赋给静态类型为 Logger 的变量再绑定。
- Context key 必须是非空引用类型。需要绑定值类型、Option 或不同角色的同型对象时，使用普通 nominal wrapper。
- typealias 不产生新的 key。
- 函数引用不捕获创建时的 Context；实际调用时才查找。注册 handler 不等于验证其所有业务 Context 已安装。
- 子协程继承绑定快照，后续重绑定彼此独立；对象引用仍可共享，Context 不提供同步。
- 缺失 Context 是应用装配或程序错误，通常映射为 500 并记录诊断；匿名用户、缺少凭证和权限不足走认证/授权规则。
- Context 不执行 dispose、close、事务提交或回滚。

### 6.4 请求级资源与并发子任务

~~~scoop
fun requestDatabase(pool: DatabasePool): HttpMiddleware =
    suspend { ctx, next ->
        val db: DatabaseSession = pool.openSession()
        db.useAsync(suspend { session ->
            context(session) {
                next()
            }
        })
    }
~~~

这里的 next() 覆盖 handler、结果编码、响应体写入和 finish，因此流式 producer 仍可访问 session。退出绑定后再结束 useAsync；错误和取消沿同一结构清理。

HttpContext 不提供一个可随意把任务延长到中间件作用域之外的公共 launch()。需要并发工作的 handler 应使用局部 coroutineScope，并在离开使用该资源的作用域之前等待子任务。仅仅“同属整个请求的任务树”不保证某个更内层 DB 作用域还活着。

若应用把工作投递到自己的长期后台 scope，应复制必要的业务值，并获取后台任务自己的资源，不能携带当前请求的 response、body 或已关闭 session。

资源存活与事务边界分开：

- session 可以存活到响应结束。
- 要让事务失败改变 HTTP 状态，事务提交必须在响应提交之前完成，通常放在 handler 内的显式 withTransaction 中。
- 在 await next() 之后自动提交事务，可能已向客户端发送成功状态。这不是默认事务模型。
- 流式响应中边输出边修改数据库需要应用自行定义一致性，框架不能撤回已经发送的内容。

## 7. 管线、响应提交与错误

### 7.1 两层调用模型

应用 handler 返回 HttpResult，内部终端 delegate 返回 Unit：

~~~text
请求任务
  └─ 绑定 HttpContext
      └─ 全局中间件 A
          └─ 全局中间件 B
              └─ 路由匹配
                  └─ 分组/路由中间件
                      └─ 调用 handler
                      └─ 准备 HttpResult 和响应字段
                      └─ 提交响应头
                      └─ 写响应体、trailers、finish/flush
                  └─ 路由中间件返回与清理
              └─ 全局中间件返回与清理
      └─ 请求终结通知
~~~

这解决一个关键问题：如果 middleware 的 next() 只等待 handler 返回结果对象，AuthContext 和 DB session 会先退出，流式响应却还没执行。

终端适配器的概念行为：

~~~scoop
suspend fun invokeEndpoint(ctx: HttpContext, handler: HttpHandler) {
    val result = handler(ctx)
    ctx.respond(result)
}
~~~

实际实现还包括以下错误边界、响应状态和清理规则，不把该示意函数当成完整实现。

### 7.2 顺序与 next()

use(A)、use(B) 的进入顺序是 A → B → endpoint，返回顺序是 endpoint → B → A。分组中间件按外层到内层进入，随后是路由自身的中间件。

每次 middleware 调用只能选择：

1. 调用 next() 恰好一次并等待它完成；
2. 调用 ctx.respond(result) 发送终结响应；
3. 抛出失败，交给相应错误边界。

next() 调用两次、两个分支并发调用、next() 返回后再次响应，均是程序错误。不自动重试 handler，因为请求体、数据库操作和响应写入可能已经发生。

middleware 正常返回却既未委托也未响应时，框架报告程序错误；若尚可响应则产生 500，避免请求无限悬挂。

### 7.3 响应提交

HttpResponse 的状态只表达必要的事实：准备中、已提交、已结束、已中止。每个请求最多提交一个最终响应；信息响应不算最终响应。

字段处理顺序：

1. 在当前请求作用域内准备结果。普通结果直接取得描述；File 结果在这里挂起打开文件并读取同一句柄的元数据，尚不读取正文。准备失败先释放本次资源，再进入错误映射。
2. HttpResult 的状态码和字段成为基础响应，按调用顺序应用 ctx.response 上暂存的 set/add/remove 操作。
3. 按登记顺序调用普通、非挂起的 beforeCommit 回调，允许最后修改应用响应草稿。
4. 按已安装的明确策略选择表示，处理内容编码及文件的条件请求／Range，确定最终状态、表示字段和 body 计划。压缩可能移除原始 Content-Length；文件条件判断可能产生 304／412／416。这里不是任意用户回调链，具体规则见 15.4、15.6。
5. 根据请求方法、最终状态和 body 选择定界，交给协议层发送；一旦开始不可撤回的发送尝试，即保守地视为已提交。
6. 正常完成 body、内容编码、trailers 和 HTTP finish；无论成功、失败、HEAD 抑制或条件请求短路，都在本次响应执行退出前关闭已取得的资源。

set 按大小写无关的字段名替换全部同名项；add 保留重复项；remove 删除全部同名项。Cookie append 产生独立 Set-Cookie 字段，不进行逗号合并。后执行的修改遵守上述操作语义。

beforeCommit 不承担异步事务提交或复杂 I/O。需要这些工作时，应在 handler 返回结果前完成。await next() 返回时通常已发送完响应，此时适合统计与清理；修改字段应直接报状态错误。

beforeCommit 观察的是最终表示选择之前的草稿，不保证它写入的长度就是编码后的线上长度。Content-Length、Content-Encoding、ETag、Content-Range 等必须与结果的实际表示一致；专用结果保留其决定表示的字段，冲突修改在发送前报程序错误。观察真正发出的状态和字节数使用完成快照，不在 beforeCommit 中猜测。File 的资源准备和压缩器的释放由当前响应执行直接拥有，不向所有结果引入通用异步工厂或可插拔执行器。

编码器继续负责消息合法性检查；server 不再完整重放协议层的校验。HttpResult 中非法的字段或定界属于服务端程序错误。

### 7.4 错误边界

| 失败位置 | 行为 |
| --- | --- |
| path/query/JSON 等应用层提取失败，尚未提交 | 产生约定的 400、413 或 415 |
| 认证、授权拒绝 | 应用明确产生 401/403 或其他声明的结果 |
| handler 或结果准备失败，尚未提交且协议连接仍可用 | 在当前请求管线内映射为错误 HttpResult |
| middleware 在进入下游前失败 | 外层请求边界处理；不能假定该 middleware 的业务 Context 已成功安装 |
| beforeCommit 自身失败 | 不重复执行已调用的回调；丢弃有问题的草稿，尝试不再经过这些回调的最小 500 |
| 响应头或部分 body 已发送后失败 | 中止 H1 连接或对应 H2 stream；记录失败，不再发送第二个最终响应 |
| 客户端取消、底层 I/O 失败、协议解析失败 | 按真实连接/stream 状态收尾；不承诺能再发送错误状态 |
| 发送完成后的 middleware / cleanup 失败 | 记录真实清理失败；不改写已经发出的成功状态 |

handler 的错误映射位于终端内部，使其错误响应仍在已经建立的认证、DB 等中间件作用域内执行。最外层还有一个仅依赖基础 HttpContext 的兜底边界，处理 middleware 自身失败。

错误映射器不能递归处理自己的失败。失败的错误结果或失败的最小响应直接进入中止路径。

默认错误正文只包含稳定、简短的信息和框架生成的 requestId。详细异常留给应用配置的错误观察入口。取消单独归类；504 留给实际网关上游超时，不作为所有服务端 deadline 的通用状态。

### 7.5 普通响应与协议升级

加入 H1 WebSocket 时，HttpResult 用封闭的结果分支区分普通响应和 WebSocket 升级。升级不是 `HttpBody.Stream`，不能由一个普通 101 加任意 producer 伪造接管。普通 handler 仍返回同一个 HttpResult；原有 text／json／stream 等函数只构造普通分支。

对普通响应，next() 始终等待正文执行结束；对升级结果，它等待握手发送和 transport 所有权交接。HTTP 中间件随后返回并清理，WebSocket 会话任务在 HTTP 管线真正退出后开始。会话不继承本请求的 HttpContext、DB session 或认证 Context，使用升级 handler 显式提供的不可变业务值和共享应用依赖重新建立自己的作用域。具体交接、失败和停机规则见 15.3。

## 8. 路由语义

首版实现静态片段、单片段参数和末尾通配，不把正则引擎作为路由依赖。

| 规则 | 决策 |
| --- | --- |
| 静态路由 | /users/me |
| 单片段参数 | /users/{id}，必须匹配非空片段 |
| 末尾通配 | /assets/{*path}，必须位于末尾且匹配非空后缀 |
| 优先级 | 静态片段高于参数，参数高于末尾通配 |
| 冲突 | 同方法的同一形状、重复路由、不同参数名但相同位置形状在 build 时拒绝 |
| 大小写 | path 和方法 token 均保留相应协议语义；path 大小写敏感，方法不自动大写 |
| 尾斜线 | /users 与 /users/ 不同，不自动跳转 |
| 连续斜线 | 保留，不自动折叠 |
| 点片段 | 不自动解析 . 或 ..；作为路由路径片段处理 |
| 查询字符串 | 不参与路径匹配 |
| 404 | 没有路径匹配时执行当前分组的 fallback；默认 404 |
| 405 | 已选中的路径没有对应方法，返回 405 并给出 Allow |
| HEAD | 显式 HEAD 优先，否则复用 GET handler 的结果元数据并抑制 body |
| OPTIONS | 显式 OPTIONS 优先；普通路径默认返回 204 和 Allow，服务器级 OPTIONS * 单独处理 |
| CONNECT | authority-form 不进入普通路径路由；首版未配置隧道能力时返回 501 |
| Upgrade | 普通请求仍可按路径处理；只有显式升级结果才发送 101 并转移 transport |

先按路径优先级选择资源，再做方法选择。比如 /users/me 只有 GET，而 /users/{id} 有 POST，POST /users/me 返回静态路径的 405，不转而执行参数路由。该规则使注册顺序不改变目标资源。

Allow 包含显式方法及适用的隐式 HEAD、OPTIONS，以稳定顺序输出。OPTIONS * 默认返回 204，列出应用可处理方法的并集；它经过全局中间件，但没有普通路径 route。单纯收到 Upgrade 头不会自动升级或改变 transport 所有权；未选择升级结果时仍是普通 HTTP 响应。

实现选择按路径片段建立的 trie：构建时保存静态分支、参数分支和末尾通配分支，运行时在完整匹配之间应用上述优先级。静态分支走到死路时可以回退到参数分支，不能因某个静态前缀存在就漏掉完整可匹配的路由。参数名属于最终选中的路由，内部捕获按位置保存。没有完整端点匹配时才选择最长适用分组的 fallback。

可参考 matchit 的路由冲突与优先级测试，但它是 Rust 库，不能直接作为 Scoop 普通源码依赖。首版只实现这里列出的有限路由规则，不为桥接 matcher 单独增加一套 native 后端；出现实际性能瓶颈后再比较优化或 FFI 的成本。

分组前缀在 build 时组合。同形状的分组前缀不能重复注册，避免 fallback 和组级中间件具有两个相同归属。已进入分组但未命中端点时，执行该分组的 fallback 及其约定的组级中间件；405 也属于已经选中的资源，不误落到根 fallback。404 和默认 405／OPTIONS 按最长适用分组选择组级中间件，等长按静态片段优先；正常端点使用其注册分组和端点中间件。全局中间件覆盖正常路由、404 和 405。构建结果保存足够的分组匹配信息，不在每请求重建 Router。

路由匹配之前 ctx.route 尚无值。授权等依赖端点的 middleware 应注册在分组或路由层；访问日志等全局 middleware 可以在 next() 之后观察最终 route 模板。

路径处理按原始编码片段划分：

- 保留原始 request target。
- 先按原始斜线分片；百分号编码的 %2F 不变成新的路由分隔符。
- 每个片段最多解码一次，再与路由文字或参数匹配；普通字符的等价百分号编码因此可以命中同一文字片段。
- 含编码斜线的参数可以通过参数 API 取得解码后的值，但不会变成另一个路由层级。
- 参数提供原始字节和严格 UTF-8 读取方式；普通 String 路由不能默默替换非法 UTF-8。
- 畸形百分号转义属于高层 400；底层已判为非法的消息沿协议错误路径处理。

首版普通应用接收 origin-form 和等价的 H2 path；绝对形式的代理请求目标、CONNECT authority-form 不自动套用该规则。若要支持 forward proxy，应增加明确模式及测试，不通过通用 URL 规范化悄悄改变 origin server 的路由语义。

## 9. 参数提取与请求体

### 9.1 常用提取

| 来源 | 默认语义 |
| --- | --- |
| Path | 必需参数缺失属于路由/程序错误；标量转换失败为 400 |
| Query | 保留顺序、重复项、无等号与空值的区别 |
| Header | 保留多值和原始字节；只在对应字段的规则允许时合并 |
| Cookie | 保留全部同名值；读取 Cookie 不表示完成认证 |
| JSON | 验证 Content-Type，以选定 ValueDecoder 解码；语法或类型不匹配为 400 |
| Form | 使用 form-urlencoded 的加号和百分号规则，与普通 URI query 区分 |
| Multipart | 通过现有增量解码器读取；part 便利 API 与限制见 15.2 |

单值 query 便利方法拒绝重复值；getAll 用于明确的列表语义。required/optional 只区别缺失，空字符串仍按目标类型解析。普通 URI query 中的 + 保留为加号；form-urlencoded 中的 + 表示空格。

JSON 默认 codec 来自 T.Companion : ValueDecoder<T>；始终提供显式 codec 入口。响应编码对应 ValueEncoder<T>。这些约束放在普通泛型函数或 final class 方法上。

json 提取器接受 application/json 与声明为 JSON 的 +json 媒体类型，采用严格 UTF-8。没有启用相应 Content-Encoding 解码器时，结构化提取器明确返回 415；原始流接口仍可交付编码后的字节。由当前请求输入触发的解析/解码错误在提取器边界转换为 HttpRejection，不把业务代码里任意 IllegalArgumentException 或 DecodingException 一概当成客户端 400。

DTO query 解码器适配现有 codec，固定扁平字段规则：重复标量拒绝、列表显式读取、缺失字段遵守 decoder 的 optional/default、未知字段按显式策略处理。具体方案见 15.7；不发明隐式的嵌套对象和数组索引命名协议。

### 9.2 Body 只能选择一个消费路径

HttpRequestBody 不在默认入口自动缓存。openStream、bytes、text、json、form 共享同一次 body 消费权；重复选择或并发读取报程序错误。

后续 multipart、已启用内容解码后的流和显式 openRawStream 也使用同一消费权；不能先读一部分压缩字节再切换成 JSON／multipart。原始 head 始终保留线上 Content-Encoding 和 Content-Length，解码后视图及两种字节上限见 15.5。

bytes/text/json 是有界便利方法。结果由调用方持有；第二次调用不隐式返回一个框架隐藏的缓存。需要日志和 JSON 共用内容时，显式读取一次并传递同一快照。

已有 JSON 实现处理完整文本/值树，首版 JSON HTTP API 是有界聚合解码，不能称为流式 JSON parser。大对象或长数据流使用原始 body、应用定义的逐项协议或后续真正的流式 codec。

请求体上限按实际读取的数据累计，而不只相信 Content-Length。已声明长度超过上限可以提前拒绝；没有声明长度仍在读取时执行同一上限。body 超限通常停止正常处理，是否还能发 413 取决于当前传输状态。

100-continue 在应用第一次真正需要 body 时、通过可提前完成的路由与认证检查后发送；直接拒绝的请求不先发送 100。未知 Expect 的判定需要完整 token 语义，不能仅把已有 httpExpectsContinue 的布尔结果当作完整验证；最小补齐放在 HTTP 消息工具层。

### 9.3 请求体未读完

H1 的 AsyncHttp1Body.close 在 body 未完成时关闭整个连接。因此不得在发送 401/413 之前先关闭 request body，然后再尝试写响应。

首版默认：

- 请求体正常结束后才允许正常复用 H1。
- 提前响应且 body 未结束时，先按关闭连接的响应路径写完结果，再关闭 H1。
- 不为复用自动读取任意大小的剩余上传。
- 如后续加入 drain，只能是显式的字节上限和时间上限，并有实际性能或使用需求。

H2 的输入和输出可以并行。提前响应时先完成输出，再放弃尚未消费的输入；不能在响应发送之前调用会重置整个 stream 的 close。正常双向结束的流由底层退休逻辑收尾，不对每个成功 stream 无条件追加 CANCEL。

## 10. Cookie、认证与授权

RequestCookies 对已有 HttpCookieHeader 做只读包装，延迟解析并在同一请求中复用解析结果：

- getAll(name) 返回按收到顺序排列的值。
- getSingle(name) 在无值时返回 None；多个同名值报提取错误，避免认证 Cookie 的选择依赖隐含 first/last 规则。
- 提供字节接口，String 接口执行明确的编码检查。
- 不把 Path、Domain 等响应 Cookie 属性推断为客户端已经验证的身份信息。

ResponseCookies 复用 HttpSetCookie：

- append(cookie) 追加独立 Set-Cookie。
- delete(name, path, domain) 写入相匹配的过期 Cookie；路径和域须由调用方明确选择。
- 删除不能保证清除浏览器里所有未知 Path/Domain 的同名 Cookie。
- 写响应 Cookie 不修改当前请求的 RequestCookies。
- Secure、HttpOnly、SameSite 等按显式属性处理；认证组件可以提供自己的默认值，HTTP 核心不猜认证用途。

认证与授权分为两个普通组件：

1. 认证从请求读取凭证，验证后构造 AuthContext。没有凭证可以保持匿名，非法凭证按选定策略拒绝。
2. 授权根据路由和已认证主体决定是否允许访问。需要认证但没有身份时通常返回 401，并由认证组件提供适用的 WWW-Authenticate；已认证但权限不足通常返回 403。

框架不默认把认证失败重定向到登录页。浏览器站点可以显式注册重定向结果，API 可以使用状态码与 JSON。

Session store、JWT/OAuth 协议实现、CSRF、CORS 和可信代理处理按应用或独立中间件实现。跨请求 session 不放进请求 Context 作为隐式持久存储。X-Forwarded-* 等头只有在应用配置可信代理来源后才能影响外部 URL 和客户端地址解释。

## 11. 结果与流式响应

HttpResult 的普通响应分支由状态码、字段和 HttpBody 组成。text、bytes、json、redirect、empty 都是构造它的普通函数；自定义 CSV、图片等结果也返回同一种结构。WebSocket 升级使用 7.5 的独立分支。

首版 HttpBody 支持三种实际需要的形态：

1. Empty。
2. Bytes：独立字节快照，可得 Content-Length。
3. Stream：挂起 producer，在响应执行时调用，向 HttpBodyWriter 写入；可提供已知长度和结束 trailers。

静态文件增量增加第 4 种 File，描述路径、媒体类型和文件选项，构造时不打开文件。它的准备、条件判断、区间读取和清理在响应执行中完成，见 15.6。SSE 继续复用 Stream，压缩是执行时的输出包装，不再各自增加一种 body。

流式结果示例：

~~~scoop
context(db: DatabaseSession)
suspend fun exportUsers(ctx: HttpContext): HttpResult {
    return stream(
        contentType = "text/csv; charset=utf-8",
        produce = suspend { output ->
            val rows = db.openUserExport()
            rows.useAsync(suspend { cursor ->
                writeCsv(cursor, output)
            })
        }
    )
}
~~~

该例要求 requestDatabase 的作用域覆盖 ctx.respond。函数引用和 producer 本身不负责延长 DB 的确定性寿命；是管线的等待边界使资源保持有效。

首版不接管已经打开的任意 AsyncByteSource 作为可遗弃的结果资源。producer 应在自己的执行范围中获取并关闭资源，或使用外层请求作用域拥有的资源。这样 HEAD 抑制、结果被错误替换、handler 返回后中止等路径都不需要猜测哪个打开的 source 应被谁关闭。

结果的字段和字节数据以独立快照保存。producer 每次响应执行最多调用一次；应用不要跨请求复用捕获某条请求可变对象的 producer。共享不可变结果描述不意味着捕获的业务资源自动线程安全。

定界与完成规则：

- H1 已知长度且没有 trailer 时使用 Content-Length；未知长度在 HTTP/1.1 使用 chunked，在需要时按 HTTP/1.0 的连接关闭定界。显式提供 trailer 回调时，H1.1 也使用 chunked，不能同时提供 Content-Length；仍检查声明的原文长度。H1.0 的 trailer 在提交前报程序错误。
- H2 不发送 Transfer-Encoding: chunked；通过 DATA 和 END_STREAM 表达内容。
- HEAD 保留允许的元数据，不执行 stream producer 来计算长度或生成内容。
- 204、304 等按 HTTP 规则抑制或禁止 body；不能通过高层便利 API 绕过底层规则。
- 已知长度与实际写出长度不一致是发送失败。
- trailer 与普通字段分开，在 finish 时提交，并遵守协议支持和必要声明。
- writer 允许短写；便利 writeAll 必须循环推进并响应取消。
- finish/flush 表示已接受输出完成相应本地传输步骤，不表示客户端应用已接收、解析或持久化数据。

写入等待流控和真实 I/O，producer 由背压自然暂停。框架不为流式 body 建立无界队列，也不先把整个响应收集到内存。

## 12. H1、H2 与 TLS 接入

### 12.1 连接入口

server 接收 TCP 后，为握手和协议接管建立连接作用域。TLS 握手在各自连接任务执行，不能阻塞整个 listener 的下一个 accept。

连接任务也在自己的嵌套 scope 边界捕获失败。单次握手或单条连接失败只结束该连接；listener 或 server 所有者的终止才结束整个服务。不能把所有连接直接作为会向同一父 Job 传播业务异常的无保护 launch。

- 明文默认 H1。
- 可显式启用 H2 prior knowledge，复用现有探测和前缀交接能力。
- TLS 按协商后的 ALPN 选择 h2 或 http/1.1；无 ALPN 时按配置允许 H1。
- 只宣告实际实现并启用的协议。
- h2c Upgrade 作为单独增量，不是首个 H2 闭环的必要入口。

H1 默认沿用严格接收配置。server 的 H2 接收配置建议显式选择 Http2InvalidRegularField.RejectStream；这是高层选择，区别于当前底层 Http2ReceiveOptions 的 Ignore 默认值，不需要改写底层默认行为。

所有权转移复用 takeOwnership 约定。协议接管后 server 不再绕过 HTTP connection 直接读写原始 TCP/TLS stream。

### 12.2 H1 请求循环

每条连接按协议顺序执行：

1. receiveRequest 获得完整 head 和单次 body。
2. 创建新的 HttpContext 与请求隔离作用域。
3. 执行管线并完成响应。
4. 完成必要 body/连接清理。
5. 仅在协议状态允许复用时读取下一条请求。

首版不在一条 H1 连接上并行执行多个业务 handler，不建立响应重排序缓冲。即使底层收到流水线字节，也按请求顺序处理。

H1 取消可能关闭整个连接。底层协议错误也可能在向高层抛出前就已关闭连接；不能承诺所有畸形报文都会得到一个 400。

H1 当前没有高层可直接使用的、独立于读写的“客户端已离开”通知。handler 只等待 DB 且没有网络操作时，不能保证立即感知对端关闭；已观察到的致命 I/O 故障、请求 deadline 和服务停机才触发对应取消。TCP 输入 EOF 也可能只是客户端半关闭发送方向，仍在等待响应，不能一概当作请求取消。若实际应用需要更及时的通知，应在异步协议层验证最小观测接口，不让 server 额外抢读并消费请求数据。

llhttp 的请求方法识别集合仍是当前 H1 接收能力的边界。高层注册任意方法字符串不会自动扩展 parser；H2 的方法 token 能力也不能用来宣称 H1 已支持同样的自定义方法。

### 12.3 H2 请求与取消

每个 stream 创建独立 HttpContext 和请求 Job，共享连接的协议驱动和 TLS I/O。业务 handler 不运行在 HTTP/2 onEvent 回调或底层锁内。

需要把对端 RST_STREAM 和 stream 故障传播到等待数据库等外部操作的请求：

- onEvent 中只完成框架内部的短操作：记录流失败并通知对应请求 Job。
- 流交付、请求 Job 登记和失败事件之间存在竞争；登记后读取已有 stream.failure，避免漏掉提前发生的失败。
- 不把会抛异常的应用观察回调直接作为底层 onEvent。当前底层回调抛异常会结束连接。
- 请求取消只收尾对应 stream；连接所有者取消才结束共享传输。

当前实现还有一个必须保留的细节：acceptRequest 交付 stream 时，把该 stream 的取消监听绑定到执行 accept 的 Job。取消公共接受 Job 可能取消此前交付而仍活动的 stream。

因此：

- 正常停机通过 GOAWAY 结束接受过程，使 acceptRequest 在排队请求处理完后自然返回 None。
- 不用“取消 accept task”作为仅停止接收新 stream 的操作。
- 各请求另登记自身的取消与故障映射，并在请求结束时解除登记。
- 若原型证明确需显式转交 stream 所有者，应在 scoop.http.asyncio 增加一个范围明确的所有者交接操作；不能直接访问内部 listener 字段。

该交接操作不是当前已存在的 API，也不是未经验证就预先新增的框架要求。先验证现有 GOAWAY、监听和请求 Job 组合能否满足完整生命周期。

### 12.4 TLS 能力边界

TlsServerContext 可由应用共享，连接持有各自的异步 TLS 状态。首版配置单套证书/私钥及实际支持的 ALPN。

当前已核对的能力不包含完整的服务端客户端证书验证策略，也没有本设计可直接采用的动态 SNI 证书选择 API。不能把客户端 mTLS 配置等同于服务端已经能要求和验证客户端证书。

HTTP drain 与 TLS close_notify 也有不同含义。当前 HTTP connection 拥有传输和可能仍活动的共享 I/O；不能绕过它直接对 TLS stream 调用需要双向独占的 shutdownGracefully。基础停机承诺 HTTP 请求排空与本地资源收尾；TLS 优雅关闭的进一步组合需要在所属传输适配层明确交接顺序。

## 13. 限制、超时与请求隔离

限制只针对实际资源，不建立通用 CPU/内存计费或动态预算框架。

建议的首版默认值如下，实施前通过真实用例校准并写入库规范；这些是 server 的提议值，不冒充底层现有默认行为。

| 选项 | 建议值/策略 | 作用 |
| --- | --- | --- |
| HTTP header/start-line 限制 | 沿用现有 HttpHeaderLimits、Http1Limits | 已有默认总字段 64 KiB、单字段 16 KiB、100 项、H1 起始行 8 KiB |
| H2 协议缓冲与 SETTINGS | 沿用 Http2Limits / Http2Settings，并可显式覆盖 | 保持协议窗口和实际消息队列的边界 |
| bufferedBodyLimit | 2 MiB | bytes/text/json 等聚合入口的默认上限 |
| requestBodyLimit | 8 MiB，可按路由增大或显式关闭 | 所有请求 body 消费路径累计的默认上限 |
| maxConnections | 1024，可配置 | 已接管连接数 |
| maxActiveRequests | 256，可配置 | 正在执行的应用请求；长响应也占用 |
| 应用请求等待队列 | 首版不建立无界队列 | 无容量时直接拒绝或中止，避免只限制运行数却无限积压 |
| tlsHandshakeTimeout | 10 s | 每条连接的握手总时限 |
| firstRequestHeadTimeout | 10 s | 初始请求头完成前的边界，具体协议能力见下文 |
| keepAliveIdleTimeout | 60 s | 没有活动请求时的连接空闲 |
| bodyReadIdle / responseWriteIdle | 30 s，可配置 | 有待处理 I/O 时的真实停滞 |
| requestDeadline | 默认关闭，路由可设置 | 从进入请求管线到响应执行结束；流式端点显式选择 |
| gracefulShutdownTimeout | 30 s | 允许已有请求自然完成的时间 |

全局容量检查在创建业务任务前执行，不让过载请求先进入认证或数据库。H1 拒绝后关闭连接；H2 仅处理相应 stream。能够在现有容量和发送约束内快速完成时可以发送 503，否则关闭/取消；容量拒绝不承诺总能返回 HTTP 响应。不会为“等待发送 503”再建立无界的拒绝任务队列。

requestBodyLimit 不能替代聚合上限；有界聚合也不能消除大量并发请求的总内存压力。关闭或增大流式路由的 body 总量上限是应用的显式选择。

当前超时能力必须准确描述：

- H1 readIdle 只在实际操作需要底层读取时计时，不自动表示整个请求头的总时限。
- H2 readIdle 只覆盖已有流的响应头/body 等待；服务端尚未 accept 的初始请求头不由它完整覆盖。
- H2 的另一条流和控制帧不应刷新当前流的读写停滞时间。
- 对连接简单套用“等待下一个 acceptRequest 超时”，不能正确区分合法空闲、其他活动流和慢速头部。
- H2 首部开始/完成及连接空闲的最小观测点属于需要验证并可能补齐的底层接口。尚未补齐时不得对外宣告已实现所有首部 deadline。
- 对整体请求的 deadline 到期执行取消；如果当前 I/O 已因取消关闭 H1 或 reset H2，就不能再补发一个超时响应。
- 取消是协作式的。长时间不挂起、不检查取消的业务计算不能由 server 保证在 deadline 瞬间结束。

每个请求在嵌套 coroutineScope 中执行，在其边界捕获与分类失败。普通 launch 的子任务失败会影响父任务，不能仅在连接的最外层捕获一次异常就认为已实现请求隔离。H2 一个业务请求失败不得意外取消其他 stream；H1 失败后是否保留连接由实际协议状态决定。

## 14. 停机与完成通知

server 生命周期使用运行、排空、已关闭三个必要状态。

优雅停机：

1. 关闭 listener，停止接受新 TCP 连接；已交付连接继续由 server 拥有。
2. H1 不再开始新的业务请求；活动响应按协议标记或状态决定结束后关闭，空闲连接直接关闭。
3. H2 发送 GOAWAY，last-stream 边界覆盖实际承诺处理的请求；处理边界内已交付或已排队的请求。
4. 保持连接驱动和接受 Job 的必要寿命，等待现有请求、结果执行和清理。
5. 宽限期届满后取消剩余请求，中止相应连接/stream，并等待真实本地清理完成。
6. 解除请求监听、计时器、continuation 和 buffer 引用，结束 server 所有者。

在接收、登记请求和开始排空之间要有明确的单一状态转换，保证一个请求只能进入“已接受并会处理”或“未接受并被拒绝”其中一条路径。

优雅停机的 30 秒是自然完成的宽限，不是可以忽略 native 借用和后台 I/O 收尾的硬返回承诺。close、并发 close 和取消清理复用现有 AsyncCloseable 契约；清理异常保留其真实原因。

shutdown 由应用显式触发。当前没有通用 OS signal 订阅 API 也能完整实现该设计，不以增加 signal 框架为前提。

提供一个非挂起的请求完成观察入口，参数为不可变 HttpRequestCompletion 快照，包含 requestId、route 模板、方法、最终状态、耗时、实际字节计数、完成/取消/失败类别。它在响应执行及请求清理后调用，不依赖仍有效的请求资源，也不宣称客户端已收到全部内容。

实际字节计数分别标明协议去定界后的接收字节、内容解码后的交付字节、响应原文和编码后交给 HTTP 的字节；没有运行相应包装时两侧可以相同。这些是观测值，不统计 TLS／TCP 开销，也不据此建立计费或通用预算。

SSE 属于活动 HTTP 请求，参与同一排空和 deadline。WebSocket 成功升级后释放 HTTP 请求名额，但仍占原连接名额并登记到 server；排空阻止新升级，向现有会话发出 1001 Close，握手等待受会话 closeTimeout 和剩余停机宽限共同约束。宽限耗尽后本地 close，并等待会话任务及真实 I/O 清理。HTTP 握手完成通知与 WebSocket 会话结束通知分开，见 15.3。

应用需要异步导出日志/指标时，自行把快照交给有界队列。普通日志不要记录完整凭证、Cookie 或 body；框架只提供必要事件，不预设 tracing/metrics 平台。

## 15. 后续能力及其落点

这七项都是可独立交付的普通库能力；以下 API 名称为设计建议。协议数据和字段语法继续在 scoop.http，字节流适配在 scoop.http.asyncio 或既有 scoop.compression，路由、请求所有权和响应策略在 scoop.http.server。server 不依赖 scoop.http.client 的内部实现。

| 能力 | 复用基础 | 本次规划的交付边界 |
| --- | --- | --- |
| SSE 输出 | SseDecoder／SseSource、流式响应 | 发送消息与 encoder／sink、server 结果和可选心跳，见 15.1 |
| Multipart 上传 | MultipartSource／MultipartBody、AsyncFile | 单遍 part 提取、局部限制、显式保存，见 15.2 |
| H1 WebSocket | 握手、takeTransport、AsyncWebSocketSession | 类型化升级结果、会话所有权、停机和完成通知，见 15.3 |
| 响应压缩 | compression 增量引擎和 AsyncCompressionSink | 字段协商、明确的响应执行策略及元数据处理，见 15.4 |
| 请求解压 | AsyncDecompressionSource、单次 body | 显式策略、双侧计数、分层解码与错误映射，见 15.5 |
| 静态文件、Range、条件请求 | 文件 metadata／seek、HTTP 字段工具 | 延迟文件结果、GET／HEAD、条件判断和单区间响应，见 15.6 |
| DTO query / typed extractor adapter | ValueDecoder、函数值、companion 条件 | 扁平 query decoder 和单一静态适配入口，见 15.7 |
| 服务端 mTLS / 动态 SNI | 尚无可直接满足本设计的完整接口 | 本次不展开；留在 TLS 库单独设计 |
| HTTP/3 | 不在现有 HTTP/TLS 接入基础内 | 本次不展开；独立后续工程 |

新增配置在安装或构造边界检查，依赖输入的规则在相应消费边界检查。底层已经解析的字段、消息或协商结果直接复用，不因加入高层便利 API 再完整重验一遍。

### 15.1 SSE 输出

**落点与 API。**在 scoop.http 增加发送侧 `SseMessage` 与增量 `SseEncoder`，在 scoop.http.asyncio 增加借用 AsyncByteSink 的 `SseSink`；server 提供 `sse(options, produce): HttpResult` 及只在 producer 期间有效的 `SseWriter`。接收侧 SseEvent 带派发时的 lastEventId，不能直接充当待发送消息。

| 拟议入口 | 契约 |
| --- | --- |
| `SseMessage(data, event, id, retryMilliseconds)` | data、event、id 为可选 String，retry 为可选非负 Long；None 与空字符串不同 |
| `SseEncoder` | 按有限输出块编码一个消息或注释，支持跨 UTF-8 字符的短输出，不复制整条事件为第二份完整字节数组 |
| `SseSink.send(message)` / `comment(text)` / `flush()` | 挂起写入；每方向至多一个应用操作，处理短写；借用下游，不结束 HTTP body |
| `SseWriter.send(message)` / `comment(text)` / `flush()` | server 包装，send／comment 默认写完后 flush，可显式批量发送后统一 flush |
| `sse(options, produce: suspend (SseWriter) -> Unit)` | 构造 Stream 结果；producer 在当前请求管线中执行，返回后框架 finish HTTP body |

发送侧固定 UTF-8 和 LF，不写 BOM。data 中 CRLF／CR 规范化为 LF，每个逻辑行输出一个 data 字段，保留结尾空行；Some("") 仍输出一个空 data 字段，None 不输出 data。event 拒绝 CR／LF；id 另外拒绝 NUL，Some("") 明确清除客户端 ID。retry 输出十进制毫秒，零合法。消息末尾输出空行；注释逐行加冒号，不派发业务事件。发送 API 对会改变字段结构的值直接报参数错误，不依赖接收端忽略非法字段。协议依据为 [HTML SSE 编码与解析规则](https://html.spec.whatwg.org/multipage/server-sent-events.html#parsing-an-event-stream)。

**HTTP 与生命周期。**默认结果为 200、`Content-Type: text/event-stream; charset=utf-8`、`Cache-Control: no-cache`，不设置固定 Content-Length；no-cache 不表示关闭所有中间代理缓冲。H1／H2 定界和 flush 交给现有 writer，不注入 Connection 字段或特定代理厂商字段。响应头提交并 flush 后才等待业务事件。HEAD 只发送元数据，不调用 producer、不启动心跳。204 停止重连等业务结果在返回 sse 之前由 handler 明确选择。

默认关闭动态响应压缩，避免压缩缓冲延迟事件；显式开启时，每次 flush 必须穿透压缩同步点再到 HTTP writer。SSE 不自动关闭应用已配置的 requestDeadline；端点需要长连接时显式选择期限。Last-Event-ID 按单值请求字段读取，重复项或无法按选定文本约定解释时为 400；ID 的意义、历史查询和是否补发由应用负责，不建立框架级事件存储或重放服务。

端点或 middleware 在最终响应提交前使用 `ctx.setDeadline(Duration?)` 覆盖本次请求期限；None 明确关闭，非负时长从进入本次请求管线时计起，不重置起点。不能恢复已经到期或取消的请求；连接与 I/O idle 仍独立生效。

心跳默认关闭，启用时配置正 Duration。计时器只在 producer 的结构化作用域中存活，空闲时发送注释并 flush。业务事件和心跳共用一个输出入口，按完整事件边界串行；已有写入或背压时跳过／合并本次心跳，不积累定时事件队列，不插入半个事件中。应用自身仍只允许一个发送操作。producer 返回、失败或取消时先结束心跳任务，再正常 finish 或中止响应；不能在 finally 中继续向失败连接补发事件。

资源在 producer 内取得并 useAsync 关闭。订阅源必须响应请求取消；H2 reset、已观察 I/O 故障和 server 停机会结束等待。H1 无网络活动时的断开观测仍受 12.2 限制，心跳只是提供写入机会，不承诺即时检测。

**验收。**使用实时进度订阅端点：覆盖多行／空 data、ID 清除、仅 retry／注释、短写、UTF-8、逐事件 flush；证明首个事件在 producer 结束前可读。再组合慢客户端、心跳与发送竞争、H2 单流 reset、停机、HEAD 及显式 gzip flush。编码有独立标准字节向量，互操作不能只靠本库 decoder 的往返测试。

### 15.2 Multipart 上传

**落点与 API。**`HttpRequestBody.multipart(options): RequestMultipart` 占用 body 唯一消费权；其内部借用已选择的请求流构造现有 MultipartSource。构造检查 Content-Type 和 boundary，不预读、不为每个文件创建临时文件。RequestMultipart 的 `nextPart()` 返回只读元数据和当前单次正文；正文完成或显式 discard 后才能继续。

| 拟议入口 | 契约 |
| --- | --- |
| `nextPart(): RequestPart?` | 以线上顺序交付，保留同名字段和同名文件；正常 outer EOF 后返回 None |
| `part.head` / `nameBytes` / `filenameBytes` | 保留原字段和参数字节；form-data 的 disposition／name 必须有效，filename 可以缺失或为空 |
| `part.openStream()` | 取得受同一 part 上限约束的 AsyncByteSource 视图，用于应用自己的流式处理 |
| `part.text(maxBytes, charset)` | 有界聚合本 part，默认严格 UTF-8；只消费一次，不推断业务 DTO |
| `part.copyTo(sink, maxBytes)` | 有限缓冲、短写和背压，借用应用提供的 AsyncByteSink；返回实际复制量 |
| `part.discard(maxBytes)` | 显式有界跳过，复用 MultipartBody.discard 的超限与关闭规则 |
| `RequestMultipart.close()` | 释放本地解码视图；请求底层的放弃和最终关闭由请求执行器统一协调 |

每个 part 选择 openStream、text 或 copyTo 之一，不创建隐藏缓存；discard 可以明确放弃该 part 尚未消费的余下正文。关闭未完成 part 终止整个 multipart 视图，不静默跳到下一项。已取得的 part 流不能在推进到下一个 part 后继续读取。

表单便利层只解释 multipart/form-data；其他 multipart 使用既有通用 MultipartSource。复用 Content-Disposition 参数解析，但不把下载字段的 filename 编码猜测套到表单上。String 名称／文件名读取采用明确的严格 UTF-8 约定，字节元数据始终可得；不自动百分号解码、不生成 filename*。重复字段保持顺序，不以一个字符串 Map 静默覆盖。没有 filename 不证明该 part 是可聚合的文本，应用显式选择读取方式。相关格式依据为 [RFC 7578](https://www.rfc-editor.org/rfc/rfc7578.html)，底层边界规则沿[现有 multipart 规范](../specs/SCOOP-HTTP-MULTIPART-SPEC.md)。

文本 helper 支持调用方显式 charset，以及 part 上有效的 charset 参数；两者冲突时报提取错误。未指定时使用严格 UTF-8；`_charset_` 作为普通字段交付，应用可以在读到它后显式选择后续文本的字符集，不回头缓冲或重解码之前的 part。嵌套 multipart 保持普通 part 正文，只有应用显式组合第二个 reader 才解析；不自动展开旧式 multipart/mixed。

**大小与错误。**复用 MultipartLimits 的头部、字段数、part 数和分块上限。server 另建议 `maxFieldBytes = 64 KiB`、`maxPartBytes = 8 MiB`；requestBodyLimit 继续限制整个实际输入，启用解压时另受 15.5 的总明文限制。text 的有效上限为调用上限、字段上限和 part 上限中的最小值。copyTo／discard 同样经过 part 计数，不能通过换入口绕过限制。header、boundary、preamble、epilogue 仍计入外层 body；part Content-Length 不改变 boundary 定界，也不预先分配完整文件。

不匹配的媒体类型为 415；缺失 boundary、非法 disposition／name、截断和格式错误为 400；实际字段／part／总量超限为 413。只转换来自本次输入的错误，文件打开／写入失败和应用异常保留真实原因。reader 提前关闭不自动 drain；H1 在仍能发送错误结果时先响应再关闭连接，H2 先结束响应再放弃输入，遵守 9.3。

**保存文件。**应用选择目标 Path，打开 AsyncFile 并借用给 copyTo；客户端 filename 不参与目标路径拼接。需要失败不发布时，应用显式 createTempFile 到自己选择的目录，读完所有 part 及外层 EOF、确认成功后关闭文件并 moveFile；失败和取消由同一 useAsync／finally 关闭并删除本次临时文件。TemporaryFile.close 只关句柄，不会替应用删除已交付路径；moveFile 也不隐式跨文件系统复制。需要持久化时显式 sync。不增加上传存储插件、目录来源审计或后台清理服务。

**验收。**用含普通字段、重复附件和大文件的上传端点，覆盖任意分块、空字段／空文件名、缺失 filename、文本错误、未知长度超限、未读 part 推进、有限 discard、磁盘失败和取消。组合 gzip multipart、H2 一个上传被 reset 而另一流正常、H1 提前认证拒绝，以及完整读取前不发布临时文件。普通 API 行为放 server／HTTP 所属 Cone 测试。

### 15.3 H1 WebSocket 升级与会话

**落点与 API。**提供拟议 `webSocket(options, onSession): HttpResult` 升级结果，onSession 为 `suspend (WebSocketContext, AsyncWebSocketSession) -> Unit`。WebSocketContext 只保存 requestId、route 模板、连接只读信息和实际选定子协议等会话快照。需要 principal／tenant 时由 handler 先复制业务值并在会话函数中显式绑定；不传入可继续写 HTTP 的 HttpContext。

使用原有 WebSocketHandshake.parse／response、H1 takeTransport 及 AsyncWebSocketSession.takeOwnership，不在 server 重写握手或帧引擎。普通请求仍走路由和认证；只有端点明确返回升级结果才进入此分支。可在 GET handler 发现 HEAD 时返回普通结果；HEAD 的 GET 回退绝不执行升级，若仍返回升级结果则按方法拒绝，给出 405 和仅允许实际升级方法的 Allow。

options 包含有序的子协议选择、是否要求选中子协议、显式 Origin 判定、既有 WebSocketSessionOptions 和可选 permessage-deflate 策略。Origin 默认不另作限制；浏览器会话站点应在端点配置允许来源，它不替代凭据验证。缺少 Origin 和非法 Origin 的处理必须由该策略明确，不能把缺失头当作已经可信。默认不启用压缩，启用时只选择已有后端可实际支持的提议；不接受任意扩展插件。

**握手与所有权按下列顺序完成。**

1. 校验实际入口是 H1.1 GET、握手头和请求 body 已定界完成；这一步不读 WebSocket 帧。认证和 Origin／子协议拒绝仍可返回普通 HTTP 结果。格式错误为 400；仅版本不支持时为 426 并给出支持版本 13；若底层异常尚不能区分这一原因，应在握手工具补最小的类型化原因，不解析异常文本。H2 不进入本升级分支，也不宣告 Extended CONNECT 支持；已交付的未配置 CONNECT 按 501 处理，协议层先拒绝的请求按实际故障收尾，不发送 H1 的 101。
2. 在 server 状态锁内检查仍允许运行并预留会话名额；配置合法性检查在 101 前完成。拟议 `maxWebSocketSessions` 默认跟随 maxConnections，可显式调小；它只限制实际活动／已预留会话，不建立另一个等待队列。
3. 在普通字段草稿和 beforeCommit 处理后生成本次有效握手头；Cookie 等附加字段可以保留，Sec-WebSocket-*、Upgrade、Connection 和禁止的 body 定界字段由握手结果决定，冲突修改报程序错误。完成 101 及 flush 后调用 takeTransport，连同同次读取的全部后缀转交连接所有者。发送开始后失败不能再返回一个 HTTP 错误。
4. 连接所有者登记“已升级、待开始”的实际 transport 和 session 函数；旧 H1 停止处理，关闭旧 wrapper 不重复关传输。此时 HTTP next() 返回，中间件和请求资源完成清理，记录一次握手完成快照；请求名额直到清理结束才释放。
5. 由创建于请求 Context 之外的连接任务，在 HTTP 管线退出后建立独立会话子作用域，调用 AsyncWebSocketSession.takeOwnership，再执行 onSession。工厂接管前失败由连接所有者关闭 transport，接管后由 session 负责；任何路径都释放预留名额。禁止从仍绑定请求 Context 的任务直接 launch 会话，以免继承已经关闭的请求对象。
6. HTTP 收尾失败或停机在第 3～5 步获胜时，不启动业务会话，关闭已拥有的 transport 并完成登记清理。会话开始和排空检查使用同一登记边界，不出现 server 不知道的已升级连接。

每个握手 helper 的结果在所属边界复用；只有追加字段可能改变了实际输出时才验证新增冲突，不完整重放已解析的握手。额外来源依据为 [RFC 6455 服务端握手](https://www.rfc-editor.org/rfc/rfc6455.html#section-4.2)，传输接管沿[现有异步会话规范](../specs/SCOOP-HTTP-WEBSOCKET-ASYNC-SPEC.md)。

**会话完成。**onSession 正常返回时发起 1000 Close 并等待有限关闭；应用失败在传输允许时发 1011，server 排空发 1001，底层协议失败继续保留其原始 close code。不能发送 Close 时直接本地 close。对端主动关闭使用 awaitClosed 观察并取消仍等待外部服务的会话业务子任务；内部正常结束取消与业务真实异常分开。关闭观察在业务子任务外执行，避免来源发送或会话任务等待自身关闭。

请求的 AuthContext／数据库资源不会自动变成会话资源。onSession 必须从共享 pool 获取自己的 session 或按消息获取资源；不得捕获已关闭的请求 body、response、DB session。会话可正常复用应用服务对象。握手 deadline 到交接为止；长会话使用显式会话策略和既有关闭期限，不继承短 HTTP 请求 deadline。

HTTP 完成快照把 101 记为协议升级，耗时覆盖握手和 HTTP 清理；另提供非挂起的 HttpWebSocketCompletion 快照，记录关联 requestId、会话耗时、关闭码和失败／取消原因。它在 onSession、session 和原生资源真正收尾后通知，不向旧 HttpContext 写回状态。已有 maxConnections 直到该连接结束才释放。

**验收。**用显式认证的消息会话：检查普通拒绝、Origin、子协议和版本错误、101 与首帧同批输入、交接失败只关闭一次、HTTP 请求资源先于会话函数释放、请求 Context 不泄漏。再覆盖 ping／pong、片段消息、双向背压、permessage-deflate、对端关闭取消外部等待、升级与停机竞争，以及一条会话失败不影响其他连接。H2 WebSocket 不计入本项交付。

### 15.4 响应压缩

**落点与安装。**提供拟议 `responseCompression(options): HttpMiddleware`。它在调用 next() 前安装本请求的压缩策略，结果执行器在 7.3 的表示选择阶段读取策略并借用 HttpBodyWriter 包装现有 AsyncCompressionSink；中间件退出时恢复外层策略。绝不在 next() 返回后尝试压缩已经发送的正文，也不增加任意结果拦截器／编码插件注册表。路由内层策略可覆盖外层，结果可明确禁止压缩。

现有 HTTP 字段工具没有完整的服务端 Accept-Encoding 质量值协商入口，先在 scoop.http 补小型 `HttpAcceptEncoding` 解析／选择工具。它保留缺失字段与空字段的区别，合并允许合并的多行字段，按 ASCII 大小写无关 token、`q` 的 0～1 和至多三位小数处理；未知编码保留为协商输入，不尝试加载实现。重复同一编码的质量值采用最低值，是本库明确的消歧策略；语法错误在使用该工具的端点返回 400。相关规则以 [RFC 9110 §12.5.3](https://www.rfc-editor.org/rfc/rfc9110.html#section-12.5.3) 为依据。

**选择规则。**默认只启用 gzip；可显式配置 gzip、deflate、br、zstd 及各自已有压缩选项和偏好次序。HTTP deflate 固定映射到 CompressionFormat.Zlib，不使用 RawDeflate。候选先按客户端质量排序，同质量按服务端配置次序；显式 identity 同样参与偏好，未显式列出的 identity 作为允许时的回退。具体编码项覆盖星号；q=0 的候选从不选择。缺失 Accept-Encoding 时本库选择 identity，空字段只选择 identity。无可接受表示时返回不带正文的 406，不递归为该错误再运行压缩协商。

默认只对明确的文本、JSON／+json、XML／+xml 等可压缩媒体类型启用，不读取正文做内容猜测。已知原文小于拟议 `minimumBytes = 1024` 时优先 identity；若 identity 被禁止且仍有合法编码，则允许越过这一性能阈值。未知长度 Stream 按媒体类型和显式策略直接选择，不为判断长度预读或缓存 producer。

以下表示不做动态变换：已有 Content-Encoding、已选定的文件／Range 表示、明确禁止变换的结果，以及本库选择遵守的 Cache-Control: no-transform。SSE 默认禁用，可按 15.1 显式开启。已有编码仍须满足请求协商；若这些限制导致没有可接受表示，同样走 406，而不是偷发被排除的 identity。升级和 204 等不提供表示内容的结果直接跳过内容协商；304 只保留先前选定表示的元数据，不重新变成 406。no-transform 是这里主动采用的应用策略，不把 origin server 与代理的职责混为一谈。

**字段与执行。**

- 结果可能随 Accept-Encoding 选择变化时，合并 `Vary: Accept-Encoding`，包括最终选择 identity 的正常响应；已有 Vary 项去重但保持语义，`Vary: *` 不被缩窄。
- 启用压缩后设置准确 Content-Encoding，移除原文 Content-Length；基础实现流式生成编码字节，不为得到压缩长度完整执行 producer。原文声明长度仍在压缩器输入侧检查，HTTP writer 检查实际编码后的发送契约，两者不能混用。
- 默认把可用于语义等价内容的强 ETag 弱化，保留既有弱 ETag；应用要求强标签时应显式提供对应编码表示，或禁用动态压缩。与原文字节绑定的摘要字段不能直接沿用，默认不携带失效的摘要。文件结果已自行选定表示，不经过此弱化流程。
- HEAD 使用与 GET 相同的协商和媒体类型判断，保留应有的 Content-Encoding／Vary，无法知道编码后长度时省略它；不运行 producer 或压缩器来测长。304 等响应没有编码器和输出，但保留所选表示允许的元数据。
- Stream 原文写入借用的 AsyncCompressionSink；flush 先完成压缩同步点，再 flush HTTP writer。正常顺序是 producer 完成、压缩 finish、HTTP trailers／finish、释放包装器。失败只收尾，不在 finally 中补完残缺压缩流；不重试已接受的原文。

通用压缩 middleware 不自动赋予所有 handler 条件请求语义。应用要判断 ETag 时，必须先选择实际表示或禁用动态压缩，不能在原文上完成强比较后再发送另一个编码变体。静态文件的完整顺序由 15.6 固定。

**验收。**用文本／JSON 端点验证缺失、空、星号、重复编码、q=0、identity 排除、未知编码和 406；覆盖 Vary 合并、HEAD、已编码正文、小正文阈值、原文长度错误、强 ETag 处理。流式端点证明压缩后仍有短写背压和中途 flush，分别组合 SSE、H1 chunked、H2 reset 与正常 finish。复用压缩库格式向量，不在 server 重测整套算法。

### 15.5 请求解压

**落点与 API。**提供拟议 `requestDecompression(options): HttpMiddleware`，只在 next() 前配置本请求 HttpRequestBody 的内容解码策略，不读 body、不发 100 Continue。认证需要在读 body 前完成时放在它或提取器之前。策略必须在取得任何消费入口之前确定；后续试图改变活动 reader 的解码方式是程序错误。

安装后 openStream／bytes／text／json／form／multipart 使用解码后的视图；新增 `openRawStream()` 显式读取编码后的表示字节。两个入口仍竞争同一消费权，没有可同时读取的“原文副本”。未安装策略时 openStream 保持原始流，结构化提取器遇到未支持的 Content-Encoding 按 9.1 返回 415。原始请求 head 不被改写；body 视图另提供所选解码信息，解码后未知的长度为 None，绝不把压缩 Content-Length 冒充明文长度。

Content-Encoding 的 token 语法复用 parseHttpTokenFields；gzip、deflate、br、zstd 映射到已有格式，HTTP deflate 只用 zlib 包装。按字段列出的相反顺序逐层借用 AsyncDecompressionSource，先验证整条编码链再开始输入。默认允许四种算法、最多两层实际编码；identity 不增加解码器。未知／未启用编码和超过该局部层数策略的请求为 415，可给出 Accept-Encoding 列出可接受编码；不进行原始 DEFLATE 回退或隐式字典下载。

**实际输入限制。**

| 限制 | 默认建议与计数位置 |
| --- | --- |
| requestBodyLimit | 仍为 8 MiB；计 HTTP 去定界后、内容解码前的实际输入 |
| decodedBodyLimit | 8 MiB，可按路由覆盖；限制最终明文，并作为每个中间解码层的 maxOutputBytes，防止中间层先膨胀 |
| bufferedBodyLimit | 仍为 2 MiB；聚合 bytes／text／json 时另外执行，以实际交付明文计算 |
| 解码缓冲 | 复用现有 8192 字节默认值；各层有限推进，下游慢时不继续预取 |
| zstd 最大窗口 | 默认建议 8 MiB，可显式增大；只作用于 zstd 实际窗口，不能给其他格式传无意义选项 |

这些限制分别约束网络正文、已产生数据、聚合结果和后端窗口；不是跨请求的成本额度。所有计数检查整数溢出。读取上限不能只按下游已取走的明文计算，已经由解码器产生的中间数据也必须受其自身上限约束。关闭某项总量限制不自动关闭聚合或窗口限制。

已声明原文长度超过线上上限时可在 100 Continue 前拒绝；未知长度和压缩膨胀在实际推进时检查。格式损坏、截断、校验和错误以及不被该格式允许的尾随数据为输入 400；实际输出／窗口上限为 413。gzip 连续 member、zstd 连续 frame 等沿[压缩库规范](../specs/SCOOP-COMPRESSION-SPEC.md)处理，不把一个编码层内的连续 frame 误当成多层 Content-Encoding。底层 I/O、取消、内存失败和后端故障保留实际分类，不统一转成客户端错误。

**结束与资源。**解压器可能先交付部分明文，最后才发现 checksum／EOF 错误。需要完整上传成功后发布文件或提交业务的 handler 必须读到正常 EOF，不能在拿到第一个 JSON 值或一个 multipart part 后宣告整个上传有效。完整 JSON 聚合自然读到 EOF；multipart 的 nextPart 返回 None 也必须来自外层 EOF 验证。

包装层借用请求 body，框架关闭顺序是停止读取、等待实际 I/O／借用结束、从外到内释放解码器，再按 9.3 处理未完成 HTTP 输入。尚可发送 400／413／415 时不先关闭会导致 H1 连接消失的底层 body。已经提交响应时只中止实际连接／stream，不能再追加错误响应；不为了连接复用自动读完剩余压缩内容。

**验收。**用 gzip JSON 和压缩 multipart 上传验证两种上限、双层顺序、非法编码链、空输入、连续 member、尾随字节和尾部 checksum 失败；确认 raw／decoded 二选一、原 head 保真、提前拒绝不发 100。组合慢上传、取消、H2 流隔离和临时文件不发布，验证总量计数不会因更换包装层遗漏。

### 15.6 静态文件、Range 与条件请求

**结果与挂载。**提供拟议 `file(path, options): HttpResult`，内部为 HttpBody.File；另提供 `staticFiles(root, options): HttpHandler`，供应用显式挂到一个末尾通配路由。Router 不自动发现目录、注册资源或扫描文件，目录列表和默认首页均默认关闭。默认首页只在应用明确配置固定名称后尝试；文件媒体类型使用显式配置或有限的扩展名表，未知类型为 application/octet-stream，不读内容猜格式。

file 构造只保存 Path 和不可变选项。响应执行时打开 AsyncFile，随后从同一句柄读取 metadata 并准备表示长度、Last-Modified 和区间；GET 在提交头后有限复制，HEAD 只获取必要元数据不读正文，304／412／416 和所有失败路径都关闭该句柄。不会在 handler 返回前打开文件、把其关闭责任藏进一个可能被丢弃的 producer。缺失文件映射 404，普通访问拒绝可映射 403，其他文件错误走真实服务端失败；不把所有 OS 错误伪装成 404。

staticFiles 只使用第 8 节已解码一次的路由片段：拒绝点片段、解码后包含斜线／反斜线、NUL、绝对路径或驱动器前缀等不能作为单个相对文件名的输入，畸形映射返回 400。按片段连接到应用选定 root，不再次 URL 解码、不按字符串公共前缀判断路径归属。末尾通配不匹配空后缀，挂载根／首页需另外显式注册。路径映射只是正常输入选择；不审计 owner、权限位、inode 或 symlink 来源，文件链接沿正常 OS I/O 语义，不承诺 root 是文件系统隔离沙箱。

**表示与验证器。**首个交付默认发送 identity 文件并禁用其动态响应压缩，保证 Range、长度和验证器针对同一份字节。应用也可显式提供已编码文件路径及对应 Content-Encoding；其长度和强标签针对该实际编码文件，发送前须通过同一 Accept-Encoding 工具确认可接受。不自动探测 `.gz`／`.br` 旁文件，不运行时生成或缓存压缩副本。应用选择多个变体时明确合并 Vary。

默认生成 Last-Modified，精度按 HTTP 日期截到秒；默认不凭大小和 mtime 构造强 ETag，也不每次读取全文件求摘要。应用可通过选项提供与当前表示一致的 HttpEntityTag，负责在内容变化时更新。没有可靠标签时就只使用已有时间元数据，不承诺一个并不存在的强验证器。文件可能被其他进程修改；本库不锁文件、不做来源审计或重复内容复核。发送期间截短导致长度不足时中止，增长时最多发送准备时确定的范围；需稳定表示的应用使用不可变文件或原子替换发布。

**条件判断。**先完成路由、认证和文件存在／可读等正常检查，再对本次选定表示评估；不会让条件头把本应返回的 403／404 改成 304。新增的小型条件求值 helper 放在 scoop.http，只接收方法、当前表示是否存在、可选 ETag／修改时间和请求字段，返回继续／未修改／前置条件失败，不读取文件或缓存。

求值顺序固定为 If-Match；仅在没有 If-Match 时判断 If-Unmodified-Since；然后 If-None-Match；仅在没有 If-None-Match 且方法为 GET／HEAD 时判断 If-Modified-Since。标签匹配复用既有强／弱比较，星号按表示是否存在判断，不以“有没有 ETag”代替。失败产生 412，GET／HEAD 的未修改产生 304；其他方法的 If-None-Match 命中为 412。日期无效按 HTTP 规则忽略，格式非法的标签列表按本库提取策略返回 400。日期接收参考时间显式来自请求处理时钟。规范顺序见 [RFC 9110 §13.2](https://www.rfc-editor.org/rfc/rfc9110.html#section-13.2)。

只有结果仍为 200 的 GET 才继续考虑 Range；HEAD 忽略 Range，仍执行条件判断。If-Range 的强标签必须与所选表示强匹配，弱标签不满足；日期须能作为该表示的强验证器且精确相等，不使用“小于等于”。普通文件 metadata 不能保证同一秒内没有变化时不把其时间升级为强验证器；此类日期 If-Range 回退完整 200。不存在 Range 时忽略 If-Range。

**首个区间交付。**

| 输入／状态 | 行为 |
| --- | --- |
| 单个有效可满足 bytes 区间 | 206；正确 Content-Range 和区间长度，用同一句柄 seek 后限量复制 |
| 单个有效但不可满足区间 | 416；`Content-Range: bytes */完整长度`，空正文 |
| 未知单位、非法 Range、重复 Range 字段或多区间 | 本版明确忽略 Range，发送完整 200；不只取第一段，不误报为不可满足 |
| If-Range 不匹配或无可用强验证器 | 忽略 Range，发送完整 200 |
| 条件已产生 304／412 | 不再处理 Range；304 无 body 并保留允许的 ETag、Last-Modified、Vary 和缓存字段 |
| 空文件 | 本版忽略 Range，返回长度为零的 200，避免用空的 resolve 列表错误推导 416 |

开放终点、suffix、截断到文件末尾和溢出检查复用 HttpRange.resolve／HttpContentRange。正常文件 GET／HEAD 可以发送 `Accept-Ranges: bytes`，不表示本版承诺多区间输出。单区间是完整且有明确回退的交付范围；以后真有多区间使用需求时可组合 MultipartSink，但本次不新增 multipart/byteranges 输出。协议允许的忽略规则见 [RFC 9110 §14.2](https://www.rfc-editor.org/rfc/rfc9110.html#section-14.2)。

**验收。**用下载端点验证 GET／HEAD、文件缺失、访问失败、编码点片段和分隔符；核对精确区间字节、开放／suffix、416、空文件、多区间回退。覆盖条件优先级、弱／强标签、标签缺失但星号命中、无效日期、If-Range 回退、304 不读 body，以及已编码文件的实际长度。组合慢客户端下载中取消、文件截短、H2 并发文件句柄独立、HEAD／304／错误替换不遗留句柄；不以删除源码或工具测试缓存。

### 15.7 DTO query 与类型化提取适配器

**Query decoder。**提供拟议 `ctx.request.query.decode(codec, options): T`，并保留基于 `T.Companion : ValueDecoder<T>` 的普通泛型便利入口。decoder 放在 server 的提取模块，复用 scoop.url 的有序 query，向既有 ValueDecoder 提供 Decoder／KeyedDecodingContainer／UnkeyedDecodingContainer；不先拼成 JSON，不为 DTO 引入反射、注解扫描或新的构造器协议。

入口形状为扁平 named record。顶层字段按已解码的严格 UTF-8 名称分组，保留出现顺序和原始位置；标量读取与列表读取由所选 codec 的容器操作决定，不按名称猜类型。

| Query 情况 | 解码约定 |
| --- | --- |
| 字段缺失 | required 报缺失；optional 返回 None，默认值／Option 由现有 DTO codec 决定 |
| `name=` | 存在且值为空；String 接收空串，数值／Boolean 转换按实际规则失败 |
| `name` 无等号 | 原始 API 保留区别；DTO 标量／列表元素读取报缺值，flag 语义由应用显式提取 |
| 重复字段 | 标量拒绝；codec 请求 unkeyed 时按收到顺序交付全部项，单项也可成为长度一的列表 |
| 列表缺失或 `name=` | 缺失仍交给 optional/default；空字符串不是自动的空列表 |
| Boolean／整数／浮点 | Boolean 仅 true／false；整数和有限十进制浮点使用明确文本语法及范围检查，不裁掉空白、不把空值改成零 |
| `null` | 普通字符串，不是 Option.None；query 没有隐式 null 值 |
| `+`、百分号和 UTF-8 | 沿普通 URI query 规则，加号不变空格，解码一次，畸形编码为 400 |
| 未知字段 | 默认 Reject；可显式 Ignore，keyed.end 检查实际未读取的字段 |
| `a.b`、`a[0]`、逗号值 | 都是普通字段名或值，不形成嵌套记录、数组下标或隐式列表 |

容器记录当前字段的实际读取情况；不会因为应用枚举了 keys 就把所有字段当作已消费。每个有值的字段 decoder 允许 codec 选择单值或序列，单值只有恰好一个项时成立；不能为读一个列表额外消耗 HTTP body。嵌套 keyed、嵌套列表和不受支持的 codec 形状属于服务端装配错误，使用独立的 QueryCodecException 并映射 500，不伪装成客户端 400。自定义标量 codec 可复用同一个文本 decoder，例如把一个字符串解析为业务 Identifier。

格式／数值／重复标量等提取失败包含 query 字段名与具体项位置，转换成 HttpRejection；内部仍保留原始 DecodingException 的路径。不把 handler 随后抛出的解码异常或任意业务异常一并归为输入错误。普通 decoder 对未知字段、optional 和 end 的规则在一个边界实现，不重复做一套 DTO 语义校验。

**静态适配器。**使用函数类型和一个普通泛型函数即可，不为不同参数数量生成一族 Handler 接口：

~~~scoop
public typealias HttpExtractor<T> = suspend (HttpContext) -> T

public fun <T> adapt(
    extract: HttpExtractor<T>,
    handle: suspend (HttpContext, T) -> HttpResult
): HttpHandler = suspend { ctx ->
    val value = extract(ctx)
    handle(ctx, value)
}
~~~

拟议 queryExtractor(codec, options) 和 jsonExtractor(codec, maxBytes) 只是构造上述函数值，分别复用 query.decode 和 body.json；实例依赖仍通过闭包／构造参数传递。需要 path、query 和 JSON 组合时，应用显式编写一个 extractor 按所需次序提取，返回自己的参数 DTO 或 tuple，再适配一个 handler。默认按先校验元数据、再消费 body 的次序执行；失败不调用 handle，不并发争用 body。

资源型输入不借此延长寿命：AsyncFile、上传 part、DB session 应在明确的 useAsync／middleware 作用域中使用，不藏在一个带自动析构的 extractor 返回值里。静态适配不会提前验证业务 Context 已绑定，缺失仍是装配错误；错误结果和流式结果继续在第 7 节的完整请求管线内执行。没有泛型虚方法、DI 查找、参数名注入或隐式类型转换注册。

**验收。**用分页／筛选 DTO 和带 JSON body 的端点覆盖显式／companion codec、默认值、Option、重复标量／列表、未知字段、裸 key、空值、数值边界、Unicode 和编码等价字段名。适配器验证提取失败不调用 handler、两个 body 提取仍报单次消费错误、上下文和流式响应寿命不变。非法泛型／codec 声明只补必要的编译诊断用例，不为每个提取来源复制同一语言规则 fixture。

## 16. 实现拆分与验收

### 阶段 A：请求分派核心

实现 RouterBuilder、HttpApplication、HttpContext、HttpResult、基本字节结果和中间件执行器。使用内存请求输入验证语义，不需要先建立 socket。

完成标准：

- 路由优先级、冲突、HEAD、404/405/OPTIONS 语义固定。
- next() 顺序、提前响应、重复调用和响应提交行为完整。
- 构造函数依赖与两个应用实例隔离可验证。
- Context 在挂起、异常及流式结果执行期间保持正确。
- 缺少业务 Context 与认证拒绝产生不同结果。

### 阶段 B：H1 与 TLS

接入 TCP listener、H1 connection、单次 body、Cookie、query、JSON、流式结果和 TLS/ALPN。

完成标准：

- 一条 keep-alive 连接连续处理多个请求，各请求数据不串。
- 提前拒绝 body、短读/短写、HEAD、已提交后失败和连接收尾正确。
- 响应 producer 使用的请求资源直到完成才关闭。
- TLS 握手互不阻塞接受循环；接管前后失败的所有权明确。
- 基础限额、可实现的超时和主动停机完成真实网络验证。

### 阶段 C：H2 与服务端生命周期补齐

实现每流任务、请求隔离、stream 失败传播、GOAWAY 排空和全局容量控制。先完成首部 deadline、空闲判断和接受 Job 寿命的最小原型，必要时修订 HTTP 异步层规范及接口。

完成标准：

- 并发 stream 的 AuthContext、body、Cookie 与结果互不混用。
- 单流取消及时到达等待外部工作的 handler，其他 stream 继续运行。
- 慢读写与共享流控不会形成只等输出、无法接收 WINDOW_UPDATE 的死锁。
- GOAWAY 不误取消已接受请求，登记/取消竞争不遗漏清理。
- 不以已有 readIdle 冒充尚未实现的头部总时限。
- 正常结束不无条件发送 CANCEL。

### 阶段 D：独立便利能力

按下表逐项交付；依赖列是能力依赖，不要求简单的 D1 → D7 串行。每项先修订所属规范，再实现一个真实应用闭环，不先公开只能报“暂不支持”的空 API。新 server 公共契约在实施阶段写入 `docs/specs/SCOOP-HTTP-SERVER-SPEC.md`，并由 HTTP／标准库规范建立正常引用；本次不把尚未实现的提案登记成既有 API。

| 增量 | 前置与规范落点 | 交付闭环 |
| --- | --- | --- |
| D1：SSE 输出 | B 的流式执行；补 HTTP-MESSAGE-SPEC 的发送侧、HTTP 异步适配合同及 server spec | 内存 encoder → 借用 sink → SSE 端点；首事件可提前读取、心跳停止、取消与 HEAD 正确 |
| D2：Multipart 上传 | B 的单次 body 和现有文件 API；server spec，只有底层发现实际缺口才改 MULTIPART-SPEC | 表单／大文件逐 part 读取到 EOF → 显式发布临时文件；限制、失败和取消不发布 |
| D3：DTO 与提取适配 | A 可验证适配；接 B 的 query／JSON，写 server spec | 分页 DTO 和 JSON 端点各一例；重复／缺失规则明确，不读两次 body，不新增编译器特例 |
| D4：响应压缩 | B；补 HTTP-MESSAGE-SPEC 的 Accept-Encoding 工具及 server spec，复用 COMPRESSION-SPEC | JSON／Stream 协商 → gzip／其他显式算法 → 正常 finish；HEAD、Vary、406 和长度语义正确 |
| D5：请求解压 | B；server spec 规定输入视图、编码链和上限，复用 HTTP／COMPRESSION-SPEC | 压缩 JSON 正常解码，坏尾部和膨胀正确失败；与 D2 组合压缩上传 |
| D6：文件结果与条件请求 | B 的结果执行；复用 D4 的协商工具，补 HTTP-MESSAGE-SPEC 的纯条件 helper 及 server spec | GET／HEAD → 条件短路／单 Range → 同句柄有限发送；无动态压缩改写区间 |
| D7：H1 WebSocket | B 及 14 节 server 登记／排空；server spec，必要时补 WEBSOCKET-SPEC 的类型化失败原因 | 明确升级 → 前缀交接 → HTTP 清理 → 独立会话 → 关闭；交接与停机竞争全部收尾 |

B 可以先完成 H1 纵向闭环；C 完成后，D1～D6 中协议无关的能力都补 H2 组合验证，D7 仅 H1／WSS。D4 与 D1 只在显式开启 SSE 压缩的组合中互为验收依赖；D5 不以完整 multipart 上传作为基础解码器的前置。D6 首个范围固定为单区间及多区间 200 回退，不等待新的 multipart 输出形态。

建议的源码归属如下，名称随实际文件大小拆分，不引入新的插件 Cone：

- `scoop.http`：在已有 sse 模块增加发送模型／encoder；消息工具中增加 Accept-Encoding 和条件求值；握手原因如有缺口在 websocket 模块补齐。
- `scoop.http.asyncio`：在已有 sse 模块增加 sink；multipart、H1 handoff 与 WebSocket session 复用现有公开接口，只有原型证实必要时才修改。
- `scoop.http.server`：基础 router／pipeline／request／response／server；增量按 sse、multipart、websocket、compression、files、extract 模块组织。SSE、文件和压缩不会各自复制一套响应执行器。
- `scoop.compression`、`scoop.asyncio`、core：复用其现有合同；HTTP 策略不进入压缩器、文件系统库或 runtime。

以上阶段是实现建议，不占用或改写仓库既有里程碑编号。本次只交付 `docs/http-server/DESIGN.md` 的完整设计，不修改语言语义、runtime 契约或已发布库行为。

## 17. 测试与性能验证

普通行为放在所属 Cone 的 scoop.test 测试中。编译失败、诊断、ABI、CLI、必须由外部进程控制的互操作等，才使用相应 fixture。不能为每个 HTTP 用例重复验收相同的构建缓存、产物或底层 parser 规则。

必要组合：

- H1 两次请求和 H2 并发请求的全局共享、请求隔离、同型 Context 重绑定。
- handler / producer 挂起期间 Context 存在，真正退出后恢复。
- 接口 key 与具体 class key 不同；缺失绑定按装配失败处理。
- Cookie 重复、删除属性、多个 Set-Cookie、响应修改不影响请求。
- 单次 body、聚合上限、未知长度超限、100-continue 与提前认证拒绝。
- 路由百分号、编码斜线、非法 UTF-8、query 加号、重复标量及尾斜线。
- middleware 前后顺序、提前响应、next() 两次、提交后改 header。
- 流式资源覆盖响应执行、HEAD 不启动 producer、错误结果替换和清理失败。
- H1 parser 失败、H2 reset、输出半途失败不产生第二个最终响应。
- 多请求隔离、已观测连接故障及 H2 reset 向 DB 等待传播取消、正常 TCP 半关闭、关闭等待真实 I/O 结束。
- 停机与 accept、请求登记、GOAWAY、排队 stream 的竞争。
- 请求容量、流控和慢客户端下的有界队列行为。

第 15 节逐项验收之外，只增加确有交互的组合：

| 组合 | 需要证明的边界 |
| --- | --- |
| SSE + 压缩 + H2 reset | 完整事件 flush 穿透编码层；reset 结束订阅和心跳，不影响其他流 |
| Multipart + 请求解压 + 文件保存 | 编码／明文／part 上限分别生效；坏 checksum／取消不发布临时文件 |
| 文件 + 条件请求 + Range + 编码 | 先选表示再判断，304／HEAD 不读正文，区间偏移与实际字节一致 |
| Query／JSON 适配 + 认证 Context + 流式响应 | 元数据检查先于 body；提取和结果执行仍在同一请求资源作用域 |
| WebSocket + HTTP 中间件 + 停机 | 101、请求清理和会话开始顺序明确，交接窗口内资源始终有唯一所有者 |

新增测试按实际能力归属，不为七项各跑一套冷／热 sysroot 构建；复用现有 parser、压缩格式和帧引擎测试。SSE 和 WebSocket 的标准向量／独立对端互操作补在所属库，server 只验收自身组合行为。

内存级分派测试与真实 loopback TCP/TLS/H2 测试都需要。真实 socket 测试使用动态端口。底层协议互操作已有覆盖的部分继续复用，只新增 server 组合产生的行为。

性能测量选取小文本、JSON、带参数路由、大 body、H2 并发、慢客户端和长流式响应；同时记录吞吐、p50/p95/p99、常驻内存、每请求分配，以及大量路由/泛型组合带来的编译时间和代码体积。

便利能力另外观测首个 SSE 事件延迟、压缩前后吞吐／大小、大上传与文件下载的峰值缓冲，以及大量空闲 WebSocket 的实际资源占用。只在测量证明收益时调整压缩阈值、缓冲或默认限制，不把微基准结果变成新的功能验收门槛。

不在本设计中承诺未经测量的吞吐数字。普通行为按仓库约定使用 release；GC stress 只放在确有移动 GC、buffer、挂起和取消交互问题的定向验证中。目标覆盖遵循现有 Darwin、GNU、musl 发布范围。

## 18. 实施前需要先验证的三个技术点

这三项影响具体 API 和实现顺序，不影响“无反射、无容器、显式管线”的总体方案：

1. **H2 接受 Job 与请求 Job 的寿命组合。**用实际交付、提前 reset、请求自身取消、GOAWAY 和强制关闭原型证明监听能正确解除，判断是否真的需要新增所有者交接 API。
2. **服务端头部 deadline 的观测点。**确认 H1 开始/完成头部和 H2 初始 HEADERS/CONTINUATION 的最小事件需求，放在协议层补齐；不能拿客户端响应 timing 或 body idle 代替。
3. **响应终结与清理的组合。**覆盖 H1 未读完请求体、对端半关闭与可观察故障、H2 双向结束、beforeCommit 失败、HEAD 抑制及流式 session 清理，确保 normal finish、reset、close 各自只承担自己的职责。

这些原型应是短小、可验证的真实调用闭环。无需先实现通用 DI、反射、任意参数注入或另一套异步 runtime。

## 19. 参考资料

### 当前仓库的设计依据

以下链接相对于当前文档，可随仓库一起移动。能力状态以对应规范、实现及验收记录为准；第 15 节明确标为拟议的入口不属于这些现有接口。

- [Scoop 语言规范：泛型、函数、Task-local Context、条件接口与 codec](../specs/SCOOP-SPEC.md)
- [Scoop 运行时规范：任务 Context 与运行时契约](../specs/SCOOP-RUNTIME-SPEC.md)
- [Scoop 编译器与产物规范](../specs/SCOOP-IMPL-SPEC.md)
- [路线图：M49、M51、M53、M54、M55、M57 等既有基础](../ROADMAP.md)
- [异步 I/O 规范](../specs/SCOOP-ASYNCIO-SPEC.md)
- [HTTP 规范](../specs/SCOOP-HTTP-SPEC.md)
- [TLS 规范](../specs/SCOOP-TLS-SPEC.md)
- [结构化 coroutineScope 实现](../../sysroot/src/scoop.core/src/coroutine_scope.scoop)
- [H1 connection 公共接口](../../sysroot/src/scoop.http.asyncio/src/http1_connection.scoop)
- [H2 connection 公共接口](../../sysroot/src/scoop.http.asyncio/src/http2_connection.scoop)
- [H2 请求交付](../../sysroot/src/scoop.http.asyncio/src/http2_requests.scoop)
- [H2 等待与取消监听](../../sysroot/src/scoop.http.asyncio/src/http2_wait.scoop)
- [H2 stream 接口](../../sysroot/src/scoop.http.asyncio/src/http2_stream.scoop)
- [H2 read/write idle 实现](../../sysroot/src/scoop.http.asyncio/src/http2_idle.scoop)
- [JSON 显式与 companion codec](../../sysroot/src/scoop.json/src/json.scoop)
- [有序 query 模型](../../sysroot/src/scoop.url/src/query.scoop)
- [Cookie 请求字段解析](../../sysroot/src/scoop.http/src/cookie/header.scoop)
- [HTTP 消息工具：Range、ETag、SSE 与 Cookie](../specs/SCOOP-HTTP-MESSAGE-SPEC.md)
- [Multipart 编解码与异步 part 生命周期](../specs/SCOOP-HTTP-MULTIPART-SPEC.md)
- [WebSocket 握手与扩展协商](../specs/SCOOP-HTTP-WEBSOCKET-SPEC.md)
- [WebSocket 异步会话、关闭与 permessage-deflate](../specs/SCOOP-HTTP-WEBSOCKET-ASYNC-SPEC.md)
- [压缩流、输出限制与格式完成](../specs/SCOOP-COMPRESSION-SPEC.md)
- [URL 规范：query 与百分号编码](../specs/SCOOP-URL-SPEC.md)
- [core 的 Decoder 与 ValueDecoder](../../sysroot/src/scoop.core/src/decoding.scoop)
- [现有 SSE 接收消息模型](../../sysroot/src/scoop.http/src/sse/model.scoop)
- [异步 multipart source](../../sysroot/src/scoop.http.asyncio/src/multipart/source.scoop)
- [异步文件公开接口](../../sysroot/src/scoop.asyncio/src/file.scoop)

### 后续能力的协议依据

- [HTML Standard：Server-sent events](https://html.spec.whatwg.org/multipage/server-sent-events.html)
- [RFC 7578：multipart/form-data](https://www.rfc-editor.org/rfc/rfc7578.html)
- [RFC 6455：WebSocket](https://www.rfc-editor.org/rfc/rfc6455.html)
- [RFC 9110：内容编码协商](https://www.rfc-editor.org/rfc/rfc9110.html#section-12.5.3)
- [RFC 9110：条件请求的求值顺序](https://www.rfc-editor.org/rfc/rfc9110.html#section-13.2.2)
- [RFC 9110：Range](https://www.rfc-editor.org/rfc/rfc9110.html#section-14.2)

### 外部框架

本设计参考官方文档及官方仓库中的接口和行为；GitHub master/main 链接用于说明调研位置，不代表已把该分支内容作为 Scoop 的版本依赖。

- [axum 0.8.9：概览](https://docs.rs/axum/0.8.9/axum/)
- [axum：提取器与 body 消费](https://docs.rs/axum/0.8.9/axum/extract/index.html)
- [axum：handler](https://docs.rs/axum/0.8.9/axum/handler/index.html)
- [axum：Router](https://docs.rs/axum/0.8.9/axum/struct.Router.html)
- [axum：中间件](https://docs.rs/axum/0.8.9/axum/middleware/index.html)
- [matchit：路由匹配与冲突规则](https://github.com/ibraheemdev/matchit)
- [ASP.NET Core 10：Minimal API 参数绑定](https://learn.microsoft.com/en-us/aspnet/core/fundamentals/minimal-apis/parameter-binding?view=aspnetcore-10.0)
- [ASP.NET Core 10：依赖注入与请求服务](https://learn.microsoft.com/en-us/aspnet/core/fundamentals/dependency-injection?view=aspnetcore-10.0)
- [ASP.NET Core 10：使用 HttpContext](https://learn.microsoft.com/en-us/aspnet/core/fundamentals/use-http-context?view=aspnetcore-10.0)
- [ASP.NET Core 10：中间件](https://learn.microsoft.com/en-us/aspnet/core/fundamentals/middleware/?view=aspnetcore-10.0)
- [ASP.NET Core 10：结果执行](https://learn.microsoft.com/en-us/aspnet/core/fundamentals/minimal-apis/responses?view=aspnetcore-10.0)
- [Vert.x Web：官方手册源码](https://github.com/vert-x3/vertx-web/blob/master/vertx-web/src/main/asciidoc/index.adoc)
- [Vert.x Web：RoutingContext](https://github.com/vert-x3/vertx-web/blob/master/vertx-web/src/main/java/io/vertx/ext/web/RoutingContext.java)
- [Vert.x Core：执行 Context](https://github.com/eclipse-vertx/vert.x/blob/master/vertx-core/src/main/java/io/vertx/core/Context.java)
- [Javalin：Context](https://github.com/javalin/javalin/blob/master/javalin/src/main/java/io/javalin/http/Context.kt)
- [Javalin：应用配置](https://github.com/javalin/javalin/blob/master/javalin/src/main/java/io/javalin/config/JavalinConfig.kt)
- [Armeria：HttpService](https://github.com/line/armeria/blob/main/core/src/main/java/com/linecorp/armeria/server/HttpService.java)
- [Armeria：ServiceRequestContext](https://github.com/line/armeria/blob/main/core/src/main/java/com/linecorp/armeria/server/ServiceRequestContext.java)
- [Armeria：RequestContext](https://github.com/line/armeria/blob/main/core/src/main/java/com/linecorp/armeria/common/RequestContext.java)
