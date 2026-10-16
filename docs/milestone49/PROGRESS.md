# M49 实施进度

基线：`d233d05a5`。日期：2026-10-16。规范先行，按功能验证后提交；本机定向验证后在 Linux GNU／musl 完整验收。日志和测试工作目录位于仓库外，不使用 `/tmp`。

| 批次 | 状态 |
| --- | --- |
| core 任务树、失败传播与取消 | 实施中，公共合同已写入语言规范 11.9.1 |
| 可取消挂起协调 | 合同已定稿，待实现 |
| platform 异步 buffer 借用与 Libuv 错误域 | 待实施 |
| asyncio 事件循环、定时等待与投递 | 待实施 |
| TCP 与异步流组合 | 待实施 |
| 文件、DNS、UDP | 待实施 |
| 三目标组合及完整验收 | 待实施 |

开始时清理旧 debug 构建数据，保留 release 工具，`target` 从约 103 GB 降至 1.3 GB。M47 的已有文档改动不属于本任务提交。

core 初次跨 Cone 验证发现：普通泛型模板调用 internal suspend 成员时，HIR 已保存支持声明，但 MIR callable 导出选择遗漏该成员。已先独立修复为遍历共有完整声明目录；新增 `m49-suspend-support-export` 覆盖 private 顶层及 internal 成员的真实跨 Cone 调用，debug／release 和 moving GC 通过。相关 Rust 119 项由一轮 118 项通过及旧断言修订后的单项补验闭合；格式化和 workspace clippy 通过。未改变 wire 或 runtime ABI。
