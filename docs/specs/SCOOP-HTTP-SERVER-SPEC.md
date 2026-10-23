# Scoop HTTP server 库规范

状态：按[设计](../http-server/DESIGN.md)分步实施，已完成范围与验证记录见[进度](../http-server/PROGRESS.md)。本文规定普通 Cone `scoop:scoop.http.server:0.1.0` 的应用层合同。协议与异步传输继续遵守 [HTTP 规范](SCOOP-HTTP-SPEC.md)，不增加编译器或 runtime 特例。

## 1. 请求分派与依赖

HttpHandler 是 `suspend (HttpContext) -> HttpResult`，HttpNext 是 `suspend () -> Unit`，HttpMiddleware 是 `suspend (HttpContext, HttpNext) -> Unit`。应用共享对象通过构造参数或闭包传递；请求对象通过 HttpContext 或语言规范 8.3 的精确类型 Context 绑定传递。框架不提供服务容器、运行期函数参数扫描或自动事务提交。

RouterBuilder 配置路由、middleware、分组与 fallback；build 返回不可变 HttpApplication，并关闭该 builder 及其 RouteGroup 的修改入口。已注册的字段、路径和 middleware 列表保存快照。不同 application 互不修改配置；捕获的业务对象是否允许并发由应用负责。

HttpContext 属于一次请求，保存 request、response、route、pathParams 和 requestId。实际执行必须处于活动 Job 中，requestJob 为该请求任务。框架在执行全局 middleware 前绑定精确 HttpContext。离开请求管线恢复原绑定；挂起和恢复不结束绑定。应用 Context 的缺失是装配错误，默认 500，不是匿名用户的认证结果。

## 2. 路由与提取

方法为大小写敏感 HTTP token。路径按原始 `/` 分片，各片段至多百分号解码一次；编码斜线不形成路由层级。静态片段比较解码后的字节，参数保留原始和解码后的字节；String 访问使用严格 UTF-8。路径不折叠连续斜线、尾斜线或点片段，query 不参与路由。

路径形状支持静态片段、`{name}` 和仅出现在末尾的 `{*name}`。参数名须非空且同一路由不重复；普通参数匹配非空片段，末尾通配匹配非空后缀。畸形花括号和非法百分号为配置错误；请求中的非法百分号和文本转换错误为 400。

配置期按路径片段构造 trie。完整匹配按静态、参数、通配的顺序选择；静态分支没有完整匹配时允许回退。先选择路径资源，再选择方法；同方法、同路径形状的重复注册拒绝。不同方法可使用不同的参数名，取最终选择的端点声明。分组前缀不能包含末尾通配，同形状分组前缀重复注册拒绝，以免 fallback 和组级 middleware 有两个相同归属。

显式 HEAD／OPTIONS 优先；HEAD 否则使用 GET handler，但不执行响应 producer。普通路径 OPTIONS 默认 204，Allow 包括所有显式方法及适用的隐式 HEAD、OPTIONS，稳定按 token 排序。资源无相应方法时 405，不退回低优先级路径。OPTIONS * 只经全局 middleware，Allow 为全应用的方法并集。未配置的 CONNECT 为 501；非 origin-form 的普通 H1 target 不作为路径匹配，返回 400。

无端点匹配时采用最长适用分组的 fallback，等长按静态片段优先；默认 404。404 和默认 405／OPTIONS 执行这个最长适用分组的 middleware，正常端点执行其注册分组及端点 middleware。全局 middleware 覆盖所有这些结果。route 只在路径端点选定后可见，fallback 没有虚构的端点模板。

Query 复用 UrlQuery，普通 URI 的 `+` 保持加号。标量读取拒绝重复值；optional 只容许字段缺失，不把无等号或空字符串自动当作缺失。Path 参数缺失为程序错误，转换失败为 HttpRejection。DTO 与 body 提取的增量合同随后按所属功能补充。

## 3. 管线与响应

middleware 按登记顺序进入，反向退出。每次只能等待 next 一次、respond 一次，或抛异常；重复／并发 next、未处理却正常返回、响应后再委托均报程序错误。next 的寿命包括 handler、结果执行、流式 producer、finish 和下游清理。局部并发必须在资源作用域内等待，不因 Context 绑定自动延长资源寿命。

HttpResult 保存状态码、HttpField 快照和 HttpBody。基础 body 为 Empty、Bytes 和 Stream；Stream 保存可选已知长度、挂起 producer 和完成时取得的 trailer。text、bytes、json、redirect 和 empty 是普通构造函数。JSON 使用显式或 companion ValueEncoder，不反射业务对象。

HttpResponse 保存提交前的 set／add／remove 操作及 beforeCommit。set／remove 按字段名 ASCII 大小写无关地替换／移除；add 保留重复项，Set-Cookie 不合并。执行依次应用结果字段、暂存操作、普通非挂起 beforeCommit；回调接收可修改的 HttpResponseDraft，回调序列退出后草稿只读。状态依次为准备、已提交、已结束，失败可转为已中止。开始不可撤回的发送尝试前标为已提交，之后不再修改头或发送第二个最终响应。

HttpBodyWriter 是执行期间借用的 AsyncByteSink 视图，允许短写，flush 不代表客户端已经处理内容。producer 不能 finish 底层 HTTP 消息。原文已知长度在实际写入时检查，完成时检查精确长度；超长／过短均失败。HEAD 不调用 producer 或 trailer 回调；204／304／205 等响应按 HTTP 消息内容规则处理。正常输出依次完成 producer、trailer 和底层 finish；结束后 writer 不可再用。

handler 和结果准备失败在终端内部映射，使错误结果仍在业务 Context 范围内执行。HttpRejection 携带明确结果；其他非取消失败默认 500，正文仅含稳定提示和 requestId。beforeCommit 失败不重放回调，以不应用原草稿的最小 500 收尾。mapper 或错误结果失败直接中止，不递归映射。最外层另外处理 middleware 自身失败。取消及提交后的异常保留实际原因并中止，发送后的清理异常不能改写已经发出的状态。

错误观察入口是普通非挂起函数，接收当前 context 和原始异常；观察器失败不得替代主异常或造成另一条正常请求失败。观察器与应用代码均不在框架内部锁内调用。

## 4. 分步交付与验证

接入、超时与停机、SSE、multipart、WebSocket、压缩、文件和 DTO 的完整范围保持设计第 12～17 节，按完成的功能逐项扩充本规范。尚未实现的功能不公开占位 API。测试用所属 Cone 的 release 测试；只有 CLI／诊断／外部对端等必要边界使用 fixture。协议引擎的既有验证不在 server 重放。
