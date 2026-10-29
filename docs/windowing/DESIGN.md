# 桌面窗口与事件库设计

状态：基础架构与 macOS GUI 骨架已实现，正在实施 Windows GUI 骨架。更新：2026-10-30。Windows 阶段复用两个普通 Cone，交付与 macOS 同级的窗口、属性、基础输入／IME、显示器、native lease、renderer 和 asyncio；Linux Wayland／X11 及其他后端仍留待后续。实现、验收边界与硬件交互限制见 [PROGRESS.md](PROGRESS.md)，使用方式见[窗口库 README](../../sysroot/src/scoop.window/README.md)。

本轮按 winit 的职责搭建可供 GUI 使用的骨架：真实窗口／主循环、显式生命周期与 proxy、基础窗口属性、DPI／显示器快照、键盘／指针／滚轮／文本输入、native lease、renderer 和 asyncio 接入。winit 的完整能力地图用于确定边界和后续扩展方向，不要求本轮复制它的全部平台扩展。全屏控制／显示模式切换、全局原始设备监听、触摸／高级手势、光标捕获、文件拖放及平台专有的复杂窗口交互分批补充；未实施项不以 Unsupported 伪装成平台不支持。控件、布局、菜单体系和应用编辑器仍由上层 GUI 实现。

目标是为 Skia／HarfBuzz 上层 GUI 提供 winit 对应的窗口、输入、显示器与主循环能力。公共 API 使用 Scoop，平台实现使用成熟的系统／协议库。功能基线取 [winit 0.30.13](https://docs.rs/winit/0.30.13/winit/)，另参考 0.31 beta 的演进方向，不追随其未稳定 API；逐项范围见 [WINIT-MAPPING.md](WINIT-MAPPING.md)，目标合同见[窗口库规范草案](../specs/SCOOP-WINDOW-SPEC.md)。

Scoop 尚未发布任何版本。语言、库 API、runtime ABI、native 产物组织和构建流程均可按实际需求调整；不为旧草案保留兼容别名、转发层、双轨实现或预先冻结未来平台的参数与事件。每次变更先更新规范，再同步实现、调用方和必要重建。同一次构建的类型、格式、目标与 ABI 相容性仍是正确性要求。取消历史兼容负担不扩大本次工作范围，跨流水线的新能力仍须独立设计。

## 1. 已落地 async backend 的接入结论

本设计核对主线源码，实施期间已合入 Windows 基础能力的 [b0de037002](https://github.com/windoze/scoop/tree/b0de037002)。下表区分已有机制与窗口层的责任：

| 已有实现 | 可复用的事实 | 窗口层的责任 |
| --- | --- | --- |
| [PlatformEventLoop](../../sysroot/src/scoop.asyncio/src/platform_event_loop.scoop) | 单次 run／attach；attach 捕获 Context；初始化失败回滚 | 从主线程、当前 Job 外安装，不在窗口事件内再启动 loop |
| [LoopDriver](../../sysroot/src/scoop.asyncio/src/loop_drive.scoop) | 有限批次、非阻塞 pump、阻止递归进入 | 宿主保留原生输入和重绘的执行机会 |
| [LoopMailbox](../../sysroot/src/scoop.asyncio/src/loop_mailbox.scoop) | post 经根 continuation 执行；取消交给 owner | 直接复用任务、Context 和取消规则，不另建 GUI Job |
| [EventLoopSession](../../sysroot/src/scoop.asyncio/src/event_loop_session.scoop) | I/O 排空后注销 watch，再销毁并通知 onClosed | 在 onClosed 之前维持原生主循环 |
| [EventLoopHost](../../sysroot/src/scoop.asyncio/src/host/host.scoop) | 已迁入 Windows 分支的 EventLoopNotification 与 watch；remove 等 ready 退出 | 提供真实窗口宿主适配 |
| [MainDispatchHost](../../sysroot/src/scoop.asyncio/platform/darwin/main_dispatch_host.scoop) | 主队列通知及 ForeignCallback 保活／异常观察 | 继续验证菜单跟踪、拖动和模态，不把 CFRunLoop 测试当成完整 AppKit 验收 |

[实施记录](../asyncio-backends/PROGRESS.md)已报告 Darwin／GNU／musl、Linux 6.1、取消竞争和定向 moving GC 验收。本次复核 Darwin 常规 release 测试及真实 AppKit 模态／菜单 tracking，未重跑 Linux 或完整 GC stress 专测。[性能记录](../asyncio-backends/PERFORMANCE.md)是窗口接入前的基线；本阶段不预设新窗口层具有性能优势。

当前工作是窗口系统集成。主线已实现 IOCP、manual-reset event 通知及对应 attach 路径；窗口集成直接采用这组接口，不再另建 wake-source enum 或旧宿主兼容层。接口、实现和实际调用方已同步，见第 4 节。平台基础的 I/O 验收不等于对应窗口后端已经完成。

## 2. 分层与交付边界

新增两个普通 Cone：

- `scoop.window`：依赖 core／std／platform，提供应用主循环、窗口、事件、显示器、输入和受控 native 句柄借用；不依赖 asyncio、Skia 或 HarfBuzz。
- `scoop.window.asyncio`：依赖 window／asyncio，把已有 PlatformEventLoop 附着到窗口宿主，协调应用退出与 I/O 收尾。没有第二套 Job、cancel、join 或 native I/O 实现。

选择“移植能力与平台语义”，不把 Rust ABI 或 winit 类型放进 Scoop API。原生层分别使用 AppKit、Win32／IMM32、libwayland-client／wayland-protocols／libxkbcommon／libdecor、Xlib／XInput2／XRandR／Xcursor／XKB／XIM。引用或改写 winit 的键映射、算法和平台规则时保留相应许可证与来源。窗口协议、键盘映射、输入法与装饰优先复用这些成熟库，不自行实现 wire protocol 或完整输入法。

本期采用上述原生后端，不新增独立的 winit Rust staticlib。现有 [text backend](../../sysroot/native/text_backend/Cargo.toml) 已包含 Rust 标准库及静态依赖；第二个独立 staticlib 会重复打包这些部分，并可能产生符号和运行库链接冲突。最终程序的按需抽取／裁剪不能作为解决多 staticlib 组合的合同。[Rust 链接规范](https://doc.rust-lang.org/reference/linkage.html#mixed-rust-and-foreign-codebases)给出的组合方式是先将 Rust 库汇总到一个 staticlib，再交给外部链接器。

把 Rust 依赖作为 Cone 可独立声明的正式能力，再为最终程序合成一个 Rust staticlib，涉及四项独立工作：Cone 的 Rust 依赖声明；PM／driver 汇总实际依赖并生成顶层 crate；Cargo 构建及依赖更新检测；在最终链接前完成统一构建并交付 native 输入。更新检测还需明确与 Cargo 缓存、lockfile、指纹的边界，不能未经设计依赖其内部格式。现有 text backend 的固定维护构建不等于具备这套全项目能力。手工把两个固定后端并入一个归档也不解决普通 Cone 的独立声明和按需系统库依赖问题。

项目未发布，允许以后调整这些边界；当前窗口任务不承担这项跨流水线改造，也不增加项目级 Cargo 合成阶段。原生路线直接满足宿主监听注销、文本快照／编辑批次和退出顺序的控制需求，同时复用成熟系统／协议库。winit 继续作为能力和平台规则的参考，不维护第二套并行窗口实现。

共享 `ApplicationOwner`／`WindowOwner` 持有事件与业务投递队列、typed id、窗口 phase、最后已公布的 metrics、dirty 状态和 native lease 数；native 后端持有真正的 NSWindow／HWND／wl_surface／X Window、协议配置与稳定事件存储。内部 C ABI 提供 create application、run／stop／wake、drain events、window control／snapshot、通知监听注册／注销、native handle acquire／release 与 teardown 等实际入口。跨 Cone 的窗口／asyncio 连接通过第 4 节列出的公开 native 包接口完成，不访问另一 Cone 的 internal owner。原生主循环由各平台驱动，共享层不能假定每个平台都有可反复调用的 poll；平台状态不再复制一份 Job 或 managed 资源表。

Widgets、布局、状态重组、字体排版、剪贴板、菜单模型、文件对话框、无障碍树、绘制 API 和通用手势识别属于后续 GUI／平台库。窗口系统原生提供的触摸和触控板手势属于本库；应用自行识别的拖拽／长按等不在本库。移动端、Web、向已有外部窗口嵌入和任意宿主轮询入口不在首期实施范围。

## 3. 公共对象与运行入口

采用 `Application` 表示窗口系统主循环，避免与 I/O 的 `EventLoop` 同名。以下为主要接口分组，相关值类型与事件字段见窗口规范和映射表；当前交付与测试见实施记录。

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
}
```

每进程只创建一个 Application，run 只接受一次且必须在进程主线程、当前 Job 外执行；不承诺 run 返回后再次运行。基础 window Cone 不依赖 asyncio，也不检查其 Context 类型；runWithAsyncio 另由 PlatformEventLoop.attach 拒绝活动 I/O EventLoop binding 中的嵌套启动。构造只建立 managed 状态，原生应用／显示连接在接受 run 后创建。`proxy()` 可在 run 前取得，投递在开始服务后执行；启动失败和关闭后投递返回 false，不保留闭包。

Window、ActiveApplication、Monitor 的原生活动方法只允许 owner 线程；跨线程使用 proxy，所有用户代码在内部锁外执行。只读 id、不可变事件／快照可跨线程保存，但不赋予操作原生窗口的权利。跨线程不提供同步 invoke，避免 UI 等 worker、worker 又等 UI 的互锁。

proxy.post 是异步 FIFO，跨线程以入队顺序为准；关闭会丢弃尚未执行的业务投递，true 只表示已接受。它在 run 的 Context 下执行，不捕获投递线程的动态 Context。proxy 解决 UI 消息投递，session.post 解决结构化任务入口，两者不各自实现一套任务调度。

ControlFlow 默认为 Wait；WaitUntil 只接受 `MonotonicTimeSource` 的 TimeMark，拒绝其他来源，避免把 Scoop 的民用 `Instant` 当成 Rust 的单调 Instant。Poll 是显式持续运行请求，允许占用 CPU；Wait 模式没有固定 tick。截止时间仅决定下一次唤醒，不代替显示器刷新节奏。[winit ControlFlow 基线](https://docs.rs/winit/0.30.13/winit/event_loop/enum.ControlFlow.html)。

普通入口的原生回调先复制数据，再在有限事件批次中调用 handler；同一 Application 不递归派发用户事件。需要立即返回给 OS 的询问，例如 AppKit 关闭询问、IME 状态查询、未来 Win32 hit-test，使用已发布的窗口策略／输入状态作同步回答，不持有 OS 借用数据等待异步用户回调。

首期只提供库自有 Application.run，以及该循环承载 asyncio 的集成。不把 `pumpOnce` 暴露为跨平台承诺；任意外部 NSApplication／现有 GUI toolkit 嵌入需要独立的平台扩展设计。这里的“支持 GUI 主循环”是实现并拥有真实窗口主循环，不是后台线程周期调用 poll。

## 4. 跨 Cone 的 asyncio 接入

`scoop.window.asyncio` 提供 `Application.runWithAsyncio(handler: ApplicationHandler)` 扩展，以及仅在普通业务回调期间可用的 `currentApplicationScope(): CoroutineScope`。业务 handler／proxy 消息经 EventLoopSession.post 进入根作用域，`currentEventLoop()` 可用；scope.launch／async 的取消与资源寿命完全沿 asyncio。Exiting／Closing／Destroyed 属于收尾回调，不提供活动 scope，不能从中重新启动 I/O。

runWithAsyncio 在主线程、原生应用可服务后、首个业务事件前执行 attach；不会在已有 Job 中启动它。attach 失败关闭已初始化的窗口后端并将错误从 runWithAsyncio 抛出。原始 Application.run 不启动 asyncio，因此 Linux 创建窗口不要求 io_uring 或 Linux 6.1；选择集成入口才继承 asyncio 的运行要求。

`WindowLoopHost` 由 `scoop.window.asyncio` 实现，基础 window Cone 只提供真实宿主操作。`scoop.window.native` 的公开接入合同见[窗口规范第 8 节](../specs/SCOOP-WINDOW-SPEC.md#8-asyncio-集成)：

- `Application.runAttached(handler, attach)` 与 run 共用单次入口；在原生宿主可服务后把具体 `ApplicationHost` 交给 attach，取得 `ApplicationAttachment`。此低层入口为 Unsafe，供当前集成 Cone 使用。
- `ApplicationAttachment.post` 将业务闭包交给 session.post，并在执行时绑定应用 scope；`requestClose` 转交 session.requestClose。窗口层不认识 Job 以外的 asyncio 类型，也不调用其他 Cone 的 internal 方法。
- `ApplicationHost` 提供 owner 查询、对应平台的 `watchReadable`／`watchWindowsEvent` 及 ApplicationWatch.remove。WindowLoopHost 将它们适配为 asyncio 的 watch／remove；注册、派发和退休遵守同一借用合同。
- `ApplicationHost.postControl` 在业务 proxy 关闭后继续服务根取消和清理控制；`complete(failure)` 由 session.onClosed 在 owner 调用一次。即使应用没有窗口，窗口循环也要等本次附着完成、watch 注销及已接受控制工作完成后才停止。

这些入口只连接实际窗口宿主和当前 asyncio 适配器；没有适配器注册表、通用服务容器或任意 toolkit 嵌入框架。attach 返回前失败时回滚已取得资源；业务 post 被拒绝时开始收尾，不继续运行一个没有业务接收者的应用。

集成层观察根 Job 的取消，把开始退出排入窗口内部控制队列；不能等到 onClosed 才通知窗口 Closing，否则子任务 finally 与窗口释放可能互相等待。取消订阅不执行窗口操作，只唤醒 owner；关闭订阅与其他 callback 一起退休。业务事件进入 session，收尾控制走窗口内部队列，不能因业务 post 已拒绝而丢弃。

NewEvents／AboutToWait 按外部事件或业务投递形成的一轮逻辑批次发送一次；用 session.post 交付这些通知产生的内部唤醒不再开一轮相同通知。纯 I/O／watch 维护唤醒也不自动制造新的 AboutToWait，避免 Wait 模式中“通知待等待 → post 唤醒 → 再通知”形成空转。

采用 Windows 分支已实现的通知类型及 `EventLoopHost.watch(notification, ready)`。ActiveApplication 不继承 EventLoopHost，普通窗口 API 不公开描述符；目标通知合同与主线迁移位置见[asyncio 规范 11.3](../specs/SCOOP-ASYNCIO-SPEC.md#113-宿主接入合同)：

```scoop
public enum EventLoopNotification {
    ReadableDescriptor(Int),
    WindowsEvent(Ptr<Unit>)
}
```

接口直接使用 `@Unsafe watch(notification: EventLoopNotification, ready: () -> Unit): EventLoopWatch`。迁入主线时删除 asyncio 旧 watchReadable 声明，修改 MainDispatchHost、其他实际宿主、attach 和测试；没有默认转发、兼容别名或第二个通知 enum。窗口 native 包的 watchReadable 是实际 POSIX 监听入口，不是旧 asyncio 接口的兼容方法。需要重建的库按普通依赖更新处理。

WindowsEvent 指 loop 所有的 manual-reset event HANDLE；host 只监听，不 ResetEvent／CloseHandle。分支的 IOCP 后端已在同一锁边界协调真实队列发布、SetEvent 和 owner 复位。Windows 窗口宿主复用这一通知，不重新实现 IOCP 或完成队列。

外层 [MsgWaitForMultipleObjectsEx](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-msgwaitformultipleobjectsex) 联合等待之外，Windows host 还用原生 wait 回调把 event 就绪转为 owner HWND 消息，使系统拖动、调整大小和菜单的模态循环也能驱动 ready。原生 wait 回调只投递消息；一个 ready 待消费期间停止重复通知，owner 消费后再按有效状态恢复监听。remove 注销等待并退休排队消息、活动回调后才报告完成，通知 HANDLE 仍由 loop 关闭。

确实会进入原生模态循环的 beginMove／beginResize／showWindowMenu 等操作先保留当前有效输入上下文，排入 native 命令队列，等当前 handler 和 session pump 返回后执行。不能在活动 pump 栈内阻塞，再依赖被 LoopDriver 禁止的递归 pump 推进 I/O。普通用户事件仍不递归派发；所用 API 本身立即返回时不人为增加模态循环。Windows [WM_ENTERSIZEMOVE](https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-entersizemove) 的循环持续至 DefWindowProc 返回，需要真实拖动中的 timer／网络／取消验收。

watch/remove 的所有权、非内联注册、串行通知、回调退出后注销规则保持。Darwin GUI 适配在实际服务的 CFRunLoop modes 上监听 pipe，Linux 监听 eventfd；两者都不读取 loop 的通知载荷。窗口消息唤醒和 I/O 完成分别存储真实队列，可以合并通知，不能依赖“一次唤醒等于一个事件”。

第三方挂起点仍可能从其他线程恢复。需要修改窗口时回到 ApplicationProxy.post；需要继续有父任务的挂起工作时用 session／集成 scope 的入口。本设计不修改协程恢复语义或增加通用 dispatcher。

## 5. 窗口属性、DPI 与平台差异

WindowAttributes 使用普通属性／值类型，包含标题、内容尺寸约束、可见性、装饰、可调整大小、初始位置／全屏、透明、主题、窗口级别、按钮集合、图标、初始光标及激活请求。创建时取得独立快照，之后修改原属性对象或图像数组不改变已创建窗口；ApplicationOptions 同样在构造时取快照。无平台支持的必需属性使创建失败；主题／聚焦等建议性属性只表示已请求，不能把请求值伪装成 OS 已接受的状态。各方法分组见映射表。

`RequestStatus` 为 Applied／Requested／Unsupported。Applied 只用于可立即确认的设置；Requested 由后续事件／快照确认；Unsupported 不产生副作用。非法参数用通常参数异常；平台失败用 WindowException，保留 operation、错误类别、backend 和可选原生 code。supports 是当前能力提示，不代替实际调用的错误处理；运行环境失联、权限或 compositor 状态仍可能变化。

公共类型表达实际后端的共同语义与必要差异。窗口级别用 Normal／AlwaysOnTop／AlwaysOnBottom；后续全屏采用 Windowed／Borderless(monitor?)／Exclusive(videoMode)。本阶段直接提供已实施的方法，不预留一组没有对应方法的 WindowFeature 枚举或空平台分支。Windows 接入时复用语义相同的类型；发现当前表示不合适时直接修订 API 与调用方，不为保持旧事件匹配增加平行模型。

窗口内部坐标以左上为原点，x 向右、y 向下。LogicalPosition／LogicalSize／LogicalRect 使用有限 Double；PhysicalSize 使用非负 Int，表示 drawable 像素尺寸；PhysicalPosition 保留小数，适合物理像素中的指针位置。尺寸非负、scaleFactor 为有限正值；render target 遇到零尺寸暂停取得／提交帧。

`WindowMetrics` 同时携带逻辑内容大小、实际 drawable 大小与有效 scaleFactor；一次 GeometryChanged 发布一个完整快照，避免上层读到新 DPI 配旧尺寸。原生配置未完成时 metrics 为 None；Wayland 首次创建及 unmap 后重新显示均需等待 configure，不虚构可绘制尺寸。请求尺寸与实际尺寸分开；Wayland configure 中的零宽／高是客户端自行选取该维度的提示，使用有效请求、上次实际尺寸或初始默认尺寸，不能直接发布为零 drawable。

全局位置单独用 `DesktopPosition`／`DesktopRect`，表示当前显示系统的桌面坐标；不能用某一个窗口的 scaleFactor 在混合 DPI 桌面上进行全局换算。桌面位置与尺寸查询允许 None，Wayland 不伪造全局窗口位置或 primary monitor。窗口内物理／逻辑换算只在同一 metrics 快照中进行，像素分配尺寸向上取整。

MonitorId、WindowId、DeviceId 为不同值类型，库内分配且在本次 Application 内不复用；不暴露 XID、指针或数组下标作为公共 id。Monitor 是可保存的自有快照，屏幕离开后不再出现在新枚举中，旧快照内容仍可读取；名称、位置、刷新率等未知时为 None，刷新率使用可选整数毫赫兹，不能假定 60 Hz。触摸身份和显示模式控制随对应后续能力加入。

## 6. 输入与 IME

键盘事件分别表达 PhysicalKey、LogicalKey、KeyLocation、Pressed／Released、repeat、modifiers 与 synthetic 标志。LogicalKey 区分 Named／Character(String)／Dead／Unidentified；物理键使用既有标准键位集合及 Unidentified(NativeKeyCode) 退路。NativeKeyCode 保留实际平台编码来源，Windows 后端加入时表达 Win32 编码，不能把平台 scan code 当 Unicode。modifier 包含 Shift／Control／Alt／Super、锁定状态和可取得的左右侧信息。[winit KeyEvent 参照](https://docs.rs/winit/0.30.13/winit/event/struct.KeyEvent.html)。

文字统一通过 `TextInputEvent.Edit(TextInputBatch)` 中的 Commit 操作插入：普通可打印键、dead-key compose 和 IME 都归一到该入口；LogicalKey 用于快捷键和键位语义，不再隐式插入第二份字符。TextInputEvent 只含 Enabled、Disabled、Edit，旧草案的独立 Preedit／Commit／DeleteSurrounding 事件直接替换，不提供兼容别名。失焦时撤销 active preedit、清理按下状态；需要合成 release 时标记 synthetic，不伪造真实输入时间。

本阶段输入法 API 为 `setImeAllowed` 和 `setImeCursorArea`，对应 winit 的 marked／preedit、commit 与候选窗位置。AppKit 只保存 native marked text，不维护完整编辑器文档镜像，也不提供重转换；purpose 和 surrounding 文档桥后续随上层编辑器需求增加。[winit 0.30.13 的 NSTextInputClient 实现](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/view.rs)同样不提供 attributedSubstring 文档查询。

`TextInputState` 保留为应用可选用的纯值编辑辅助，包含 surroundingText、带 anchor／cursor 的 selection、可选 marked range、caret rect 和 revision；所有范围以 UTF-8 字节为单位并落在字符边界，选择保留活动端方向。AppKit 转换实际 marked text 的 UTF-16 选区，产生不依赖文档 revision 的 CurrentSelection 编辑。只改变候选窗位置不推进文本 revision。

TextInputBatch 携带可选 baseRevision 和有序编辑操作，具体载荷见[窗口规范第 5 节](../specs/SCOOP-WINDOW-SPEC.md#5-输入与文本)。一批操作只调用一次 handler，在开始时检查基础快照，按批次内顺序解释后续位置，最后发布一次新快照。不能在删除 surrounding 后立即推进公开 revision，再把同批 commit／preedit 当成过期数据。范围引用失效或 native 数据非法时整批拒绝并重新同步，不部分应用；无需旧快照的普通 CurrentSelection 提交保持可用。

Wayland 每个 text-input-v3.done 形成一个批次，依协议顺序清除旧 preedit、删除 surrounding、提交文字和设置新 preedit。协议 done serial 与编辑器 revision 分别保存，前者不匹配不能直接作为丢弃输入的理由；平台按协议处理状态提交。AppKit 一次输入解释、未来 TSF 一次编辑会话同样保留原生更新边界。[text-input-v3 协议](https://github.com/wayland-mirror/wayland-protocols/blob/main/unstable/text-input/text-input-unstable-v3.xml)给出了顺序与 serial 规则。

平台使用当前 native marked text 和候选窗矩形回答同步询问，不在 NSTextInputClient／未来 TSF 回调中挂起。当前 AppKit 桥不发布应用文档正文。窗口层只转换文本更新和坐标，编辑器保留真实文档、布局与 grapheme 导航的责任；后续 purpose／surrounding 扩展也不得向平台发布密码正文。

鼠标区分按键、进入／离开、窗口内位置、相对设备位移。滚轮保留 LineDelta 与 PixelDelta，明确相位和惯性信息；不能猜测每行固定像素数。触摸携带独立 contact id、phase、位置和可选 pressure；原生 pinch／rotation／pan／double-tap／pressure 保留事件来源与单位。不支持的原生手势不靠周期采样伪造。

raw device 事件默认只在应用有焦点时请求，设备数据没有稳定身份时 DeviceId 为 None；不同平台不保证每个物理设备都可区分。光标 None／Confined／Locked 分开表达；Locked 不隐含可见性修改，鼠标 warp 不等于相对输入。Wayland 抓取、激活、移动／调整大小所需的原生 input serial 由后端保留；beginMove／beginResize 只在当前有效用户输入派发中接受，不能用任意历史点击模拟权限。

## 7. 重绘与渲染边界

requestRedraw 只置 dirty 并唤醒，重复请求可合并。RedrawRequested 在窗口可绘制时发出；绘制回调内再次请求的重绘留到下一轮，不能递归绘制。遮挡、隐藏、零尺寸期间保留 dirty，恢复可绘制后补通知，不能为不可见窗口持续忙等。

`prePresentNotify()` 告诉窗口后端即将提交帧，用于 Wayland frame callback 等节流；它不是实际 present，也不表示 GPU 完成。渲染层拥有 Metal／Vulkan／OpenGL 的 device、swapchain、surface、提交和 vsync。窗口层不把 io_uring timer 当显示时钟，不内置 Skia surface 或强制渲染线程。[winit 绘制边界](https://docs.rs/winit/0.30.13/winit/)。

`scoop.window.native` 提供 `@Unsafe Window.acquireNativeWindow(): NativeWindowLease`。lease 保活 Window 和 display connection；`handle` 为带类型的 NativeWindowHandle，首期随实际后端声明 AppKit(nsWindow, nsView)、Wayland(display, surface)、Xlib(display, windowId, visualId, screen)。Windows 目标声明 Win32(hwnd, hinstance)；当前目标只声明实际后端的 handle 变体。XID 使用整数，指针／HANDLE 使用 Ptr<Unit>；不以一个 Long 统一所有句柄，也不为未来 enum 匹配预留没有实现的分支。

lease 的 handle 查询在 owner 上执行；显式 close 可跨线程，原子封闭该借用后把实际释放投递到 owner 的内部收尾队列，不在调用线程执行 GUI API。该队列在停止业务 proxy 投递后仍然可用，避免后台 GPU 完成回调无法释放窗口；重复 close 为空操作，调用后不得继续使用 handle。不能从 GC release 启动这条异步路径。Skia／GPU 包装持有 lease，销毁依赖原生窗口的 surface 并结束有关 GPU／native 借用后再释放。窗口进入 Closing 后不能取得新 lease，已有 lease 阻止库主动销毁原生窗口；lease 不保证外部显示服务器永不失联，也不授权跨线程操作原生对象。渲染层需要后台线程时遵守对应图形 API 的线程规则。

平台只能查询自己的类型化 handle；native renderer 按实际平台消费载荷。新增后端可以调整该 enum 与相关 renderer，按普通依赖关系重新编译。lease 只保证对象寿命，不能据此在隐藏、待 configure 或 Closing 状态提交帧；后台 renderer 须与 owner 串行协调已有提交、unmap 和后续重新配置。

## 8. 生命周期与退出顺序

Window 状态为 PendingConfigure／Open／Closing／Destroyed；重新显示时可再次进入 PendingConfigure，隐藏／映射状态由后端单独记录。CloseRequested 表示用户意图，不自动销毁；应用可忽略、确认后 requestDestroy，或先保存数据。requestDestroy 幂等、非阻塞，进入 Closing 并排队 Closing 通知，停止新输入／重绘与句柄借用。等 Closing 已交付、现有事件派发栈与 native lease 均结束后销毁原生对象，再发一次 Destroyed；Destroyed 后只保留 id／终态快照，活动操作失败。

ApplicationOptions 的最后窗口策略为 KeepRunning 或 Exit，默认 KeepRunning，与 winit 的显式退出行为一致。macOS 关闭最后窗口不被硬编码成退出，Windows 也不硬编码成相反行为。系统退出请求携带原因，由相同应用事件表达；真正的强制终止／显示服务器消失可能不允许 veto 或完整 cleanup，不承诺从被 OS 强制杀死中恢复。

正常 requestExit 或未捕获 handler 异常按以下顺序收尾：

1. 停止接受业务 proxy 投递与创建窗口，将所有剩余窗口转入 Closing，等同分别调用 requestDestroy；当前用户派发返回后，在主线程交付一次 Exiting 和各窗口的 Closing 通知，让渲染包装开始释放。
2. 集成模式调用 EventLoopSession.requestClose，沿已有根取消等待所有子任务、借用和 I/O 清理；继续服务平台事件、watch、native cleanup 与 GPU 回调。纯窗口模式没有这一步。
3. native lease 归零且派发退出的窗口可销毁；逐一发出最终 Destroyed。退出期间这些最终通知直接在 owner 的收尾 Context 中执行，不通过已经关闭的 session mailbox，不提供活动 Job。
4. 等所有窗口销毁、集成 session 的 onClosed 调用 ApplicationHost.complete、全部 watch 移除、已接受控制工作完成以及 native callback lease 退休。没有窗口时也要等附着完成；一个条件先完成不替代其他条件。
5. 断开 display／停止 AppKit 主循环，释放应用回调，run 返回；首个业务失败与清理失败沿 ResourceUseException 组合后抛出。

步骤 2、3 可以交错推进，但实际退出必须同时满足所有条件。Closing／Exiting 之后的用户异常只记录失败并继续收尾，不重入相同通知；某个 cleanup 失败也不能跳过其他窗口。泄漏的 native lease 或不返回的外部工作可能使退出无法完成，不能假称关闭成功或由 GC 强拆仍使用的窗口。

native 事件只存稳定 native payload 与数值 id，进入 managed 前复制文本／路径／坐标。所有 user callback 通过已有 ForeignCallback，捕获异常并通知应用收尾，禁止异常跨 Objective-C／C／Win32 栈展开。main 返回前必须退出注册和活动 lease；AppKit 自身需要的 native 回调不调用 managed 时也按对应 C／ObjC 对象寿命释放。

## 9. macOS 后端

用 AppKit 的 NSApplication／NSWindow／NSView／delegate／NSTextInputClient 承载真实桌面行为，全部在主线程；每轮原生派发有 autorelease pool。run 通过 NSApplication 的正常循环服务事件，退出用可返回的 stop 与必要唤醒，不调用直接结束进程的 terminate 或永不返回的 dispatch_main。[Apple 文本输入协议](https://developer.apple.com/documentation/appkit/nstextinputclient)。

ApplicationHost 的 Darwin 监听使用 libdispatch READ source 观察 pipe，再 signal／wake 一个安装到 AppKit 所服务 modes 的 CFRunLoopSource，由该 source 在 owner 上调用 ready。等待 owner 消费期间暂停 READ source 的重复通知，处理后按有效状态恢复；取消时平衡暂停并等待 cancel handler，再在 owner 注销 CFRunLoopSource。集成 Cone 的 WindowLoopHost 只适配这组操作为 asyncio 的 watch/remove；MainDispatchHost 继续服务实际使用主 dispatch queue 的调用方。必须以真实拖动、菜单和模态验证 common／tracking／modal modes 的推进，不能只靠 mode 名称推断。

窗口位置／内容尺寸、backing scale、屏幕切换、全屏转换、occlusion、主题由原生通知生成。NSView 只提供可供 renderer 使用的宿主，不由窗口库拥有 Metal device 或 Skia；是否配置 CAMetalLayer 由渲染适配管理，并在窗口关闭前恢复／释放其持有状态。

需要正式支持 Darwin native `.m`：当前[语言规范 12.2.1](../specs/SCOOP-SPEC.md)只接受 C／C++ 后缀，不能用 c_flags 绕过输入语言检查。实施时先修订该节与[实现规范 2.7](../specs/SCOOP-IMPL-SPEC.md)，再让显式选择的 `.m` 交给所选 Apple Clang，以 ARC 编译，提供独立 objc_flags；`.c`、generated-C 和 runtime 保持原语言。条件筛选先行，非 Darwin 选中 `.m` 明确诊断；本期不扩展 `.mm` 或 Scoop 的 Objective-C 语法。

framework 链接已有合同可复用；仍需实际验证本仓库 Mach-O linker 对 Objective-C 元数据、类／selector 引用、ARC 外部符号和 AppKit framework 的正常链接，不能只增加后缀就宣称可用。使用 Objective-C 薄层比大量手工 objc_msgSend cast／动态注册类更适合维护；不增加 runtime 调度 ABI。

## 10. Linux 后端

### 10.1 选择与公共边界

Linux 自动模式优先连接有效的 Wayland 桌面，再在建立任何窗口前尝试 X11；可通过 `scoop.window.platform.linux` 显式选择 WaylandOnly／X11Only 及 display name。明确指定的后端失败不得静默切换；开始运行后失联也不能把既有窗口迁到另一显示服务器。该 Linux 专用选项不放进公共 ApplicationOptions。

窗口 native 后端把显示连接、app proxy 通知、ApplicationHost 注册的通知源、键重复／ControlFlow deadline 组合在一次原生等待中。显示连接只由窗口后端读取；集成 Cone 的 WindowLoopHost 注册 asyncio 的 eventfd 并接收 ready，asyncio 消费自己的通知。所有原生等待走 NativeSafe，空闲不轮询；已有批次未处理或即将关闭时不进入阻塞等待。

### 10.2 Wayland

使用 libwayland-client；协议优先 xdg-shell，按 compositor 宣告协商 text-input-v3、fractional-scale／viewporter、pointer-constraints／relative-pointer、pointer-gestures、xdg-activation、xdg-decoration 等。协议缺失通过能力和 Unsupported 表达，不承诺全局定位、强制聚焦、独占显示模式或任意鼠标 warp。

等待严格执行 dispatch_pending → prepare_read → flush → poll → read_events／cancel_read，再 dispatch；prepare 成功后每条退出路径必须 read 或 cancel。flush 返回 EAGAIN 时临时监听可写；其他 fd 唤醒也要取消已准备的读取。禁止直接用 io_uring recv 消费 wl_display fd，或让多个 reader 争用同一协议队列。[Wayland client 合同](https://wayland.freedesktop.org/docs/html/apb.html)。

创建 xdg role 后设置标题、app id、尺寸约束等，先提交无 buffer 的初始状态；收到 configure、确定实际尺寸并 ack 后才公布 metrics、提交绘制 buffer。configure 的零宽／高表示客户端选择该维度，处理规则见第 5 节。resize 配置、scale、surface role 和 frame callback 由后端管理，renderer 在已公布 metrics 下分配像素 buffer；fractional scaling 用协商后的 viewport／buffer-scale 规则，不把 scaleFactor 强制取整。renderer 可管理自己的事件队列，但须遵守 libwayland 的共享 display 读取协议，不能自行裸读 fd。

setVisible(false) 先与 renderer 串行停止新 buffer 提交，再以 null buffer commit unmap，清除可绘制配置并保留 dirty。unmap 后 toplevel 的相关状态失效；再次显示时重新提交属性及无 buffer 初始状态，进入 PendingConfigure、等待新的 configure／ack，然后发布完整 metrics 并补 redraw。WindowId 和仍有效的 lease 保持原对象身份，但旧 metrics 不授权提前提交新帧。[xdg-shell 协议](https://github.com/wayland-mirror/wayland-protocols/blob/main/stable/xdg-shell/xdg-shell.xml)的首次映射与重新映射规则都适用。

键位和 compose 用 libxkbcommon；IME 用 text-input-v3 的批次与 done 序列，版本／能力不足时保留普通键输入并明确 IME 不可用。客户端装饰使用 libdecor 及匹配的 C 装饰插件；若 compositor 不提供服务端装饰且客户端装饰依赖不可用，要求装饰的窗口创建失败，不无提示地变成无边框。协议和装饰依赖版本在实施开始时固定，并记录许可证。

### 10.3 X11

Xlib 持有唯一 Display 与事件读取；XKB／libxkbcommon 归一键位，XIM／XIC 和 UTF-8 lookup 负责输入法，XInput2 负责可用的 raw／touch，XRandR 负责显示器／模式／热插拔，Xcursor 负责光标。不能混用 Xlib 与 XCB 作为同一连接的独立事件消费者。[libxkbcommon 官方说明](https://xkbcommon.org/doc/current/)。

实现 WM_DELETE_WINDOW、EWMH 状态、configure、焦点与 XDND 文件拖放。尺寸、置顶、激活和全屏请求需等待 WM 事实确认；窗口管理器可以忽略建议。X11 error handler 的进程级属性必须由本库自有 Application 边界管理，捕获请求相关错误，不能把异步 X error 当一次 setter 已成功，也不能静默覆盖宿主的既有 Xlib 策略。

### 10.4 依赖与发行

Linux 首期按真实桌面依赖动态链接，GNU／musl 各自使用兼容 headers、库及 libdecor／XIM 模块；不能让 musl 程序加载 glibc 插件。完整静态桌面分发不是首期验收承诺；native 声明与显式链接模式冲突走现有链接诊断。桌面库不进入无窗口程序的依赖闭包。

生成的 Wayland 协议 C 源码在维护／发行准备阶段用固定 scanner 版本生成并作为普通输入携带；消费预编译 Cone 不临时联网、不自动运行依赖安装器。缺失库和 display 按普通构建／运行错误报告。是否允许无显示器环境启动由明确的测试宿主决定，不用一个假窗口后端冒充桌面实现。

## 11. Windows 后端

本阶段以 Windows 10 1703 起的 Per-Monitor V2 为窗口 API 基线，消费现有三个 Windows x86_64 target。消息窗口和业务 HWND 均由主线程拥有，原生 C 薄层通过已有 ForeignCallback 回到 Scoop。共享生命周期、业务／收尾队列和测试辅助不复制为第二套实现。平台文件由普通 Cone source 条件选择；native handle 增加 Win32(hwnd, hinstance)，未知键码分别保留 Windows scan code 和 virtual key 来源。

普通窗口使用 Unicode Win32 API。物理键来自扩展 scan code，逻辑键来自当前键盘布局；WM_CHAR 与 IMM32 的 result／composition 统一为自有 UTF-8 编辑批次。IMM32 只发布 marked text 和候选窗矩形，不发布应用文档，取消组合交付空 Preedit。TSF 文档存储、raw device、触摸、全屏和捕获仍属原设计的后续扩展，不增加空实现。

所有 HWND 创建前确认 owner 的 DPI awareness。WM_DPICHANGED 应用系统建议 rect 后发布实际 client pixel size 与 DPI 的完整快照。桌面位置采用物理虚拟桌面坐标；内容逻辑坐标按本窗口 DPI 转换，不用单个 scale 换算整个桌面。显示器枚举使用 EnumDisplayMonitors／GetMonitorInfo／当前显示设置，离线身份由共享层退休。

通知使用一次性 native threadpool wait；worker 只向 owner HWND 投递就绪消息，owner 消费后才重新 arm。remove 停止 wait、等待 native 回调退出，排队消息以 identity 查找仍存活的登记，完成通知只在活动 ready 返回后交付。借入的 manual-reset event 不由 host reset 或 close。真实系统 move／size、菜单和模态循环的 HWND 消息仍能驱动这条路径；会进入模态的测试命令必须在当前 Scoop handler 和 session pump 返回后执行。

renderer 示例使用独立普通 Cone 的 Win32／DXGI surface，持有窗口 lease，实际提交后先释放 GPU／swapchain 再结束 lease。普通行为使用 scoop.test，三个 Windows target 共用源码；桌面交互、混合 DPI 和输入法验证分别记录实际证据与环境限制。

Windows 平台移植的参考状态见[分支进度](https://github.com/windoze/scoop/blob/0fa053fd11c11e641e2f17212ce4948d5d52447a/docs/windows-targets/PROGRESS.md)：目标、runtime、platform、IOCP 及 executable subsystem／RC 已有实现，整体移植仍在推进。窗口任务消费这些平台能力，补 Win32 消息、窗口、输入和显示系统后端；不重复开发另一套 target 或 I/O backend。

| Windows 窗口工作 | 设计要求 |
| --- | --- |
| HWND 与消息队列，WM_CLOSE／WM_DESTROY | WindowId、CloseRequested、Closing、Destroyed |
| 消息泵、系统模态循环与 manual-reset event | EventLoopNotification.WindowsEvent、HWND 消息唤醒及同一 watch/remove 退休合同 |
| Per-Monitor V2、WM_DPICHANGED 与建议 rect | 创建窗口前确认 DPI 模式，完整 WindowMetrics、请求与实际值分离 |
| 虚拟键／scan code／WM_CHAR／IME 或 TSF | PhysicalKey、LogicalKey、UTF-8 TextInputBatch／TextInputState |
| WM_POINTER／Raw Input／cursor clipping | Pointer／Touch／DeviceEvent 与 Confined／Locked 能力 |
| Win32／DXGI rendering surface | 实施时增加 NativeWindowHandle.Win32，并遵守 lease 寿命 |
| 系统关机、激活限制、DWM 功能差异 | 退出原因、RequestStatus、WindowFeature／WindowException |

Windows 后端要求创建窗口的 owner 使用 Per-Monitor V2。应用优先通过已有 `[windows].rc` 嵌入 DPI manifest；没有声明时，在任何 HWND 或依赖 DPI 的 UI 初始化前设置并确认实际模式。已是所需模式时直接使用；已有不同模式或设置失败明确报告 WindowException，不能用被虚拟化的坐标冒充真实物理尺寸。`subsystem = "gui"` 只决定 executable subsystem，不隐含 DPI 设置，也不需要新增窗口专用的资源配置语言。参考[默认 DPI awareness](https://learn.microsoft.com/en-us/windows/win32/hidpi/setting-the-default-dpi-awareness-for-a-process)与 [WM_DPICHANGED](https://learn.microsoft.com/en-us/windows/win32/hidpi/wm-dpichanged)。

平台专有能力放 `platform.windows`；共同语义继续通过公共窗口 API 表达。真实 Windows 接入发现 API、事件、native 产物或构建合同需要变化时直接修订，并同步其他实际后端和调用方。首期不以“未来无需改 API”为验收承诺，也不创建 Windows 窗口空实现；后续验收必须包含真实模态循环、混合 DPI 和 renderer。

## 12. 实施顺序与验收

1. 按本草案定义最小端到端子集，先落实实际需要的 `.m` native 管线，并采用 Windows 分支的 EventLoopNotification／watch；同步实际宿主与调用方，不新增旧签名兼容层。
2. 用真实 window／window.asyncio 两个 Cone 完成公开宿主接入、窗口创建、输入、提交一帧及退出。持有 renderer lease 和活动 I/O 时请求退出，确认窗口、session、watch 与 lease 能依次或交错收尾；没有窗口时也能等待 session 完成。渲染使用小型实际 surface 适配，不建设完整 GUI 或通用渲染框架。
3. 后续 Linux 阶段完成 Wayland 的首次配置、零尺寸提示、隐藏／重显、DPI、IME 批次、装饰、frame、拖放及协议能力差异，并实现 X11 对应行为。GNU／musl 共用 Scoop 层，不复制测试实现。
4. 在已有基础光标、monitor、主题和窗口属性之上，按映射表分批补充 touch／gesture／raw device、全屏控制及高级可选属性；每项“平台不支持”均有真实 capability 依据和明确结果。
5. 每个阶段以配套示例、真实目标测试和明确限制独立交付。当前基础架构／macOS 骨架阶段与完整能力地图分别记录；Linux 窗口及平台专测另行安排，平台基础分支通过不等于窗口可用。

纯值／归一规则、状态、错误与普通 API 行为用所属 Cone 的 scoop.test；真实窗口与 callback 测试使用主线程测试程序，必要的 OS 进程控制／ABI／native 编译能力才用少量 fixture。当前目标为真实 macOS 与 Windows 桌面；后续 Linux 验收覆盖 Wayland compositor、X11 WM 及 GNU／musl 库。Xvfb 或 headless compositor 可覆盖自动化行为，但不能替代实体显示器、输入法候选交互与真实用户拖动的手工验证。

重点组合：多个窗口与任务并存；关闭／外线程投递竞争；移动 GC 下事件文本和 ForeignCallback；关闭中 surface lease 未释放；用户 handler／清理抛错；菜单、拖动、模态期间 timer／网络／取消推进；混合 DPI 跨屏与热插拔；零 drawable 与 Wayland 零尺寸提示的区别；隐藏／重显后真实 buffer 提交；中文／日文组合、emoji／UTF-16 range、dead keys、重复输入与失焦；同批 delete／commit／preedit、选择方向、协议 serial 与文本 revision 的区别；Wayland flush EAGAIN／prepared-read 取消；X11 WM 忽略请求；display 断开；退出时 I/O、window、watch 各自先完成及没有窗口的顺序。

Windows 的首期工作是审查公共语义与已实现平台基础的接缝：普通窗口参数不携带 fd／HWND，错误和 native 句柄保留实际来源与宽度；通知复用 EventLoopNotification，Windows 实施满足模态循环和 DPI 初始化合同。无需预留所有未来 enum 分支，也不把静态表示审查或空 stub 当成 Windows 窗口验收。

性能只记录空闲 CPU／唤醒、输入到 handler、redraw 合并、I/O 与 UI 同时繁忙时的延迟及关闭耗时；主循环不引入通用预算或计费。渲染性能与完整 GUI 基准留到实际 renderer 接入。当前代码与验收结果见实施记录。
