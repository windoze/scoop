# GUI 基础层实施记录

日期：2026-10-31。分支：`codex/gui-macos`。范围为共享架构和 macOS 两个目标；其他平台 renderer 留待后续。合同见 [设计](DESIGN.md)与 [规范](../specs/SCOOP-GUI-SPEC.md)。

## 当前状态

| 内容 | 状态 |
| --- | --- |
| window 鼠标序列取消、单调输入时间 | 已实现并验证 Darwin 两目标 |
| 固定 Skia／HarfBuzz／ICU 依赖与绘图 Cone | 进行中 |
| GUI 宿主、共享帧调度、macOS Metal 呈现 | 待完成 |
| 字体、段落布局及索引／几何查询 | 待完成 |
| 动画、输入区域、焦点和 IME 路由 | 待完成 |
| 组合示例、两目标集成验收 | 待完成 |

## window 前置能力

`InputTime.Event`／`Observed` 明确区分事件时间和接收时间，两者使用既有 `MonotonicTimeSource`。原生事件排队前保存秒／纳秒，键盘补发 release 重新采样，业务回调不重读时间。当前 AppKit 和 Win32 映射使用 Observed。

`PointerEvent.Cancelled` 结束窗口聚合的按下按钮集合，携带 FocusLost 或 CaptureLost 及独立时间，不伪造位置和正常 Up。AppKit 覆盖窗口／应用失焦及菜单跟踪，重复取消和取消后的迟到 Up 不重复交付；Left 继续正常拖动。为保持现有后端与共享 ABI 一致，Win32 同步字段、WM_CAPTURECHANGED／WM_CANCELMODE 和正常 ReleaseCapture 顺序；本轮没有执行 Windows 验证。

Darwin ARM64 和 Rosetta x86_64 的 `scoop.window` 各 4 容器、12 测试通过，包括真实 AppKit 输入方法、通知、IME、跨线程 lease 与关闭。`scoop.window.asyncio` 各 1 容器、1 测试覆盖原生 modal／menu 循环内的 I/O、根取消与 lease 收尾。x86_64 初次执行在 modal 观察断言失败、空闲复跑通过；测试辅助程序随后将短观察期的起点移至真正进入对应原生循环，避免 AppKit 冷启动消耗观察期。

Scoop 格式化、Cone check 及变更的 Objective-C 严格编译检查通过。验证全部使用 `scoop.test` 和普通测试 Cone，没有新增或执行 fixture；原始日志保存在仓库外。

协议调用与通知测试不等同于实际用户拖动、外部窗口抢焦点、输入法候选交互或混合 DPI 硬件验证。GUI 的显示链路和桌面验收仍须随后续批次完成。
