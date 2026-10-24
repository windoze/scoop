# LLM client 实施记录

分支：`codex/llm-client`。状态：实施中。设计依据为 [DESIGN.md](DESIGN.md)、[USAGE.md](USAGE.md)、[协议调研](INVESTIGATION.md)与 [AI SDK 对照评审](AI-SDK-REVIEW.md)。

## 功能闭环

每项先明确规范，再实现、执行所属 Cone 的 `scoop.test`，本机通过后在 `nuc12:~/repos/scoop` 验证 Linux，完成后单独提交。发现底层缺口先在所属库补齐，不在 LLM client 绕过。

| 功能 | 状态 |
| --- | --- |
| 架构、缺省用法与 Vercel AI SDK 对照 | 设计已整理 |
| HTTP 请求级观察、发送／接收 UTC 与单调时间 | 完成：本机与 Linux GNU 各 189/189 通过 |
| TokenUsage 公共计数数据与 companion codec | 完成：本机与 Linux GNU 各 5/5 通过 |
| Unit companion codec 与显式访问 | 完成：本机与 Linux GNU 的 JSON Cone 各 94/94 通过 |
| 输入／输出媒体与生成 capability 数据 | 实施中：本机与 Linux GNU 各 18/18 通过，待提交 |
| 公共请求／响应、capability、工具、扩展、错误与历史 codec | 待实施 |
| Client 寿命、流式单消费者／保留策略与结构化输出 | 待实施 |
| Chat protocol、基础与 DeepSeek dialect、工具历史回传 | 待实施 |
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
- HTTP 新增 9 个用例，覆盖请求／client observer 共存与去重、共享 H1／H2 连接、信息／最终响应头、重试与重定向、缓存、发送前失败、关闭保护、SSE 与 WebSocket。完整所属 Cone 测试在两平台各通过 189 个用例。连接级首字节采样保持原配置，发送／接收 UTC 采样独立可用。
- Linux 验证使用 `~/repos/scoop-work/llm-client/checkout` 独立工作树；Unit 修复后已从相应源码重建 release 工具、23 个库及 runtime，保留 `~/repos/scoop` 中的未提交改动。旧 runtime 与新工具不匹配的启动失败在正式准备后重跑通过。
- 整体实现仍在进行；验证仅覆盖表中已交付项。后续仅记录通过范围、失败闭合和实际限制。
