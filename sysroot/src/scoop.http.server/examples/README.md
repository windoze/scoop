# HTTP server 示例

这些程序使用公开 API，可通过 `scoop run` 单独运行。每个 `.scoop` 文件在头部声明依赖，直接消费已发布的 sysroot 库；不需要创建新的 Cone，也不会被父库收集为生产源码。

| 程序 | 内容 |
| --- | --- |
| [hello.scoop](hello.scoop) | 默认地址与端口、直接配置路由、文本响应 |
| [routing.scoop](routing.scoop) | 预先构建 application、多层参数路由、分组 middleware 与 fallback |
| [json.scoop](json.scoop) | 带默认值的 query DTO、JSON 请求与响应、自动生成 codec |
| [sse.scoop](sse.scoop) | 逐条发送并 flush 的 SSE 事件流 |
| [spa/server.scoop](spa/server.scoop) | 完整的简单启动、嵌套 API 与 SPA 静态文件服务，附 HTML／CSS／JavaScript |

## 运行准备

使用安装好的 Scoop 时，确保 sysroot 包含 `scoop.http.server` 及其依赖。在仓库中开发时，先按[准备脚本说明](../../../../scripts/README.md)准备目标 sysroot，再从仓库根目录执行：

```sh
export PATH="$PWD/target/release:$PATH"
export SCOOP_SYSROOT="$PWD/sysroot"
mkdir -p "$PWD/target/http-server/examples/scratch"
export TMPDIR="$PWD/target/http-server/examples/scratch"
cd sysroot/src/scoop.http.server/examples
```

下文命令均从此 `examples` 目录开始。每次只运行一个示例；它们默认监听 `127.0.0.1:8080`，按 Ctrl+C 结束前台进程。需要换端口时，在现有调用中传入 `HttpServer.listen(scope, port = 3000)`；预先构建 application 的形式则是 `HttpServer.listen(application, scope, port = 3000)`。

事件循环仍由程序显式创建，`listen` 沿用调用方的 loop 和 scope。

全部内置库示例见 [sysroot 示例索引](../../../README.md)。也可使用 [HTTP client 示例](../../scoop.http.client/examples/README.md)调用这里的 hello 和 JSON 服务。

## 简单启动

```sh
scoop run hello.scoop --release
```

在另一个终端访问：

```sh
curl http://127.0.0.1:8080/
# Hello, Scoop!
```

这个示例直接使用 `HttpServer.listen` 的默认值与路由配置重载，不需要自行创建 TCP listener。

## 嵌套路由

```sh
scoop run routing.scoop --release
```

```sh
curl -i http://127.0.0.1:8080/api/v1/teams/scoop/users/42
# 200；X-Example: nested-routes；正文 team:scoop, user:42

curl -i http://127.0.0.1:8080/api/v1/missing
# 404；X-Example: nested-routes；正文 API route not found

curl -i http://127.0.0.1:8080/outside
# 404；正文 Page not found
```

`/api` → `/v1` → `/teams/{team}` 的分组会组合成完整路径；子路由可读取各层参数，并继承外层 middleware 和 fallback。此例先用 `RouterBuilder.build()` 构造 application，再交给同一个 `HttpServer.listen`。

## Query 与 JSON

```sh
scoop run json.scoop --release
```

```sh
curl http://127.0.0.1:8080/greet
# {"message":"Hello, world!"}

curl 'http://127.0.0.1:8080/greet?name=Scoop'
# {"message":"Hello, Scoop!"}

curl -H 'Content-Type: application/json' \
  -d '{"name":"Scoop"}' http://127.0.0.1:8080/greet
# {"message":"Hello, Scoop!"}

curl -i -H 'Content-Type: application/json' \
  -d '{' http://127.0.0.1:8080/greet
# 400

curl -i -H 'Content-Type: text/plain' \
  -d 'Scoop' http://127.0.0.1:8080/greet
# 415
```

DTO 的 companion 声明 `ValueDecoder<T>` 或 `ValueEncoder<T>`，由标准生成器补全 codec。GET 的 query 缺少 `name` 时使用 DTO 的默认值；POST 的 JSON 则要求提供 `name`。

## SSE

```sh
scoop run sse.scoop --release
```

```sh
curl -N http://127.0.0.1:8080/events
```

服务端立即发送第一个 `tick` 事件，之后每秒发送一个，共五个。每条事件都有递增的 `id` 和 `tick N` 数据，发送时立即 flush；最后一条发送完成后结束本次响应，server 继续监听。

## 完整 SPA

这个示例的 `public` 是相对于进程工作目录的路径，因此先进入 `spa`：

```sh
cd spa
scoop run server.scoop --release
```

在浏览器打开 <http://127.0.0.1:8080/>，点击「个人设置」切换到 `/settings/profile`，再刷新页面。客户端通过 History API 切换页面；服务端找不到对应文件时，由 `fallbackFile` 返回 `public/index.html`。页面上的「查询用户 42」会调用嵌套 API。

也可以直接检查各类请求：

```sh
curl http://127.0.0.1:8080/hello
# Hello, Scoop!

curl http://127.0.0.1:8080/api/v1/users/42
# user:42

curl -I http://127.0.0.1:8080/settings/profile
# 200；Content-Type: text/html; charset=utf-8

curl -I http://127.0.0.1:8080/assets/main.js
# 200；JavaScript 文件

curl -i http://127.0.0.1:8080/assets/missing.js
# 404

curl -i http://127.0.0.1:8080/api/v1/missing
# 404
```

真实文件正常发送，目录首页优先于 SPA 回退。`/assets/{*path}` 单独使用没有 fallback 的 `staticFiles`，保留资源缺失的 404；`/api/{*path}` 显式返回 404，避免未知 API 路径命中根 SPA 通配路由。分组 fallback 只处理未匹配路径，因此这里不能用它覆盖已经匹配的 SPA 路由。

末尾通配不匹配空后缀，示例另外注册了 `/`、`/api`、`/api/`、`/assets` 和 `/assets/`。入口文件路径显式写成 `root.resolve(Path.fromString("index.html"))`；fallback 自身缺失时仍返回 404。

hello 服务与 GET 客户端的完整配对说明见 `scoop example http/get`（[正文](../../../share/scoop/examples/topics/http/get.md)）。
