# LLM client 实施记录

分支：`codex/llm-client`。状态：实施中。设计依据为 [DESIGN.md](DESIGN.md)、[USAGE.md](USAGE.md)、[协议调研](INVESTIGATION.md)与 [AI SDK 对照评审](AI-SDK-REVIEW.md)。

## 功能闭环

每项先明确规范，再实现、执行所属 Cone 的 `scoop.test`，本机通过后在 `nuc12:~/repos/scoop` 验证 Linux，完成后单独提交。发现底层缺口先在所属库补齐，不在 LLM client 绕过。

| 功能 | 状态 |
| --- | --- |
| 架构、缺省用法与 Vercel AI SDK 对照 | 设计已整理 |
| HTTP 请求级观察、发送／接收 UTC 与单调时间 | 完成：本机与 Linux GNU 各 189/189 通过 |
| TokenUsage 公共计数数据与 companion codec | 完成：本机与 Linux GNU 各 4/4 通过 |
| 公共请求／响应、capability、工具、扩展、错误与历史 codec | 待实施 |
| Client 寿命、流式单消费者／保留策略与结构化输出 | 待实施 |
| Chat protocol、基础与 DeepSeek dialect、工具历史回传 | 待实施 |
| Responses、Messages、Gemini protocols 与 server-side tools | 待实施 |
| 协议矩阵中的其他协议、部署／认证与显式操作 | 待实施 |
| 计时、token usage／count、缓存时间与 dialect billing | 待实施 |
| 持久会话、跨协议组合与使用示例验收 | 待实施 |

## 验证与构建纪律

- 当前没有编译器实现变更；按任务约定不运行完整 fixture 套件。
- 测试使用真实本地协议服务及普通 Cone 测试，不依赖付费模型调用。
- 临时文件和原始日志放在仓库外的 `../scoop-work/llm-client/`，运行命令设置该目录下的 `TMPDIR`；不使用 `/tmp`。
- 在没有构建进程时清理不再需要的 `target` 产物，保留仍需复用的工具与 sysroot。
- TokenUsage 已验证未知／零值、缓存与 reasoning 分项、大整数、模态细项及构造边界，已准备 Darwin／Linux GNU 库产物。供应商归一化与流式 usage 聚合随协议实现交付。
- HTTP 新增 9 个用例，覆盖请求／client observer 共存与去重、共享 H1／H2 连接、信息／最终响应头、重试与重定向、缓存、发送前失败、关闭保护、SSE 与 WebSocket。完整所属 Cone 测试在两平台各通过 189 个用例。连接级首字节采样保持原配置，发送／接收 UTC 采样独立可用。
- Linux 验证使用 `~/repos/scoop-work/llm-client/checkout` 独立工作树与既有 release 工具，保留 `~/repos/scoop` 中的未提交改动。
- 整体实现仍在进行；验证仅覆盖表中已交付项。后续仅记录通过范围、失败闭合和实际限制。
