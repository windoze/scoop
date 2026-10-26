# EventLoop 后端实施记录

设计与合同见 [DESIGN.md](DESIGN.md) 和 [asyncio 规范](../specs/SCOOP-ASYNCIO-SPEC.md)。

## 进度

- 2026-10-27：开始实施，保留现有公共行为，以共享 owner、Darwin、宿主接入、Linux、消费者迁移的顺序提交。
- 当前仍由 libuv 提供原生后端；`PlatformEventLoop`、session 与新后端尚未交付。

## 验证约定

普通行为与真实 native 宿主测试均使用所属 Cone 的 `scoop.test`，不新增 fixture。每批先格式化与检查，再运行影响范围内的 release 测试；完成平台替换后再运行相关组合测试。原始日志保存在仓库外，不使用 `/tmp`。

Linux 远端原仓库存在未提交工作，测试使用独立源目录及目标输出，不覆盖该工作区。
