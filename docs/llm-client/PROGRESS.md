# LLM client 实施记录

分支：`codex/llm-client`。状态：实施中。设计依据为 [DESIGN.md](DESIGN.md)、[USAGE.md](USAGE.md)、[协议调研](INVESTIGATION.md)与 [AI SDK 对照评审](AI-SDK-REVIEW.md)。

## 功能闭环

每项先明确规范，再实现、执行所属 Cone 的 `scoop.test`，本机通过后在 `nuc12:~/repos/scoop` 验证 Linux，完成后单独提交。发现底层缺口先在所属库补齐，不在 LLM client 绕过。

| 功能 | 状态 |
| --- | --- |
| 架构、缺省用法与 Vercel AI SDK 对照 | 设计已整理 |
| HTTP 请求级观察、发送／接收 UTC 与单调时间 | 完成：本机与 Linux GNU 各 189/189 通过 |
| HTTP timeout 缺省读取与请求级覆盖组合 | 完成：本机与 Linux GNU 各 190/190 通过 |
| TokenUsage 公共计数数据与 companion codec | 完成：本机与 Linux GNU 各 5/5 通过 |
| Unit companion codec 与显式访问 | 完成：本机与 Linux GNU 的 JSON Cone 各 94/94 通过 |
| 输入／输出媒体与生成 capability 数据 | 完成：本机与 Linux GNU 各 18/18 通过 |
| Url companion codec | 完成：本机与 Linux GNU 的 URL Cone 各 19/19 通过 |
| 公共请求、四类工具声明、按位置扩展与历史数据 codec | 完成：本机与 Linux GNU 各 32/32 通过 |
| 公共响应、结构化解码、历史读取与计时快照数据 | 完成：本机与 Linux GNU 各 46/46 通过 |
| Chat 完整响应、OpenAI／DeepSeek dialect 与实际历史投影 | 完成：本机与 Linux GNU 各 59/59 通过 |
| Chat 请求编码、多模态、工具／扩展位置与历史接续 | 完成：本机与 Linux GNU 各 75/75 通过 |
| 工具 capability 与模型绑定检查 | 待实施 |
| Client 寿命、流式单消费者／保留策略与结构化输出 | 待实施 |
| Chat 生成传输、provider 绑定与流式 reducer | 待实施 |
| Responses、Messages、Gemini protocols 与 server-side tools | 待实施 |
| 协议矩阵中的其他协议、部署／认证与显式操作 | 待实施 |
| 计时、token usage／count、缓存时间与 dialect billing | 待实施 |
| 持久会话、跨协议组合与使用示例验收 | 待实施 |

## 验证与构建纪律

- Capability 实施触发 Unit companion 的 core／parser 缺口，已先行修复。Parser 共 691 项与 HIR 定向用例通过；JSON Cone 两平台各 94 项通过，其中新增 4 项覆盖 companion、条件泛型、容器和错误输入。不引入 fixture。
- HIR 定向用例使用既有 opt-level 1／debug 0 配置，保留调试断言和溢出检查，未调整线程栈；无优化测试进程曾栈溢出，切换到该配置后通过。release 定向运行也通过。
- 测试使用真实本地协议服务及普通 Cone 测试，不依赖付费模型调用。
- 临时文件和原始日志放在仓库外的 `../scoop-work/llm-client/`，运行命令设置该目录下的 `TMPDIR`；不使用 `/tmp`。
- 在没有构建进程时清理不再需要的 `target` 产物，保留仍需复用的工具与 sysroot。
- TokenUsage 已验证缺失字段恢复缺省、未知／零值、缓存与 reasoning 分项、大整数、模态细项及构造边界，已准备 Darwin／Linux GNU 库产物。供应商归一化与流式 usage 聚合随协议实现交付。
- Capability 新增 13 项测试，覆盖三态、缺省 Unknown、MIME／传递形式精确组合、参数与重复组合、输入／输出和媒体种类的独立性、生成控制、已知零／未知上限及 codec 往返。当前负责数据描述与查询，请求校验和工具种类能力仍随后续实现。
- 媒体请求需要保存 Url，已在 URL 库补 companion codec；新增 5 项测试覆盖规范 URI、相对／非网络引用、Unicode、缺失／空组件与带路径的解码失败。复用现有解析器，不执行网络 I/O。
- 请求模型新增 14 项外部 package 消费者测试，覆盖缺省 User、显式零／false／空集合、结构化输出 schema、扩展作用位置与重复诊断、媒体／视频帧、原生历史与旧工具引用、四类工具及精确选择。两平台各 32 项通过；目前验证数据与保存合同，实际协议编码、capability 联动和输出 toInput 仍待实现。
- 响应与计时新增 14 项测试，完整 LLM Cone 两平台各 46 项通过。覆盖有序读取视图、工具参数只保留一份可保存原文、多候选／多文本消息选择、拒绝和不完整输出、原生历史读取、引用单位、失败记录、分次重试及 wall clock 回拨。当前 toInput 读取已经投影的历史；协议投影、实时采样和生成调用随后接入。
- Chat 响应解释新增 13 项测试，完整 LLM Cone 两平台各 59 项通过。覆盖 DeepSeek reasoning 历史、音频 ID 投影、function／custom 调用、无效与截断参数、未知原生活动、多候选、带路径错误及两种缓存口径；请求编码、生成传输和流式 reducer 继续实施。
- Chat 请求编码新增 16 项测试，完整 LLM Cone 两平台各 75 项通过。覆盖缺省省略、显式零／false／空数组、按位置扩展、管理字段冲突、媒体格式与文件归属、DeepSeek thinking 约束、工具选择、请求级搜索及保存后的历史接续。OpenAI Chat 搜索声明映射 web_search_options；不能表达的显式选项报告输入路径，不伪造搜索活动。
- HTTP 新增 9 个用例，覆盖请求／client observer 共存与去重、共享 H1／H2 连接、信息／最终响应头、重试与重定向、缓存、发送前失败、关闭保护、SSE 与 WebSocket。完整所属 Cone 测试在两平台各通过 189 个用例。连接级首字节采样保持原配置，发送／接收 UTC 采样独立可用。
- 生成接入前补齐 HTTP 的只读 defaultTimeouts；新增普通 Cone 用例验证借入者可保留网络／排池设置而单独更改响应头等待，后续普通调用仍使用原缺省。完整 HTTP Cone 两平台各 190 项通过。
- Linux 验证使用 `~/repos/scoop-work/llm-client/checkout` 独立工作树；Unit 修复后已从相应源码重建 release 工具、23 个库及 runtime，保留 `~/repos/scoop` 中的未提交改动。旧 runtime 与新工具不匹配的启动失败在正式准备后重跑通过。
- 整体实现仍在进行；验证仅覆盖表中已交付项。后续仅记录通过范围、失败闭合和实际限制。
