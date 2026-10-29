# Scoop 桌面窗口库规范草案

状态：设计，待实施。日期：2026-10-27。本文件描述计划中的普通 Cone `scoop.window` 与 `scoop.window.asyncio`，不代表已发布 API。依据 [设计](../windowing/DESIGN.md)与 [winit 能力映射](../windowing/WINIT-MAPPING.md)；不修改现有语言或 runtime 的生效合同。

## 1. 分层与目标

window 依赖 core／std／platform；window.asyncio 另依赖 asyncio。首期目标为 Darwin ARM64、Linux x86-64 GNU／musl，Linux 同时设计 Wayland 与 X11。Windows 仅保留统一 API、事件语义及 native／wake source 表示，不增加 target 或实现。

公共应用对象不暴露 fd、HWND、XID、dispatch queue 或 ring。现有 Job／CoroutineScope、Context、ForeignCallback、时间、路径与异常规则继续适用；窗口库不承担布局、排版、绘制设备、I/O 调度或另一套任务树。

## 2. 应用与线程

Application 构造接收 ApplicationOptions，run 接收 ApplicationHandler。handler 只有 `onEvent(application: ActiveApplication, event: ApplicationEvent)` 一个分发入口。每进程一个 Application、一次 run；入口要求进程主线程、当前 Job 和 I/O EventLoop binding 外。ActiveApplication 负责 createWindow、monitors、primaryMonitor、setControlFlow、requestExit。

ApplicationProxy 提供跨线程 post 与 requestExit；post 为 `(ActiveApplication) -> Unit`，返回 Boolean，异步排队。false 表示拒绝且不保留闭包；true 不保证之后关闭时仍执行。关闭停止业务投递，清理通知继续处理。所有原生窗口操作只在 owner 线程；WindowId／DeviceId／MonitorId 和不可变快照可跨线程保存。

ControlFlow 为 Wait／Poll／WaitUntil(TimeMark)，默认 Wait；TimeMark 必须来自 MonotonicTimeSource。每轮消费有限快照，新增事件进入后续轮；锁内不执行用户代码，不递归调用 handler。OS 必须立即得到的回答使用已发布 native 状态，不等待可挂起的用户处理。

run Context 在业务回调中恢复，外线程 post 不替换该 Context。用户异常终止业务派发并开始有序收尾；native bridge 捕获异常，不跨 C／ObjC 栈展开。最终清理通知继续执行，异常记录后继续关闭其他资源。

## 3. 事件模型

ApplicationEvent 的完整首期类别为：NewEvents(StartCause)、Resumed、Suspended、MemoryWarning、Window(WindowId, WindowEvent)、Device(DeviceId?, DeviceEvent)、MonitorsChanged、AboutToWait、ExitRequested(ExitReason)、Exiting。启动在创建业务窗口之前发送一次 Resumed；失焦不等于 Suspended。proxy 使用类型化闭包，不另设无类型 user-event payload。

WindowEvent 类别为：CloseRequested、Closing、Destroyed、GeometryChanged(WindowMetrics)、Moved(DesktopPosition)、Focused(Boolean)、Occluded(Boolean)、ThemeChanged(Theme?)、Keyboard(KeyEvent)、ModifiersChanged(Modifiers)、Pointer(PointerEvent)、Touch(TouchEvent)、Gesture(NativeGestureEvent)、TextInput(TextInputEvent)、FileDrop(FileDropEvent)、ActivationChanged(ActivationResult)。RedrawRequested 为独立窗口事件。payload 为自有值，不携带 OS 回调栈的借出指针。

NewEvents 原因区分 Init、Poll、Wake 与 Deadline；平台没有信息时不伪造输入硬件时间。允许合并 redraw、连续 geometry 和 pointer move，但不得跨过按键／按钮、IME 编辑、焦点、close 等顺序边界；Commit、drop、button、touch 的开始／结束及 lifecycle 不得丢弃。

## 4. 窗口与显示器

WindowAttributes、Window 的目标能力与方法分组以映射表为完整首期清单。请求返回 Applied／Requested／Unsupported；Requested 不保证系统采纳，实际状态由通知／快照决定。查询无法知道时返回 None；不把 desired state 写成 observed state。创建所需属性不支持时明确失败，不悄悄改变含义。

WindowException 保留 WindowErrorKind、operation、backend 和可选 native code；类别包含 Unsupported、Unavailable、PermissionDenied、InvalidState、ResourceBusy、Disconnected、NativeFailure。参数错误使用现有参数异常；backend 表示 AppKit／Wayland／X11／Win32，不假定原生 code 都是 errno。能力提示不替代实际操作错误。

WindowId、DeviceId、MonitorId、TouchId 为不同值类型，只在一次 Application 生命周期内比较，不暴露原生身份，不复用活过的编号。Monitor 保存可选 name、desktopBounds、refreshRateMilliHertz 与视频模式；失效后的控制请求报告 unavailable。primaryMonitor 可为 None。

LogicalPosition／LogicalSize／LogicalRect 采用有限 Double，内容坐标左上原点、向右／向下递增。PhysicalSize 为非负 Int 像素尺寸；PhysicalPosition 允许小数。DesktopPosition／DesktopRect 仅用于同一显示系统的全局桌面坐标，不允许通过单个窗口 DPI 在混合 DPI 桌面上换算。

WindowMetrics 包含 logicalSize、physicalSize、scaleFactor，scaleFactor 必须有限且为正；首个原生配置前 metrics 为 None，GeometryChanged 原子发布完整快照。请求与实际配置分离；渲染所需像素向上取整，零 drawable 大小不得申请或提交帧。

全屏统一为 Windowed／Borderless(Monitor?)／Exclusive(VideoMode)，级别统一为 Normal／AlwaysOnTop／AlwaysOnBottom。不支持的操作明确返回，Windows 实现不能要求新增一个表达同一语义的变体。平台专有选项放在 platform 子包。

## 5. 输入与文本

KeyEvent 分离 PhysicalKey、LogicalKey、KeyLocation、state、repeat、modifiers 和 synthetic；LogicalKey 支持 Named／Character(String)／Dead／Unidentified，未知 native 键码带 Darwin／XKB／Win32 来源。修饰键含 Shift、Control、Alt、Super、锁定状态和可取得的左右侧信息。

文字统一由 TextInputEvent.Commit 交付；普通键、compose、IME 不得重复插入。IME 提供 Enabled／Preedit／Commit／Disabled，以及基于 TextInputState 快照的替换／surrounding 删除编辑。范围为 UTF-8 字节区间且位于字符边界，平台 UTF-16 索引在 native 边界转换；陈旧 revision 的编辑不得应用于新快照。未知范围用 None，非法 native 范围触发状态重同步，不越界访问。

TextInputState 包含 surroundingText、selection、markedRange?、caretRect、revision；setImeAllowed、setImePurpose、setImeCursorArea 与 updateTextInput 只在 owner 上更新。系统同步询问从 native 副本回答；password 模式不发布密码正文。组合与真实编辑状态由上层编辑器持有，窗口库不负责排版或 grapheme 导航。

TextInputEvent 的数据合同如下；Utf8Range 是非负、半开的 Long 字节区间，start 不大于 end：

| 变体 | 载荷与语义 |
| --- | --- |
| Enabled／Disabled | 开始／停止接受该窗口的原生组合输入 |
| Preedit | text、可选 selection（相对 preedit）、可选 replacement（带已发布快照 revision 和范围）；空 text 清除组合 |
| Commit | text、replacement；replacement 为 CurrentSelection 或 Range(revision, Utf8Range)，前者用于无需引用旧快照的普通输入 |
| DeleteSurrounding | revision、beforeBytes、afterBytes；相对该快照 selection 的两侧，须校验长度与字符边界 |

普通 Commit 不因没有 surrounding 快照而丢失；只有明确引用旧 revision 的范围编辑才受快照一致性约束。IME 目的统一 Normal／Password／Terminal，平台不能完全满足某种 purpose 时仅作为建议，不改变实际文本的编码。

Pointer 区分位置、按钮、进入／离开、滚轮；滚轮区分行／像素并保留相位。raw motion 是设备单位；Touch 含 contact id、phase 和可选压力；Gesture 只报告原生手势。DeviceId 不可得时为 None。失焦清理组合与按下状态，必要的补发 release 标记 synthetic。

光标模式为 None／Confined／Locked，warp 与相对输入分离；custom cursor 的 RGBA 与 hotspot 在创建时复制。移动／resize／系统窗口菜单请求需要有效输入上下文的平台，由后端保留原生 serial 并核对，不能靠定时回调伪造用户操作。

## 6. 绘制与 native 句柄

requestRedraw 合并 dirty，RedrawRequested 不递归触发；隐藏、遮挡或零尺寸时保留 pending redraw，恢复后再处理。prePresentNotify 是提交前通知，不能等价为 present、vsync 或 GPU 完成。图形设备与渲染管线由上层 renderer 管理。

native 子包提供 Unsafe 的 acquireNativeWindow，返回 NativeWindowLease；handle 具有 AppKit、Wayland、Xlib、Win32 四种明确载荷，分别保存真正需要的指针／句柄／整数 XID。取得 lease 和查询 handle 只在 owner 上进行，lease 保活窗口和 display connection；窗口 Closing 后不能取得新 lease。

lease.close 可跨线程且幂等，原子结束借用并把实际释放排入 owner 内部收尾队列，不直接在调用线程操作 GUI，不依赖已经停止接受业务的 ApplicationProxy。调用后不能再使用 handle；队列实际消费前继续保活原生对象。GC release 不得代替这个显式操作。

已有 lease 阻止库主动销毁原生窗口，但不保证显示服务器永远可用，也不扩展平台图形对象的线程权限。renderer 先结束 surface 及有关原生使用，再释放 lease；GC release 不调用窗口 API。Windows handle variant 只有表示合同，本期没有可产生它的后端。

## 7. 窗口及应用关闭

状态为 PendingConfigure／Open／Closing／Destroyed。CloseRequested 只表示意图；requestDestroy 幂等、非阻塞，先进入 Closing／通知并拒绝新业务，等 Closing 已交付、callback 和 lease 均退休后实际销毁并发一次 Destroyed。Closing 后不再交付业务输入／重绘；迟到原生事件按 id 和 phase 丢弃，Destroyed 为该窗口的最后事件。

ApplicationOptions.lastWindowPolicy 为 KeepRunning／Exit，默认 KeepRunning。退出先停止业务投递，发送 Exiting／Closing；随后继续服务 native cleanup、窗口销毁和可选 I/O 根收尾。所有窗口、watch、native callback、集成 session 均完成后，才停止主循环并返回／抛错。清理错误不阻止其他资源关闭，按 ResourceUseException 保留正文与清理失败。

在用户显式退出、handler 失败、安装失败和显示连接失联中都必须消费已经取得的资源。OS 强制终止和无法返回的 native 工作不承诺完整或有界关闭；不能通过 GC 或提前关闭句柄伪装成功。

## 8. asyncio 集成

Application.runWithAsyncio(handler) 是独立 Cone 的扩展，复用 PlatformEventLoop.attach 与 EventLoopSession。业务 handler 及 proxy 消息在 session 根 Context 下运行，currentEventLoop 与 currentApplicationScope 可用。根取消及时触发窗口收尾控制，不等 onClosed 才开始释放窗口；退出时 requestClose，继续原生循环至 onClosed。Exiting／Closing／Destroyed 在 owner 收尾 Context 执行，没有活动业务 scope。用于派发通知的内部唤醒不得循环生成 NewEvents／AboutToWait。

原始 Application.run 不初始化 io_uring；集成入口继承 asyncio 的平台要求。第三方挂起函数不自动获得 UI 线程亲和性；跨线程窗口操作通过 proxy 返回 owner。

EventLoopHost 的 planned wake-source 兼容扩展见[asyncio 规范第 12 节](SCOOP-ASYNCIO-SPEC.md)。窗口公共对象不继承该低层接口，Windows 使用 manual-reset event 的表示已经确定；加入后端无需改窗口应用入口或事件签名。

## 9. 构建与验收边界

Darwin 需要先按设计补齐普通 native Objective-C `.m` 编译与 Mach-O 链接验证；Linux 使用目标兼容的 Wayland／X11／输入／装饰库。具体编译与发行规则在实施前同步语言规范 12.2.1、实现规范 2.7；本草案不宣称当前编译器已支持 `.m`。

验收覆盖真实 AppKit、Wayland 与 X11，GNU／musl 分别验证；状态／值行为以 Cone 测试为主，必要 native ABI／外部进程控制使用少量 fixture。Windows 此阶段只有合同审查，没有平台验收；完整范围与验收组合见设计第 12 节。
