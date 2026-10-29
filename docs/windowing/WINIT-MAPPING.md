# winit 能力映射与首期范围

状态：分阶段实施；更新于 2026-10-30。与 [DESIGN.md](DESIGN.md) 和[规范](../specs/SCOOP-WINDOW-SPEC.md)一起阅读。基线为 winit 0.30.13 的桌面能力；下表是完整能力地图，实际交付以 [PROGRESS.md](PROGRESS.md) 为准。

当前阶段先交付 macOS GUI 骨架：应用／窗口、proxy／关闭／lease、基础属性、显示器与 DPI、键盘／鼠标／滚轮／IME、renderer 与 asyncio 接入。独占显示模式、全局 raw device、触摸／高级手势、光标捕获和复杂平台窗口交互后续补充。表中的“本期”描述原设计的窗口层目标范围，不能据此扩大当前阶段，也不代表要实现完整 AppKit GUI。

范围分为：本期通用、本期按平台能力、平台扩展、后续／范围外。不承诺逐字复制 Rust API、trait、Drop 或所有平台扩展。Scoop 尚未发布；本表随实际实现直接修订，不保留旧草案签名兼容层，也不为未来平台穷举无实现的变体。窗口后端使用原生系统／协议库；全项目 Rust 依赖汇总和统一 staticlib 构建属于独立流水线工作，不在本表范围内。

## 1. 应用与窗口

参照：[ApplicationHandler](https://docs.rs/winit/0.30.13/winit/application/trait.ApplicationHandler.html)、[Window](https://docs.rs/winit/0.30.13/winit/window/struct.Window.html)、[WindowAttributes](https://docs.rs/winit/0.30.13/winit/window/struct.WindowAttributes.html)。

| winit 能力 | Scoop 目标 | 范围与平台差异 |
| --- | --- | --- |
| EventLoop／ActiveEventLoop／run_app | Application／ActiveApplication／run | 本期通用；单次、主线程 |
| EventLoopProxy／user_event | ApplicationProxy.post | 本期；类型化闭包代替泛型 user-event 通道 |
| new_events／about_to_wait | ApplicationEvent.NewEvents／AboutToWait | 本期；有限批次，等待前重新检查队列 |
| resumed／suspended／exiting／memory_warning | ApplicationEvent 对应生命周期事件 | 本期模型；启动先 Resumed，不把失焦当 Suspended；只报告系统提供的 memory warning |
| ControlFlow | Wait／Poll／WaitUntil(TimeMark) | 本期；单调来源检查 |
| run_on_demand／pump_app_events／嵌入外部 loop | 专用平台接入 | 后续；首期不作跨平台承诺 |
| create_window／WindowId | createWindow／WindowId | 本期；创建与可绘制配置分开，Wayland configure 的零维度由客户端选择 |
| title／visible／resizable／decorations | setTitle／setVisible／setResizable／setDecorations | 本期；显示系统限制明确返回，Wayland 隐藏／重显重新执行映射与 configure 握手 |
| inner／outer size、position、resize increments | metrics、outerBounds、desktopPosition、requestSize／requestPosition／setResizeIncrements | 本期按能力；Wayland 无全局窗口位置 |
| min／max inner size | setSizeConstraints | 本期；逻辑尺寸，参数互相一致 |
| enabled buttons／window level | setEnabledButtons／setWindowLevel | 本期按能力，保留按钮集合与上下层级 |
| minimize／maximize | requestMinimized／requestMaximized | 本期；请求不等于状态确认，未知状态为 None |
| borderless／exclusive fullscreen | requestFullscreen(Fullscreen) | 本期按能力；Wayland 不提供独占显示模式；退出恢复实际改变的模式 |
| focus／attention | requestFocus／requestAttention | 本期按能力；不承诺绕过 OS 激活策略 |
| window menu／drag／drag resize | showWindowMenu／beginMove／beginResize | 本期按能力；保存当前输入上下文，阻塞的原生模态操作在 handler／session pump 返回后执行 |
| transparency／blur／content protection | 对应属性与设置方法 | 本期按能力；不把视觉提示当作安全保证 |
| theme／preferred theme | observedTheme／requestTheme | 本期；未知与系统跟随分开 |
| window icon | WindowIcon 与 setIcon | 本期按能力；不把 macOS 应用图标冒充单窗口图标 |
| close on Drop | requestDestroy／Closing／Destroyed | 本期改用显式寿命；GC 不调用 GUI |
| raw-window-handle／raw-display-handle | NativeWindowLease 与类型化 native handle | 本期按实际后端提供载荷，Windows 实施时增加 Win32；lease 不授权待配置时提交帧 |

## 2. 事件、输入与绘制

参照：[WindowEvent](https://docs.rs/winit/0.30.13/winit/event/enum.WindowEvent.html)、[KeyEvent](https://docs.rs/winit/0.30.13/winit/event/struct.KeyEvent.html)、[Ime](https://docs.rs/winit/0.30.13/winit/event/enum.Ime.html)。

| winit 能力 | Scoop 目标 | 范围与平台差异 |
| --- | --- | --- |
| CloseRequested／Destroyed | 同名窗口事件，补充 Closing | 本期，意图与销毁分离 |
| Resized／ScaleFactorChanged | GeometryChanged(WindowMetrics) | 本期，完整快照替代借出的 InnerSizeWriter |
| Moved／Focused／Occluded／ThemeChanged | 位置、焦点、遮挡、主题事件 | 本期；平台不知道的状态保持未知 |
| KeyboardInput／ModifiersChanged | KeyEvent／ModifiersChanged | 本期，物理键／逻辑键／位置／repeat／synthetic 分离 |
| KeyEvent.text 与 IME Commit | TextInputEvent.Edit 批次中的 Commit 操作 | 本期统一文字入口；同批删除、提交和组合更新只调用一次 handler |
| Ime Preedit／Enabled／Disabled | 批次内 Preedit／ClearPreedit 及 Enabled／Disabled 事件 | 本期；UTF-8 字节范围、选择活动端、candidate rect；完整操作见窗口规范第 5 节 |
| IME allowed／cursor area、dead-key reset | setImeAllowed／setImeCursorArea、失焦撤销组合 | 本阶段；只保存 native marked text，UTF-16 转 UTF-8，不发布应用文档 |
| IME purpose、surrounding 文档与重转换 | 后续文本输入扩展 | 后续；TextInputState 当前为应用纯值辅助，不要求窗口层维护编辑器镜像 |
| CursorEntered／Left／Moved／MouseInput | 指针事件与按钮 | 本期；坐标和设备来源明确 |
| MouseWheel／AxisMotion | LineDelta／PixelDelta 与轴事件 | 本期按能力；设备单位不冒充像素 |
| DeviceEvent、raw motion 与监听策略 | ApplicationEvent.Device、FocusedOnly／Always／Never | 本期按能力；不隐式打开全局监听 |
| Touch／pressure | TouchEvent／可选 pressure | 本期按能力；macOS 无触摸屏事件时不伪造 |
| Pinch／Pan／Rotation／DoubleTap／TouchpadPressure | NativeGestureEvent | 本期按能力；不是通用手势识别器 |
| cursor icon／custom cursor／visibility | CursorIcon／CustomCursor／setCursorVisible | 本期；自定义 RGBA 图像／hotspot 复制后取得独立寿命 |
| cursor grab／warp／hittest | None／Confined／Locked、requestCursorPosition／setCursorHitTest | 本期按能力；Wayland 不支持任意 warp |
| HoveredFile／DroppedFile／HoveredFileCancelled | FileDrop Enter／Drop／Leave | 本期，Path／OsString 保留平台路径，不把任意 URI 强转路径 |
| RedrawRequested／request_redraw | 同名绘制请求 | 本期；合并、无递归、零 drawable 与隐藏节流，重新 configure 后恢复 |
| pre_present_notify | prePresentNotify | 本期；为提交前通知，不是 present／GPU fence |
| monitor／primary monitor／video modes | Monitor／VideoMode／可选 primary | 本期按能力；毫赫兹、可选字段、热拔插 |
| ActivationTokenDone、startup notification | Linux 平台扩展与公共激活结果 | 本期 Linux 按协议；OS token 不进入普通窗口身份或权限机制 |

## 3. 平台扩展和范围外能力

| 类别 | 决策 |
| --- | --- |
| macOS activation policy、透明标题栏／content view、应用标识 | `scoop.window.platform.macos` 普通选项；不把 NSWindow style mask 放到公共属性 |
| Linux backend／display name、Wayland app id、X11 class／instance、visual 选择 | `scoop.window.platform.linux` 与必要的 wayland／x11 子包；不向全平台代码提供 XID 操作 |
| Windows owner HWND、taskbar／DWM、TSF／COM、Win32 样式扩展 | 后续 `platform.windows`；复用 Windows 分支平台基础，补窗口创建前的 Per-Monitor V2 与系统模态循环接入 |
| 任意 native parent／child window 嵌入、run-on-demand | 后续专项，不能用 WindowId 假装支持外部句柄所有权 |
| Android／iOS／Web／orbital 的专用入口与扩展 | 范围外 |
| clipboard、菜单模型、文件对话框、accessibility、IME 编辑器布局、Skia 图形设备 | 不属于 winit 对应基础层，后续库实现 |

平台能力缺失与未实施功能必须区分。实现某一交付批次时，不能把尚未写完的分支永久返回 Unsupported，声称该平台没有能力。先以真实窗口、提交一帧、asyncio 和 lease 关闭完成端到端子集，再扩大能力；每批按实际范围交付并修订 API。当前骨架阶段完成与整个能力地图完成分别记录。
