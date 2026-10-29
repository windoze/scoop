# Scoop 桌面窗口库规范

状态：实施中。日期：2026-10-30。本文件描述普通 Cone `scoop.window` 与 `scoop.window.asyncio` 的当前目标合同；实际交付与验证范围见[实施记录](../windowing/PROGRESS.md)。依据 [设计](../windowing/DESIGN.md)与 [winit 能力映射](../windowing/WINIT-MAPPING.md)。

## 1. 分层与目标

window 依赖 core／std／platform；window.asyncio 另依赖 asyncio。本阶段实施基础架构与 Darwin ARM64／AppKit。Linux Wayland／X11、Windows 及其他后端留待后续；不添加没有后端的 native handle 变体或假窗口实现。两个库及其原生薄层、测试、示例均以普通 Cone 组织，窗口逻辑不进入编译器或 runtime。

本轮范围按 winit 定位为 GUI 骨架：应用／窗口与基础属性、基础输入／IME、DPI／显示器快照、渲染句柄和 I/O 集成。下文的完整窗口层模型同时保留后续扩展方向；独占全屏／视频模式切换、全局 raw device、触摸／高级手势、光标捕获及复杂平台窗口交互不作为当前骨架验收项。未实施接口随实际需求再增加，不为占据 API 名称加入空实现。

公共应用对象不暴露 fd、HWND、XID、dispatch queue 或 ring。现有 Job／CoroutineScope、Context、ForeignCallback、时间、路径与异常规则继续适用；窗口库不承担布局、排版、绘制设备、I/O 调度或另一套任务树。

## 2. 应用与线程

Application 构造接收 ApplicationOptions 并取得独立快照，run 接收 ApplicationHandler。handler 只有 `onEvent(application: ActiveApplication, event: ApplicationEvent)` 一个分发入口。每进程一个 Application、一次 run；入口要求进程主线程和当前 Job 外。构造只建立 managed 状态，接受 run 后才建立原生应用；启动失败消费已取得资源且不能重用该 Application。基础库不认识 I/O Context，集成入口另由 PlatformEventLoop.attach 拒绝活动 I/O EventLoop binding。ActiveApplication 负责 createWindow、monitors、primaryMonitor、setControlFlow、requestExit。

ApplicationProxy 提供跨线程 post 与 requestExit；post 为 `(ActiveApplication) -> Unit`，返回 Boolean，异步排队。false 表示拒绝且不保留闭包；true 不保证之后关闭时仍执行。关闭停止业务投递，清理通知继续处理。所有原生窗口操作只在 owner 线程；WindowId／DeviceId／MonitorId 和不可变快照可跨线程保存。

ControlFlow 为 Wait／Poll／WaitUntil(TimeMark)，默认 Wait；TimeMark 必须来自 MonotonicTimeSource。每轮消费有限快照，新增事件进入后续轮；锁内不执行用户代码，不递归调用 handler。OS 必须立即得到的回答使用已发布 native 状态，不等待可挂起的用户处理。

run Context 在业务回调中恢复，外线程 post 不替换该 Context。用户异常终止业务派发并开始有序收尾；native bridge 捕获异常，不跨 C／ObjC 栈展开。最终清理通知继续执行，异常记录后继续关闭其他资源。

## 3. 事件模型

ApplicationEvent 的完整首期类别为：NewEvents(StartCause)、Resumed、Suspended、MemoryWarning、Window(WindowId, WindowEvent)、Device(DeviceId?, DeviceEvent)、MonitorsChanged、AboutToWait、ExitRequested(ExitReason)、Exiting。启动在创建业务窗口之前发送一次 Resumed；失焦不等于 Suspended。proxy 使用类型化闭包，不另设无类型 user-event payload。

WindowEvent 类别为：CloseRequested、Closing、Destroyed、GeometryChanged(WindowMetrics)、Moved(DesktopPosition)、Focused(Boolean)、Occluded(Boolean)、ThemeChanged(Theme?)、Keyboard(KeyEvent)、ModifiersChanged(Modifiers)、Pointer(PointerEvent)、Touch(TouchEvent)、Gesture(NativeGestureEvent)、TextInput(TextInputEvent)、FileDrop(FileDropEvent)、ActivationChanged(ActivationResult)。RedrawRequested 为独立窗口事件。payload 为自有值，不携带 OS 回调栈的借出指针。

NewEvents 原因区分 Init、Poll、Wake 与 Deadline；平台没有信息时不伪造输入硬件时间。允许合并 redraw、连续 geometry 和 pointer move，但不得跨过按键／按钮、IME 编辑、焦点、close 等顺序边界；Commit、drop、button、touch 的开始／结束及 lifecycle 不得丢弃。

## 4. 窗口与显示器

WindowAttributes、Window 的目标能力与方法分组以映射表为完整首期清单。请求返回 Applied／Requested／Unsupported；Requested 不保证系统采纳，实际状态由通知／快照决定。查询无法知道时返回 None；不把 desired state 写成 observed state。创建所需属性不支持时明确失败，不悄悄改变含义。

WindowException 保留 WindowErrorKind、operation、backend 和可选 native code；类别包含 Unsupported、Unavailable、PermissionDenied、InvalidState、ResourceBusy、Disconnected、NativeFailure。参数错误使用现有参数异常；backend 随实际后端提供，本阶段为 AppKit，不假定原生 code 都是 errno。能力提示不替代实际操作错误。

WindowId、DeviceId、MonitorId、TouchId 为不同值类型，只在一次 Application 生命周期内比较，不暴露原生身份，不复用活过的编号。本阶段 Monitor 为不可变快照，保存 id、可选 name／desktopBounds／refreshRateMilliHertz、physicalSize 与 scaleFactor；不持有原生指针。ActiveApplication.monitors／primaryMonitor 与 Window.currentMonitor 在 owner 上获取快照，后者及 primaryMonitor 可为 None。AppKit 用 NSScreen 的桌面矩形和 backing 转换，CoreGraphics 未给出正刷新率时返回 None，不把最大可用刷新率当作实际刷新率。显示器变更通知刷新当前身份集合，已观察到离开的身份退休；再次出现分配新编号。后续加入视频模式及控制请求时再定义对应活动句柄的失效行为。

LogicalPosition／LogicalSize／LogicalRect 采用有限 Double，内容坐标左上原点、向右／向下递增。PhysicalSize 为非负 Int 像素尺寸；PhysicalPosition 允许小数。DesktopPosition／DesktopRect 仅用于同一显示系统的全局桌面坐标，不允许通过单个窗口 DPI 在混合 DPI 桌面上换算。

WindowMetrics 包含 logicalSize、physicalSize、scaleFactor，scaleFactor 必须有限且为正；首个原生配置前 metrics 为 None，GeometryChanged 原子发布完整快照。请求与实际配置分离；渲染所需像素向上取整，零 drawable 大小不得申请或提交帧。

全屏统一为 Windowed／Borderless(Monitor?)／Exclusive(VideoMode)，级别统一为 Normal／AlwaysOnTop／AlwaysOnBottom。不支持的操作明确返回，Windows 实现不能要求新增一个表达同一语义的变体。平台专有选项放在 platform 子包。

尺寸约束分别保存可选 min／max，非负且 min 不大于 max；创建尺寸必须满足约束。运行中收紧约束时请求调整当前内容大小，实际结果由 GeometryChanged 确认。resize increments 为可选的有限正尺寸，None 恢复平台默认。属性在创建返回前复制并应用，再按 visible／activation 请求显示窗口；不会先显示一个忽略所需属性的窗口。

AppKit 使用原生内容尺寸／桌面坐标、titlebar 按钮、window level、miniaturize／zoom、appearance 与 NSVisualEffectView。模糊背景放在 renderer 宿主 view 后面，关闭模糊时恢复独立的 transparent 设置。content protection 使用系统分享提示，返回 Requested，不保证屏幕捕获系统遵守该提示。macOS 的 activation policy 和 titlebar／full-size-content 选项位于 platform.macos；应用标识只查询实际 main bundle，命令行进程没有 bundle identifier 时返回 None。

## 5. 输入与文本

KeyEvent 分离 PhysicalKey、LogicalKey、KeyLocation、state、repeat、modifiers 和 synthetic；LogicalKey 支持 Named／Character(String)／Dead／Unidentified，未知 native 键码保留实际编码来源，本阶段为 Darwin。修饰键含 Shift、Control、Alt、Super、锁定状态和可取得的左右侧信息。

文字统一由 `TextInputEvent.Edit(TextInputBatch)` 中的 Commit 操作交付；普通键、compose、IME 不得重复插入。TextInputEvent 只有 Enabled／Disabled／Edit。一次 AppKit 输入解释形成一个编辑批次，以一个事件调用 handler。范围为 UTF-8 字节区间且位于字符边界，平台 UTF-16 索引在 native 边界转换；非法 native 范围触发状态重同步，不越界访问。

本阶段提供 owner 线程上的 setImeAllowed 与 setImeCursorArea，采用 winit 0.30.13 的 AppKit 文本输入边界：NSTextInputClient 只保存当前 marked text 和候选窗矩形，不向系统发布完整编辑器文档，attributedSubstring 查询返回 nil。原生询问直接从这些 native 值回答，不进入 managed handler 或挂起。完整 surrounding 文档桥接、重转换与 purpose 扩展后续按实际编辑器需求增加。

TextInputState 是应用可选用的纯值编辑辅助，包含 surroundingText、selection、markedRange?、caretRect、revision；它不是本阶段自动发布给 OS 的文档镜像。selection 保存 anchor／cursor，不能排序后丢失选择活动端；markedRange 为 Utf8Range。只修改候选窗位置不推进应用文本 revision。组合与真实编辑状态由上层编辑器持有，窗口库不负责排版或 grapheme 导航。

TextInputBatch 保存可选 baseRevision 与有序的 TextInputOperation 数组。Utf8Range 是非负、半开的 Long 字节区间，start 不大于 end；Utf8Selection 的两端均非负。操作如下：

| 变体 | 载荷与语义 |
| --- | --- |
| ClearPreedit | 清除当前组合状态 |
| Preedit | text、可选 selection（相对 preedit，保留活动端）、replacement；空 text 清除组合 |
| Commit | text、replacement；replacement 为 CurrentSelection 或 Range(Utf8Range)，范围引用该批前序操作产生的临时文本 |
| DeleteSurrounding | beforeBytes、afterBytes；相对当前 selection 两侧，须校验长度与字符边界 |

纯值批次开始时核对 baseRevision，按顺序在临时文本上解释范围，整批通过后发布一次新快照。任何范围或 revision 失效均拒绝整批，不能部分应用。AppKit 基础桥产生 baseRevision=None、CurrentSelection 的 Preedit／Commit；普通文本不依赖 surrounding 快照。取消组合产生空 Preedit，删除应用正在显示的组合内容；不能只用 ClearPreedit 清除标记后再重复插入 Commit。原生 UTF-16 选区必须合法且不切断 surrogate pair，非法批次拒绝并撤销组合。未来需要 surrounding 的协议再接入文档快照与协议 serial，不为当前桥引入独立同步状态机。

`TextInputState.apply(batch)` 提供上述字节编辑规则的纯值实现，供输入归一和应用同步快照使用，不承担文档布局。CurrentSelection 在存在 marked range 时替换组合文本，否则替换 selection；ClearPreedit 只清除 marked 标记。Preedit 插入后设置 marked range，并把相对 selection 平移到新文本；Commit 清除组合，光标落在插入末尾。DeleteSurrounding 删除 selection 之前／之后指定字节，保留选区正文与活动端方向，清除组合标记。成功批次统一使 revision 加一，空批次保持原值；过期 revision 报 IllegalStateException，非法范围、尺寸或 revision 溢出报 IllegalArgumentException，原快照保持不变。

Pointer 区分位置、按钮、进入／离开、滚轮；滚轮区分行／像素并保留相位。raw motion 是设备单位；Touch 含 contact id、phase 和可选压力；Gesture 只报告原生手势。DeviceId 不可得时为 None。失焦清理组合与按下状态，必要的补发 release 标记 synthetic。

光标模式为 None／Confined／Locked，warp 与相对输入分离；custom cursor 的 RGBA 与 hotspot 在创建时复制。移动／resize／系统窗口菜单请求需要有效输入上下文的平台，由后端保留原生 serial 并核对，不能靠定时回调伪造用户操作。

## 6. 绘制与 native 句柄

requestRedraw 合并 dirty，RedrawRequested 不递归触发；隐藏、遮挡或零尺寸时保留 pending redraw，恢复后再处理。prePresentNotify 是提交前通知，不能等价为 present、vsync 或 GPU 完成。图形设备与渲染管线由上层 renderer 管理。

native 子包提供 Unsafe 的 acquireNativeWindow，返回 NativeWindowLease；本阶段 handle 为 `NativeWindowHandle.AppKit(nsWindow: Ptr<Unit>, nsView: Ptr<Unit>)`。其他变体随实际后端增加。取得 lease 和查询 handle 只在 owner 上进行，lease 保活窗口和 display connection；窗口 Closing 后不能取得新 lease。

lease.close 可跨线程且幂等，原子结束借用并把实际释放排入 owner 内部收尾队列，不直接在调用线程操作 GUI，不依赖已经停止接受业务的 ApplicationProxy。调用后不能再使用 handle；队列实际消费前继续保活原生对象。GC release 不得代替这个显式操作。

已有 lease 阻止库主动销毁原生窗口，但不保证显示服务器永远可用，也不扩展平台图形对象的线程权限。renderer 先结束 surface 及有关原生使用，再释放 lease；GC release 不调用窗口 API。lease 不授权在隐藏、零尺寸、待配置或 Closing 时提交帧。

## 7. 窗口及应用关闭

状态为 PendingConfigure／Open／Closing／Destroyed。CloseRequested 只表示意图；requestDestroy 幂等、非阻塞，先进入 Closing／通知并拒绝新业务，等 Closing 已交付、callback 和 lease 均退休后实际销毁并发一次 Destroyed。Closing 后不再交付业务输入／重绘；迟到原生事件按 id 和 phase 丢弃，Destroyed 为该窗口的最后事件。

ApplicationOptions.lastWindowPolicy 为 KeepRunning／Exit，默认 KeepRunning。退出先停止业务投递，发送 Exiting／Closing；随后继续服务 native cleanup、窗口销毁和可选 I/O 根收尾。所有窗口、watch、native callback、集成 session 均完成后，才停止主循环并返回／抛错。清理错误不阻止其他资源关闭，按 ResourceUseException 保留正文与清理失败。

在用户显式退出、handler 失败、安装失败和显示连接失联中都必须消费已经取得的资源。OS 强制终止和无法返回的 native 工作不承诺完整或有界关闭；不能通过 GC 或提前关闭句柄伪装成功。

## 8. asyncio 集成

Application.runWithAsyncio(handler) 是独立 Cone 的扩展，复用 PlatformEventLoop.attach 与 EventLoopSession。业务 handler 及 proxy 消息在 session 根 Context 下运行，currentEventLoop 与 currentApplicationScope 可用。根取消及时触发窗口收尾控制，不等 onClosed 才开始释放窗口；退出时 requestClose，继续原生循环至 onClosed。Exiting／Closing／Destroyed 在 owner 收尾 Context 执行，没有活动业务 scope。用于派发通知的内部唤醒不得循环生成 NewEvents／AboutToWait。

原始 Application.run 不初始化 io_uring；集成入口继承 asyncio 的平台要求。第三方挂起函数不自动获得 UI 线程亲和性；跨线程窗口操作通过 proxy 返回 owner。

`scoop.window.native` 为当前集成 Cone 提供以下公开低层接口，不暴露 internal owner：

- `@Unsafe Application.runAttached(handler, attach: (ApplicationHost) -> ApplicationAttachment)` 与 run 共用单次入口；原生宿主可服务后、首个业务事件前调用 attach。安装失败回滚所有原生资源。
- ApplicationAttachment 是普通接口，`post(block: () -> Unit): Boolean` 投递业务闭包，`requestClose()` 请求现有 session 收尾；基础库不依赖 CoroutineScope／EventLoop 类型。
- ApplicationHost 提供 `isCurrentThread()`、Darwin 的 `@Unsafe watchReadable(descriptor: Int, ready: () -> Unit): ApplicationWatch`；监听不读取或关闭借入 fd、不内联 ready。ApplicationWatch.remove 在 owner 上注销，当前 ready 退出、排队通知退休后回调一次。
- `postControl(block: () -> Unit): Boolean` 在关闭业务队列后继续接受收尾工作；控制闭包用 `requestExit()` 在 owner 开始关闭。`complete(failure: Throwable?)` 由 session.onClosed 在 owner 调用一次。无窗口时仍等待附着完成、watch 注销和已接受控制工作执行完。

WindowLoopHost 将这些实际操作适配为[asyncio 规范 11.3](SCOOP-ASYNCIO-SPEC.md#113-宿主接入合同)的 EventLoopNotification／watch。根取消订阅只排入控制队列，不跨线程直接操作窗口；集成 scope 仅在业务回调期间绑定。通知派发与纯 I/O／watch 维护唤醒不自行产生新一轮 NewEvents／AboutToWait，避免空闲自唤醒。

## 9. 构建与验收边界

Darwin 使用语言规范 12.2.1、实现规范 2.7 的普通 native Objective-C `.m`／ARC 与 framework 声明。AppKit backend 由 NSApplication 正常循环驱动，以 stop 和唤醒返回；不调用 terminate。通知监听以 dispatch READ source 观察 fd，通过 common／tracking／modal modes 的 CFRunLoopSource 在 owner 派发；等待消费时暂停重复通知，注销先平衡暂停并等待 cancel handler，再退休 owner source。渲染层自行管理实际 layer／surface。

本阶段以所属 Cone 的 scoop.test 验证值、输入归一、API、真实 AppKit、native callback 与 asyncio 收尾；`.m` 编译／链接／诊断使用必要的 compiler fixture。真实 surface 示例用单独 renderer Cone 消费 native lease。显示器热插拔、混合 DPI、输入法候选交互与真实用户拖动等依赖实际硬件／桌面输入的验收单独记录，不能用合成事件冒充。Linux／其他平台验收留待后续。
