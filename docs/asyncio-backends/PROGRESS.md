# EventLoop 后端实施记录

设计与合同见 [DESIGN.md](DESIGN.md) 和 [asyncio 规范](../specs/SCOOP-ASYNCIO-SPEC.md)。

## 进度

- 2026-10-27：开始实施，保留现有公共行为，以共享 owner、Darwin、宿主接入、Linux、消费者迁移的顺序提交。
- 共享层已完成：资源、操作和连接竞速只依赖 `LoopOwner`；一次非阻塞处理消费有限快照并阻止递归进入，同步运行复用该流程。native 公共头不再暴露 uv 类型，过渡适配的布局集中在私有头。
- 当前仍由 libuv 提供原生后端；`PlatformEventLoop`、session 与新后端尚未交付。

## 验证约定

普通行为与真实 native 宿主测试均使用所属 Cone 的 `scoop.test`，不新增 fixture。每批先格式化与检查，再运行影响范围内的 release 测试；完成平台替换后再运行相关组合测试。原始日志保存在仓库外，不使用 `/tmp`。

Linux 远端原仓库存在未提交工作，测试使用独立源目录及目标输出，不覆盖该工作区。

## 已完成验证

- 共享层：Darwin release，`scoop check --tests` 与受影响 C 的严格编译检查通过；asyncio 55 个容器、135 项测试全部通过，含分轮投递及嵌套处理两项新增测试。复用了同一测试构建，未运行全仓测试。
- 验证前重建了与现有 sysroot 配套的 release 命令，解决旧本地命令的 ABI 版本不匹配。未修改 ABI。
- 首次清理使用 `cargo clean --profile dev`，仓库 `target` 从约 21 GB 降至 8.9 GB；配套命令重建后约 9.4 GB。
