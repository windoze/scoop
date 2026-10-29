# 桌面窗口与事件库设计

状态：设计，待实施。日期：2026-10-27。只设计 macOS、Linux Wayland／X11；Windows 仅确定公共契约和后端接缝，不实现、不增加 Windows target 或伪实现。

目标是为 Skia／HarfBuzz 上层 GUI 提供 winit 对应的窗口、输入、显示器与主循环能力。公共 API 使用 Scoop，平台实现使用成熟的系统／协议库。功能基线取 [winit 0.30.13](https://docs.rs/winit/0.30.13/winit/)，另参考 0.31 beta 的演进方向，不追随其未稳定 API；逐项范围见 [WINIT-MAPPING.md](WINIT-MAPPING.md)，目标合同见[窗口库规范草案](../specs/SCOOP-WINDOW-SPEC.md)。

## 1. 已落地 async backend 的接入结论

本设计已核对当前源码，而非直接沿用旧提案：

| 已有实现 | 可复用的事实 | 窗口层的责任 |
| --- | --- | --- |
| [PlatformEventLoop](../../sysroot/src/scoop.asyncio/src/platform_event_loop.scoop) | 单次 run／attach；attach 捕获 Context；初始化失败回滚 | 从主线程、当前 Job 外安装，不在窗口事件内再启动 loop |
| [LoopDriver](../../sysroot/src/scoop.asyncio/src/loop_drive.scoop) | 有限批次、非阻塞 pump、阻止递归进入 | 宿主保留原生输入和重绘的执行机会 |
| [LoopMailbox](../../sysroot/src/scoop.asyncio/src/loop_mailbox.scoop) | post 经根 continuation 执行；取消交给 owner | 直接复用任务、Context 和取消规则，不另建 GUI Job |
| [EventLoopSession](../../sysroot/src/scoop.asyncio/src/event_loop_session.scoop) | I/O 排空后注销 watch，再销毁并通知 onClosed | 在 onClosed 之前维持原生主循环 |
| [EventLoopHost](../../sysroot/src/scoop.asyncio/src/host/host.scoop) | 监听只借用 fd；remove 等 ready 退出 | 提供真实 AppKit／Wayland／X11 宿主适配 |
| [MainDispatchHost](../../sysroot/src/scoop.asyncio/platform/darwin/main_dispatch_host.scoop) | 主队列通知及 ForeignCallback 保活／异常观察 | 继续验证菜单跟踪、拖动和模态，不把 CFRunLoop 测试当成完整 AppKit 验收 |

[实施记录](../asyncio-backends/PROGRESS.md)已报告 Darwin／GNU／musl、Linux 6.1、取消竞争和定向 moving GC 验收；本次没有重跑这些测试。[性能记录](../asyncio-backends/PERFORMANCE.md)也明确窗口系统尚未验证，Darwin 部分指标低于原 libuv 基线，因此本设计不预设新窗口层具有性能优势。

当前缺口是窗口系统集成，而不是另一套 I/O 调度器。另有一个必须修正的可移植性边界：`watchReadable(Int)` 只表达 POSIX fd；不能将 HANDLE 截断成 Int 或把 Windows 消息循环伪装为 fd，处理方案见第 4 节。

## 2. 分层与交付边界

新增两个普通 Cone：

- `scoop.window`：依赖 core／std／platform，提供应用主循环、窗口、事件、显示器、输入和受控 native 句柄借用；不依赖 asyncio、Skia 或 HarfBuzz。
- `scoop.window.asyncio`：依赖 window／asyncio，把已有 PlatformEventLoop 附着到窗口宿主，协调应用退出与 I/O 收尾。没有第二套 Job、cancel、join 或 native I/O 实现。

选择“移植能力与平台语义”，不把 Rust ABI 或 winit 类型放进 Scoop API。原生层分别使用 AppKit、libwayland-client／wayland-protocols／libxkbcommon／libdecor、Xlib／XInput2／XRandR／Xcursor／XKB／XIM。引用或改写 winit 的键映射、算法和平台规则时保留相应许可证与来源。窗口协议、键盘映射、输入法与装饰优先复用这些成熟库，不自行实现 wire protocol 或完整输入法。

暂不以 Rust winit 加 C ABI 包装作为交付方式：当前 Cone native 管线面向 C／C++，现有 async host 需要直接控制监听注销和退出顺序。引入 Rust 库构建、Rust 对象分发及另一套平台循环的适配会成为额外工作。此选择不要求以后把新平台逻辑写成 Scoop；平台实现语言不进入公共合同。

共享 `ApplicationOwner`／`WindowOwner` 持有事件与业务投递队列、typed id、窗口 phase、最后已公布的 metrics、dirty 状态和 native lease 数；native 后端持有真正的 NSWindow／wl_surface／X Window、协议配置与稳定事件存储。内部 C ABI 只需 create application、run／stop／wake、drain events、window control／snapshot、native handle acquire／release 与 teardown 等实际入口。原生主循环由各平台驱动，共享层不能假定每个平台都有可反复调用的 poll；平台状态不再复制一份 Job 或 managed 资源表。

Widgets、布局、状态重组、字体排版、剪贴板、菜单模型、文件对话框、无障碍树、绘制 API 和通用手势识别属于后续 GUI／平台库。窗口系统原生提供的触摸和触控板手势属于本库；应用自行识别的拖拽／长按等不在本库。Windows、移动端、Web、向已有外部窗口嵌入和任意宿主轮询入口不在首期实施范围。

## 3. 公共对象与运行入口

采用 `Application` 表示窗口系统主循环，避免与 I/O 的 `EventLoop` 同名。以下为主要目标签名，相关值类型与事件字段见规范草案和映射表；这些声明尚未实现。

```scoop
public class Application public constructor(options: ApplicationOptions = ApplicationOptions()) {
    public fun run(handler: ApplicationHandler)
    public fun proxy(): ApplicationProxy
}

public interface ApplicationHandler {
    public fun onEvent(application: ActiveApplication, event: ApplicationEvent)
}

public class ActiveApplication internal constructor(owner: ApplicationOwner) {
    public fun createWindow(attributes: WindowAttributes = WindowAttributes()): Window
    public fun monitors(): Array<Monitor>
    public fun primaryMonitor(): Monitor?
    public fun setControlFlow(flow: ControlFlow)
    public fun requestExit()
}

public class ApplicationProxy internal constructor(owner: ApplicationOwner) {
    public fun post(block: (ActiveApplication) -> Unit): Boolean
    public fun requestExit(): Boolean
}

public enum ControlFlow { Wait, Poll, WaitUntil(TimeMark) }

public class Window internal constructor(owner: WindowOwner) {
    public val id: WindowId
    public val state: WindowState
    public val metrics: WindowMetrics?
    public fun requestRedraw()
    public fun requestDestroy()
    public fun requestSize(size: LogicalSize): RequestStatus
    public fun setTitle(title: String)
    public fun supports(feature: WindowFeature): Boolean
}
```

每进程只创建一个 Application，run 只接受一次且必须在进程主线程、当前 Job 与活动 I/O EventLoop binding 外执行；不承诺 run 返回后再次运行。构造只建立 managed 状态，原生应用／显示连接在接受 run 后创建。`proxy()` 可在 run 前取得，投递在开始服务后执行；启动失败和关闭后投递返回 false，不保留闭包。

Window、ActiveApplication、Monitor 的原生活动方法只允许 owner 线程；跨线程使用 proxy，所有用户代码在内部锁外执行。只读 id、不可变事件／快照可跨线程保存，但不赋予操作原生窗口的权利。跨线程不提供同步 invoke，避免 UI 等 worker、worker 又等 UI 的互锁。

proxy.post 是异步 FIFO，跨线程以入队顺序为准；关闭会丢弃尚未执行的业务投递，true 只表示已接受。它在 run 的 Context 下执行，不捕获投递线程的动态 Context。proxy 解决 UI 消息投递，session.post 解决结构化任务入口，两者不各自实现一套任务调度。

ControlFlow 默认为 Wait；WaitUntil 只接受 `MonotonicTimeSource` 的 TimeMark，拒绝其他来源，避免把 Scoop 的民用 `Instant` 当成 Rust 的单调 Instant。Poll 是显式持续运行请求，允许占用 CPU；Wait 模式没有固定 tick。截止时间仅决定下一次唤醒，不代替显示器刷新节奏。[winit ControlFlow 基线](https://docs.rs/winit/0.30.13/winit/event_loop/enum.ControlFlow.html)。

普通入口的原生回调先复制数据，再在有限事件批次中调用 handler；同一 Application 不递归派发用户事件。需要立即返回给 OS 的询问，例如 AppKit 关闭询问、IME 状态查询、未来 Win32 hit-test，使用已发布的窗口策略／输入状态作同步回答，不持有 OS 借用数据等待异步用户回调。

首期只提供库自有 Application.run，以及该循环承载 asyncio 的集成。不把 `pumpOnce` 暴露为跨平台承诺；任意外部 NSApplication／现有 GUI toolkit 嵌入需要独立的平台扩展设计。这里的“支持 GUI 主循环”是实现并拥有真实窗口主循环，不是后台线程周期调用 poll。

## 4. 与 asyncio 的接入及 Windows 唤醒接缝

`scoop.window.asyncio` 提供 `Application.runWithAsyncio(handler: ApplicationHandler)` 扩展，以及仅在普通业务回调期间可用的 `currentApplicationScope(): CoroutineScope`。业务 handler／proxy 消息经 EventLoopSession.post 进入根作用域，`currentEventLoop()` 可用；scope.launch／async 的取消与资源寿命完全沿 asyncio。Exiting／Closing／Destroyed 属于收尾回调，不提供活动 scope，不能从中重新启动 I/O。

runWithAsyncio 在主线程、原生应用可服务后、首个业务事件前执行 attach；不会在已有 Job 中启动它。attach 失败关闭已初始化的窗口后端并将错误从 runWithAsyncio 抛出。原始 Application.run 不启动 asyncio，因此 Linux 创建窗口不要求 io_uring 或 Linux 6.1；选择集成入口才继承 asyncio 的运行要求。

集成层观察根 Job 的取消，把开始退出排入窗口内部控制队列；不能等到 onClosed 才通知窗口 Closing，否则子任务 finally 与窗口释放可能互相等待。取消订阅不执行窗口操作，只唤醒 owner；关闭订阅与其他 callback 一起退休。业务事件进入 session，收尾控制走窗口内部队列，不能因业务 post 已拒绝而丢弃。

NewEvents／AboutToWait 按外部事件或业务投递形成的一轮逻辑批次发送一次；用 session.post 交付这些通知产生的内部唤醒不再开一轮相同通知。纯 I/O／watch 维护唤醒也不自动制造新的 AboutToWait，避免 Wait 模式中“通知待等待 → post 唤醒 → 再通知”形成空转。

GUI 内部的 `WindowLoopHost` 实现宿主接口，但 ActiveApplication 不继承 EventLoopHost，也不公开描述符。近期采用以下**兼容扩展**，完整草案见[asyncio 规范第 12 节](../specs/SCOOP-ASYNCIO-SPEC.md)：

```scoop
public enum EventLoopWakeSource {
    PosixReadable(Int),
    WindowsEvent(Ptr<Unit>)
}
```

EventLoopHost 新增 `@Unsafe watch(source: EventLoopWakeSource, ready: () -> Unit): EventLoopWatch`。默认实现把 PosixReadable 转发给现有 watchReadable，其他来源报不支持；watchReadable 保留，默认报不支持。因此现有只实现 watchReadable 的宿主继续有效，Windows 宿主可只实现 watch。attach 改为调用 watch，其签名与 EventLoopSession API 不变；避免两个默认方法相互转发。

这里保证现有宿主的源码兼容；接口成员增加后按普通依赖图重建相关 `.slib`，不宣称新旧库对象布局可以混装。这不改变 runtime ABI 或构建缓存的更新规则。

WindowsEvent 专指 loop 所有的 manual-reset event HANDLE，不是 socket、HWND 或 completion port；host 只等待，不 ResetEvent／CloseHandle。生产者发布完成后置位，loop 处理完真实队列后在自身同步边界复位并复查，避免丢唤醒。Windows 可用消息等待与 event 共同驱动 owner，不能阻塞消息泵等待 I/O。相关系统边界见 [MsgWaitForMultipleObjectsEx](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-msgwaitformultipleobjectsex)。本期只确定该 variant 的合同，不创建 Windows event 或 I/O 实现。

watch/remove 的所有权、非内联注册、串行通知、回调退出后注销规则保持。Darwin GUI 适配在实际服务的 CFRunLoop modes 上监听 pipe，Linux 监听 eventfd；两者都不读取 loop 的通知载荷。窗口消息唤醒和 I/O 完成分别存储真实队列，可以合并通知，不能依赖“一次唤醒等于一个事件”。

第三方挂起点仍可能从其他线程恢复。需要修改窗口时回到 ApplicationProxy.post；需要继续有父任务的挂起工作时用 session／集成 scope 的入口。本设计不修改协程恢复语义或增加通用 dispatcher。

## 5. 窗口属性、DPI 与平台差异

WindowAttributes 使用普通属性／值类型，包含标题、内容尺寸约束、可见性、装饰、可调整大小、初始位置／全屏、透明、主题、窗口级别、按钮集合、图标、初始光标及激活请求。创建时取得独立快照，之后修改原属性对象或图像数组不改变已创建窗口；ApplicationOptions 同样在构造时取快照。无平台支持的必需属性使创建失败；主题／聚焦等建议性属性只表示已请求，不能把请求值伪装成 OS 已接受的状态。各方法分组见映射表。

`RequestStatus` 为 Applied／Requested／Unsupported。Applied 只用于可立即确认的设置；Requested 由后续事件／快照确认；Unsupported 不产生副作用。非法参数用通常参数异常；平台失败用 WindowException，保留 operation、错误类别、backend 和可选原生 code。supports 是当前能力提示，不代替实际调用的错误处理；运行环境失联、权限或 compositor 状态仍可能变化。

公共类型预先覆盖 AppKit、Wayland、X11、Win32 的语义交集与差异。窗口级别用 Normal／AlwaysOnTop／AlwaysOnBottom；全屏用 Windowed／Borderless(monitor?)／Exclusive(videoMode)。WindowFeature 包含定位、最小化、最大化、独占全屏、光标锁定／限制／移动、装饰、按钮、主题、blur、content protection、原始设备事件、拖放与 IME 等实际功能；Windows 新后端映射到现有类型，不追加“Windows 专用窗口事件”来表达已有功能。

窗口内部坐标以左上为原点，x 向右、y 向下。LogicalPosition／LogicalSize／LogicalRect 使用有限 Double；PhysicalSize 使用非负 Int，表示 drawable 像素尺寸；PhysicalPosition 保留小数，适合物理像素中的指针位置。尺寸非负、scaleFactor 为有限正值；render target 遇到零尺寸暂停取得／提交帧。

`WindowMetrics` 同时携带逻辑内容大小、实际 drawable 大小与有效 scaleFactor；一次 GeometryChanged 发布一个完整快照，避免上层读到新 DPI 配旧尺寸。首个原生配置前 metrics 为 None；Wayland createWindow 返回后尚需等待 configure，不虚构可绘制尺寸。请求尺寸与实际尺寸分开，后者以原生配置为准。

全局位置单独用 `DesktopPosition`／`DesktopRect`，表示当前显示系统的桌面坐标；不能用某一个窗口的 scaleFactor 在混合 DPI 桌面上进行全局换算。桌面位置与尺寸查询允许 None，Wayland 不伪造全局窗口位置或 primary monitor。窗口内物理／逻辑换算只在同一 metrics 快照中进行，像素分配尺寸向上取整。

MonitorId、WindowId、DeviceId、TouchId 均为不同值类型，库内分配且在本次 Application 内不复用；不暴露 XID、指针或数组下标作为公共 id。Monitor 快照可保存，热拔插后控制操作报告 unavailable；名称、位置、刷新率等未知时为 None，刷新率使用可选整数毫赫兹，不能假定 60 Hz。

## 6. 输入与 IME

键盘事件分别表达 PhysicalKey、LogicalKey、KeyLocation、Pressed／Released、repeat、modifiers 与 synthetic 标志。LogicalKey 区分 Named／Character(String)／Dead／Unidentified；物理键使用既有标准键位集合及 Unidentified(NativeKeyCode) 退路。NativeKeyCode 预先区分 Darwin、XKB 与 Win32 编码，不能把平台 scan code 当 Unicode。modifier 包含 Shift／Control／Alt／Super、锁定状态和可取得的左右侧信息。[winit KeyEvent 参照](https://docs.rs/winit/0.30.13/winit/event/struct.KeyEvent.html)。

文字插入只有 `TextInputEvent.Commit` 一条入口：普通可打印键、dead-key compose 和 IME 都归一到该事件；LogicalKey 用于快捷键和键位语义，不再隐式插入第二份字符。编辑器不能同时从按键与 IME 重复提交文本。失焦时撤销 active preedit、清理按下状态；需要合成 release 时标记 synthetic，不伪造真实输入时间。

输入法基础 API 为 `setImeAllowed`、`setImePurpose`、`setImeCursorArea`，事件为 Enabled／Preedit／Commit／Disabled。[winit IME](https://docs.rs/winit/0.30.13/winit/event/enum.Ime.html)使用 preedit 字节位置；Scoop 同样统一 UTF-8 byte range，并要求位于字符边界。平台层转换 AppKit／Win32 的 UTF-16 索引与 Wayland 的字节索引；无效 native 范围丢弃该范围并重新同步组合状态，不能越界或直接 panic。

为满足原生文本服务的同步询问，再提供 `updateTextInput(state: TextInputState)`：不可变的 surroundingText、selection、可选 marked range、caret rect 及修订号。平台使用已发布 native 副本回答询问，不在 NSTextInputClient／未来 TSF 的同步回调中挂起。组合的替换范围或删除 surrounding 请求带来源修订号；编辑器状态已变化时放弃旧编辑并同步新快照，不把旧 offset 应用于新文本。password 目的不得向平台提供密码正文，仍可提供候选窗位置和必要长度信息。

鼠标区分按键、进入／离开、窗口内位置、相对设备位移。滚轮保留 LineDelta 与 PixelDelta，明确相位和惯性信息；不能猜测每行固定像素数。触摸携带独立 contact id、phase、位置和可选 pressure；原生 pinch／rotation／pan／double-tap／pressure 保留事件来源与单位。不支持的原生手势不靠周期采样伪造。

raw device 事件默认只在应用有焦点时请求，设备数据没有稳定身份时 DeviceId 为 None；不同平台不保证每个物理设备都可区分。光标 None／Confined／Locked 分开表达；Locked 不隐含可见性修改，鼠标 warp 不等于相对输入。Wayland 抓取、激活、移动／调整大小所需的原生 input serial 由后端保留；beginMove／beginResize 只在当前有效用户输入派发中接受，不能用任意历史点击模拟权限。

## 7. 重绘与渲染边界

requestRedraw 只置 dirty 并唤醒，重复请求可合并。RedrawRequested 在窗口可绘制时发出；绘制回调内再次请求的重绘留到下一轮，不能递归绘制。遮挡、隐藏、零尺寸期间保留 dirty，恢复可绘制后补通知，不能为不可见窗口持续忙等。

`prePresentNotify()` 告诉窗口后端即将提交帧，用于 Wayland frame callback 等节流；它不是实际 present，也不表示 GPU 完成。渲染层拥有 Metal／Vulkan／OpenGL 的 device、swapchain、surface、提交和 vsync。窗口层不把 io_uring timer 当显示时钟，不内置 Skia surface 或强制渲染线程。[winit 绘制边界](https://docs.rs/winit/0.30.13/winit/)。

`scoop.window.native` 提供 `@Unsafe Window.acquireNativeWindow(): NativeWindowLease`。lease 保活 Window 和 display connection；`handle` 为带类型的 NativeWindowHandle，首版即声明 AppKit(nsWindow, nsView)、Wayland(display, surface)、Xlib(display, windowId, visualId, screen)、Win32(hwnd, hinstance) 四种载荷。XID 使用整数，指针／HANDLE 使用 Ptr<Unit>；不以一个 Long 统一所有句柄。

lease 的 handle 查询在 owner 上执行；显式 close 可跨线程，原子封闭该借用后把实际释放投递到 owner 的内部收尾队列，不在调用线程执行 GUI API。该队列在停止业务 proxy 投递后仍然可用，避免后台 GPU 完成回调无法释放窗口；重复 close 为空操作，调用后不得继续使用 handle。不能从 GC release 启动这条异步路径。Skia／GPU 包装持有 lease，销毁依赖原生窗口的 surface 并结束有关 GPU／native 借用后再释放。窗口进入 Closing 后不能取得新 lease，已有 lease 阻止库主动销毁原生窗口；lease 不保证外部显示服务器永不失联，也不授权跨线程操作原生对象。渲染层需要后台线程时遵守对应图形 API 的线程规则。

平台只能查询自己的类型化 handle；Windows variant 不提供实现但已在签名中存在。普通 GUI 消费者不对这个 enum 分支；native renderer 本来就按平台实现，因此以后增加 Win32 renderer 无需改窗口应用 API。

## 8. 生命周期与退出顺序

Window 状态为 PendingConfigure／Open／Closing／Destroyed。CloseRequested 表示用户意图，不自动销毁；应用可忽略、确认后 requestDestroy，或先保存数据。requestDestroy 幂等、非阻塞，进入 Closing 并排队 Closing 通知，停止新输入／重绘与句柄借用。等 Closing 已交付、现有事件派发栈与 native lease 均结束后销毁原生对象，再发一次 Destroyed；Destroyed 后只保留 id／终态快照，活动操作失败。

ApplicationOptions 的最后窗口策略为 KeepRunning 或 Exit，默认 KeepRunning，与 winit 的显式退出行为一致。macOS 关闭最后窗口不被硬编码成退出，Windows 也不硬编码成相反行为。系统退出请求携带原因，由相同应用事件表达；真正的强制终止／显示服务器消失可能不允许 veto 或完整 cleanup，不承诺从被 OS 强制杀死中恢复。

正常 requestExit 或未捕获 handler 异常按以下顺序收尾：

1. 停止接受业务 proxy 投递与创建窗口，将所有剩余窗口转入 Closing，等同分别调用 requestDestroy；当前用户派发返回后，在主线程交付一次 Exiting 和各窗口的 Closing 通知，让渲染包装开始释放。
2. 集成模式调用 EventLoopSession.requestClose，沿已有根取消等待所有子任务、借用和 I/O 清理；继续服务平台事件、watch、native cleanup 与 GPU 回调。纯窗口模式没有这一步。
3. native lease 归零且派发退出的窗口可销毁；逐一发出最终 Destroyed。退出期间这些最终通知直接在 owner 的收尾 Context 中执行，不通过已经关闭的 session mailbox，不提供活动 Job。
4. 等所有窗口销毁、集成 session 的 onClosed、全部 watch 移除以及 native callback lease 退休。一个条件先完成不替代其他条件。
5. 断开 display／停止 AppKit 主循环，释放应用回调，run 返回；首个业务失败与清理失败沿 ResourceUseException 组合后抛出。

步骤 2、3 可以交错推进，但实际退出必须同时满足所有条件。Closing／Exiting 之后的用户异常只记录失败并继续收尾，不重入相同通知；某个 cleanup 失败也不能跳过其他窗口。泄漏的 native lease 或不返回的外部工作可能使退出无法完成，不能假称关闭成功或由 GC 强拆仍使用的窗口。

native 事件只存稳定 native payload 与数值 id，进入 managed 前复制文本／路径／坐标。所有 user callback 通过已有 ForeignCallback，捕获异常并通知应用收尾，禁止异常跨 Objective-C／C／Win32 栈展开。main 返回前必须退出注册和活动 lease；AppKit 自身需要的 native 回调不调用 managed 时也按对应 C／ObjC 对象寿命释放。

## 9. macOS 后端

用 AppKit 的 NSApplication／NSWindow／NSView／delegate／NSTextInputClient 承载真实桌面行为，全部在主线程；每轮原生派发有 autorelease pool。run 通过 NSApplication 的正常循环服务事件，退出用可返回的 stop 与必要唤醒，不调用直接结束进程的 terminate 或永不返回的 dispatch_main。[Apple 文本输入协议](https://developer.apple.com/documentation/appkit/nstextinputclient)。

async host 使用 libdispatch READ source 观察 pipe，再 signal／wake 一个安装到 AppKit 所服务 modes 的 CFRunLoopSource，由该 source 在 owner 上调用 ready。等待 owner 消费期间暂停 READ source 的重复通知，处理后按有效状态恢复；取消时平衡暂停并等待 cancel handler，再在 owner 注销 CFRunLoopSource。这个窗口专用 host 满足既有 watch/remove 合同；MainDispatchHost 继续适用于其原有主队列服务场景。必须以真实拖动、菜单和模态验证 common／tracking／modal modes 的推进，不能只靠 mode 名称推断。

窗口位置／内容尺寸、backing scale、屏幕切换、全屏转换、occlusion、主题由原生通知生成。NSView 只提供可供 renderer 使用的宿主，不由窗口库拥有 Metal device 或 Skia；是否配置 CAMetalLayer 由渲染适配管理，并在窗口关闭前恢复／释放其持有状态。

需要正式支持 Darwin native `.m`：当前[语言规范 12.2.1](../specs/SCOOP-SPEC.md)只接受 C／C++ 后缀，不能用 c_flags 绕过输入语言检查。实施时先修订该节与[实现规范 2.7](../specs/SCOOP-IMPL-SPEC.md)，再让显式选择的 `.m` 交给所选 Apple Clang，以 ARC 编译，提供独立 objc_flags；`.c`、generated-C 和 runtime 保持原语言。条件筛选先行，非 Darwin 选中 `.m` 明确诊断；本期不扩展 `.mm` 或 Scoop 的 Objective-C 语法。

framework 链接已有合同可复用；仍需实际验证本仓库 Mach-O linker 对 Objective-C 元数据、类／selector 引用、ARC 外部符号和 AppKit framework 的正常链接，不能只增加后缀就宣称可用。使用 Objective-C 薄层比大量手工 objc_msgSend cast／动态注册类更适合维护；不增加 runtime 调度 ABI。

## 10. Linux 后端

### 10.1 选择与公共边界

Linux 自动模式优先连接有效的 Wayland 桌面，再在建立任何窗口前尝试 X11；可通过 `scoop.window.platform.linux` 显式选择 WaylandOnly／X11Only 及 display name。明确指定的后端失败不得静默切换；开始运行后失联也不能把既有窗口迁到另一显示服务器。该 Linux 专用选项不放进公共 ApplicationOptions。

`WindowLoopHost` 把窗口连接、app proxy 通知、asyncio 通知、键重复／ControlFlow deadline 组合在一次原生等待中。只有它读取显示连接；asyncio 只处理自己的 eventfd。所有原生等待走 NativeSafe，空闲不轮询；已有批次未处理或即将关闭时不进入阻塞等待。

### 10.2 Wayland

使用 libwayland-client；协议优先 xdg-shell，按 compositor 宣告协商 text-input-v3、fractional-scale／viewporter、pointer-constraints／relative-pointer、pointer-gestures、xdg-activation、xdg-decoration 等。协议缺失通过能力和 Unsupported 表达，不承诺全局定位、强制聚焦、独占显示模式或任意鼠标 warp。

等待严格执行 dispatch_pending → prepare_read → flush → poll → read_events／cancel_read，再 dispatch；prepare 成功后每条退出路径必须 read 或 cancel。flush 返回 EAGAIN 时临时监听可写；其他 fd 唤醒也要取消已准备的读取。禁止直接用 io_uring recv 消费 wl_display fd，或让多个 reader 争用同一协议队列。[Wayland client 合同](https://wayland.freedesktop.org/docs/html/apb.html)。

首次 xdg configure／ack 完成前不提交绘制 buffer。resize 配置、scale、surface role 和 frame callback 由后端管理，renderer 在已公布 metrics 下分配像素 buffer；fractional scaling 用协商后的 viewport／buffer-scale 规则，不把 scaleFactor 强制取整。renderer 可管理自己的事件队列，但须遵守 libwayland 的共享 display 读取协议，不能自行裸读 fd。

键位和 compose 用 libxkbcommon；IME 用 text-input-v3 的批次与 done 序列，版本／能力不足时保留普通键输入并明确 IME 不可用。客户端装饰使用 libdecor 及匹配的 C 装饰插件；若 compositor 不提供服务端装饰且客户端装饰依赖不可用，要求装饰的窗口创建失败，不无提示地变成无边框。协议和装饰依赖版本在实施开始时固定，并记录许可证。

### 10.3 X11

Xlib 持有唯一 Display 与事件读取；XKB／libxkbcommon 归一键位，XIM／XIC 和 UTF-8 lookup 负责输入法，XInput2 负责可用的 raw／touch，XRandR 负责显示器／模式／热插拔，Xcursor 负责光标。不能混用 Xlib 与 XCB 作为同一连接的独立事件消费者。[libxkbcommon 官方说明](https://xkbcommon.org/doc/current/)。

实现 WM_DELETE_WINDOW、EWMH 状态、configure、焦点与 XDND 文件拖放。尺寸、置顶、激活和全屏请求需等待 WM 事实确认；窗口管理器可以忽略建议。X11 error handler 的进程级属性必须由本库自有 Application 边界管理，捕获请求相关错误，不能把异步 X error 当一次 setter 已成功，也不能静默覆盖宿主的既有 Xlib 策略。

### 10.4 依赖与发行

Linux 首期按真实桌面依赖动态链接，GNU／musl 各自使用兼容 headers、库及 libdecor／XIM 模块；不能让 musl 程序加载 glibc 插件。完整静态桌面分发不是首期验收承诺；native 声明与显式链接模式冲突走现有链接诊断。桌面库不进入无窗口程序的依赖闭包。

生成的 Wayland 协议 C 源码在维护／发行准备阶段用固定 scanner 版本生成并作为普通输入携带；消费预编译 Cone 不临时联网、不自动运行依赖安装器。缺失库和 display 按普通构建／运行错误报告。是否允许无显示器环境启动由明确的测试宿主决定，不用一个假窗口后端冒充桌面实现。

## 11. Windows 以后接入时不改什么

未来实现只增加 `native/windows` 与对应 Scoop 平台扩展、构建 target 集成；普通应用仍使用同一 Application／Window／事件／输入／显示器／runWithAsyncio API。下面的接缝本次就确定：

| 将来的 Windows 实现 | 已确定的公共表达 |
| --- | --- |
| HWND 与消息队列，WM_CLOSE／WM_DESTROY | WindowId、CloseRequested、Closing、Destroyed |
| 消息泵与 manual-reset event 联合等待 | EventLoopWakeSource.WindowsEvent、同一 watch/remove |
| WM_DPICHANGED、per-monitor DPI 与建议 rect | 完整 WindowMetrics、GeometryChanged、请求与实际值分离 |
| 虚拟键／scan code／WM_CHAR／IME 或 TSF | PhysicalKey、LogicalKey、UTF-8 TextInputEvent／TextInputState |
| WM_POINTER／Raw Input／cursor clipping | Pointer／Touch／DeviceEvent 与 Confined／Locked 能力 |
| Win32／DXGI rendering surface | NativeWindowHandle.Win32 与同一 lease 寿命 |
| 系统关机、激活限制、DWM 功能差异 | 退出原因、RequestStatus、WindowFeature／WindowException |

Windows DPI 推荐区域与消息处理的实际语义见 [WM_DPICHANGED](https://learn.microsoft.com/en-us/windows/win32/hidpi/wm-dpichanged)。此处仅检验设计没有依赖 fd、POSIX errno、XID 或 AppKit 的单一表示，不实现 Windows 消息常量、ffi、TSF／COM、DWM 或 I/O backend。

Windows 的编译器 target、runtime、platform 路径／错误／线程与异步 I/O 支持仍是另一个平台移植任务。它可以改变 native 实现和产物，但不要求窗口消费者改方法名、参数或事件匹配。平台专有附加能力放 `platform.windows`；本设计不承诺任何未来新功能永远不增加 API。

## 12. 实施顺序与验收

1. 先冻结规范草案中的公共语义；落实必要 `.m` native 管线和兼容 wake-source 扩展，现有 async host 与调用方回归保持通过。
2. 实现共同对象、typed id、proxy、事件归一、metrics、关闭及 native lease；Darwin 完成真实窗口创建、输入、重绘、退出和 asyncio 联动。
3. 完成 Wayland 的 configure、DPI、键盘／IME、装饰、frame、拖放和协议能力差异；再实现 X11 对应行为。GNU／musl 共用同一 Scoop 层，不复制测试实现。
4. 按映射表交付光标、touch／gesture／raw device、monitor、全屏、主题和可选属性；每项“平台不支持”均有真实 capability 依据和明确结果。
5. 完成配套示例和三目标定向验收后才把文档改为已实现。没有 Windows 构建或平台专测前，不标记 Windows 可用。

纯值／归一规则、状态、错误与普通 API 行为用所属 Cone 的 scoop.test；真实窗口与 callback 测试优先使用主线程测试程序，必要的 OS 进程控制／ABI／native 编译能力才用少量 fixture。目标环境至少包含真实 macOS、Wayland compositor、X11 WM，并分别使用 GNU／musl 库；Xvfb 或 headless compositor 可覆盖自动化行为，但不能替代实体显示器、IME 与 AppKit 的手工验证。

重点组合：多个窗口与任务并存；关闭／外线程投递竞争；移动 GC 下事件文本和 ForeignCallback；关闭中 surface lease 未释放；用户 handler／清理抛错；菜单、拖动、模态期间 timer／网络推进；混合 DPI 跨屏与热插拔；零尺寸／隐藏窗口重绘；中文／日文组合、emoji／UTF-16 range、dead keys、重复输入与失焦；Wayland flush EAGAIN／prepared-read 取消；X11 WM 忽略请求；display 断开；退出时 I/O、window、watch 各自先完成的顺序。

Windows 可移植性在本期通过 API 与表示审查确认：公共窗口参数没有 fd／HWND／errno，native 层四种 handle 载荷完整，wake source 有完整 Windows 合同，事件和输入没有 POSIX-only 假设。不创建只供测试的 Windows 假实现，不把编译几个空 stub 当作 Windows 已验收。

性能只记录空闲 CPU／唤醒、输入到 handler、redraw 合并、I/O 与 UI 同时繁忙时的延迟及关闭耗时；主循环不引入通用预算或计费。渲染性能与完整 GUI 基准留到实际 renderer 接入。本次没有实现代码、运行新测试或发布任何窗口库产物。
