# Scoop 桌面窗口库规范

状态：基础架构与 macOS GUI 骨架已实现，Linux 阶段实施中。日期：2026-10-30。本文件描述普通 Cone `scoop.window` 与 `scoop.window.asyncio` 的合同；实际交付与验证范围见 [macOS 实施记录](../windowing/PROGRESS.md)与 [Linux 实施记录](../windowing/LINUX-PROGRESS.md)。依据 [设计](../windowing/DESIGN.md)与 [winit 能力映射](../windowing/WINIT-MAPPING.md)。

## 1. 分层与目标

window 依赖 core／std／platform；window.asyncio 另依赖 asyncio。已有 Darwin ARM64／AppKit，本阶段加入 Linux amd64 GNU／musl 的 Wayland／X11；Windows 及其他后端留待后续。两个库及其原生薄层、测试、示例均以普通 Cone 组织，窗口逻辑不进入编译器或 runtime。

本轮范围按 winit 定位为 GUI 骨架：应用／窗口与基础属性、基础输入／IME、DPI／显示器快照、渲染句柄和 I/O 集成；Linux 阶段另包含第 10 节的文件拖放。下文的完整窗口层模型同时保留后续扩展方向；全屏控制／视频模式切换、全局 raw device、触摸／高级手势、光标捕获及复杂平台窗口交互不作为当前骨架验收项。未实施接口随实际需求再增加，不为占据 API 名称加入空实现。

公共应用对象不暴露 fd、HWND、XID、dispatch queue 或 ring。现有 Job／CoroutineScope、Context、ForeignCallback、时间、路径与异常规则继续适用；窗口库不承担布局、排版、绘制设备、I/O 调度或另一套任务树。

## 2. 应用与线程

Application 构造接收 ApplicationOptions 并取得独立快照，run 接收 ApplicationHandler。handler 只有 `onEvent(application: ActiveApplication, event: ApplicationEvent)` 一个分发入口。每进程一个 Application、一次 run；入口要求进程主线程和当前 Job 外。构造只建立 managed 状态，接受 run 后才建立原生应用；启动失败消费已取得资源且不能重用该 Application。基础库不认识 I/O Context，集成入口另由 PlatformEventLoop.attach 拒绝活动 I/O EventLoop binding。ActiveApplication 负责 createWindow、monitors、primaryMonitor、setControlFlow、requestExit。

ApplicationProxy 提供跨线程 post 与 requestExit；post 为 `(ActiveApplication) -> Unit`，返回 Boolean，异步排队。false 表示拒绝且不保留闭包；true 不保证之后关闭时仍执行。关闭停止业务投递，清理通知继续处理。所有原生窗口操作只在 owner 线程；WindowId／DeviceId／MonitorId 和不可变快照可跨线程保存。

ControlFlow 为 Wait／Poll／WaitUntil(TimeMark)，默认 Wait；TimeMark 必须来自 MonotonicTimeSource。每轮消费有限快照，新增事件进入后续轮；锁内不执行用户代码，不递归调用 handler。OS 必须立即得到的回答使用已发布 native 状态，不等待可挂起的用户处理。

run Context 在业务回调中恢复，外线程 post 不替换该 Context。用户异常终止业务派发并开始有序收尾；native bridge 捕获异常，不跨 C／ObjC 栈展开。最终清理通知继续执行，异常记录后继续关闭其他资源。

## 3. 事件模型

ApplicationEvent 类别为：NewEvents(StartCause)、Resumed、Suspended、MemoryWarning、Window(WindowId, WindowEvent)、MonitorsChanged、AboutToWait、ExitRequested(ExitReason)、Exiting。启动在创建业务窗口之前发送一次 Resumed；失焦不等于 Suspended，AppKit 不捏造没有收到的 suspended／memory warning。proxy 使用类型化闭包，不另设无类型 user-event payload。全局 Device 事件属于后续设备输入扩展。

WindowEvent 类别为：CloseRequested、Closing、Destroyed、GeometryChanged(WindowMetrics)、Moved(DesktopPosition)、Focused(Boolean)、Occluded(Boolean)、ThemeChanged(Theme?)、Keyboard(KeyEvent)、ModifiersChanged(Modifiers)、Pointer(PointerEvent)、TextInput(TextInputEvent)、FileDrop(FileDropEvent)。RedrawRequested 为独立窗口事件。payload 为自有值，不携带 OS 回调栈的借出指针。文件拖放当前由 Linux 后端交付；触摸、手势和平台专有激活结果随对应能力后续增加。

NewEvents 原因区分 Init、Poll、Wake 与 Deadline；平台没有信息时不伪造输入硬件时间。允许合并 redraw、连续 geometry 和 pointer move，但不得跨过按键／按钮、IME 编辑、焦点、close 等顺序边界；Commit、drop、button、touch 的开始／结束及 lifecycle 不得丢弃。

## 4. 窗口与显示器

WindowAttributes、Window 的能力与方法分组见映射表，当前阶段只提供其中标为本期的已实施接口。请求返回 Applied／Requested／Unsupported；Requested 不保证系统采纳，实际状态由通知／快照决定。查询无法知道时返回 None；不把 desired state 写成 observed state。创建所需属性不支持时明确失败，不悄悄改变含义。

WindowException 保留 WindowErrorKind、operation、backend 和可选 native code；类别包含 Unsupported、Unavailable、PermissionDenied、InvalidState、ResourceBusy、Disconnected、NativeFailure。参数错误使用现有参数异常；backend 为可选的 AppKit／Wayland／X11，不假定原生 code 都是 errno。能力提示不替代实际操作错误。标题与 Linux display name、app id、instance 不能含 NUL，违反时报告 IllegalArgumentException。

WindowId、DeviceId、MonitorId 为不同值类型，只在一次 Application 生命周期内比较，不暴露原生身份，不复用活过的编号。本阶段 Monitor 为不可变快照，保存 id、可选 name／desktopBounds／refreshRateMilliHertz、physicalSize 与 scaleFactor；不持有原生指针。ActiveApplication.monitors／primaryMonitor 与 Window.currentMonitor 在 owner 上获取快照，后者及 primaryMonitor 可为 None。AppKit 用 NSScreen 的桌面矩形和 backing 转换，CoreGraphics 未给出正刷新率时返回 None，不把最大可用刷新率当作实际刷新率。显示器变更通知刷新当前身份集合，已观察到离开的身份退休；再次出现分配新编号。后续加入视频模式及控制请求时再定义对应活动句柄的失效行为。

LogicalPosition／LogicalSize／LogicalRect 采用有限 Double，内容坐标左上原点、向右／向下递增。PhysicalSize 为非负 Int 像素尺寸；PhysicalPosition 允许小数。DesktopPosition／DesktopRect 仅用于同一显示系统的全局桌面坐标，不允许通过单个窗口 DPI 在混合 DPI 桌面上换算。

WindowMetrics 包含 logicalSize、physicalSize、scaleFactor，scaleFactor 必须有限且为正；首个原生配置前 metrics 为 None，GeometryChanged 原子发布完整快照。请求与实际配置分离；渲染所需像素向上取整，零 drawable 大小不得申请或提交帧。

窗口级别为 Normal／AlwaysOnTop／AlwaysOnBottom。后续全屏控制采用 Windowed／Borderless(Monitor?)／Exclusive(VideoMode)，并与显示模式切换和恢复一起实施；相同平台语义复用现有表示。平台专有选项放在 platform 子包。

尺寸约束分别保存可选 min／max，非负且 min 不大于 max；创建尺寸必须满足约束。运行中收紧约束时请求调整当前内容大小，实际结果由 GeometryChanged 确认。resize increments 为可选的有限正尺寸，None 恢复平台默认。属性在创建返回前复制并应用，再按 visible／activation 请求显示窗口；不会先显示一个忽略所需属性的窗口。

AppKit 使用原生内容尺寸／桌面坐标、titlebar 按钮、window level、miniaturize／zoom、appearance 与 NSVisualEffectView。改变装饰、resizable 或 full-size-content 的 style mask 后恢复输入 view 为 first responder，保证键盘路由连续。模糊背景放在 renderer 宿主 view 后面，关闭模糊时恢复独立的 transparent 设置。content protection 使用系统分享提示，返回 Requested，不保证屏幕捕获系统遵守该提示。macOS 的 activation policy 和 titlebar／full-size-content 选项位于 platform.macos；应用标识只查询实际 main bundle，命令行进程没有 bundle identifier 时返回 None。

## 5. 输入与文本

KeyEvent 分离 PhysicalKey、LogicalKey、KeyLocation、state、repeat、modifiers 和 synthetic；LogicalKey 支持 Named／Character(String)／Dead／Unidentified，未知 native 键码保留 Darwin／Xkb 编码来源。修饰键含 Shift、Control、Alt、Super、锁定状态和可取得的左右侧信息。

AppKit 使用实际 NSEvent keyCode 和当前布局产生的字符；Command／Control 快捷键不产生普通文本提交。无装饰窗口仍可成为 key／main window，接收键盘不依赖原生标题栏。已交付的按下状态在失焦时补发 synthetic release，清空按住的修饰键并保留已知锁定状态；没有硬件来源的设备 id 为 None。CapsLock 通知表达锁定状态，不伪造物理按键释放。指针同时携带同次 native 事件的逻辑／物理位置与 scale，两种位置均以内容区左上为原点，物理位置由该次窗口 backing scale 换算；不能直接混入 backing surface 的底部原点。精确滚轮的 AppKit points 转换为物理 PixelDelta，普通滚轮保持 LineDelta，普通 phase 和 momentumPhase 分别保留。光标基础图标使用 NSCursor，可见性只在本窗口内容区域生效，退出该区域、失焦或销毁时平衡系统 hide／unhide。

文字统一由 `TextInputEvent.Edit(TextInputBatch)` 中的 Commit 操作交付；普通键、compose、IME 不得重复插入。TextInputEvent 只有 Enabled／Disabled／Edit。一次 AppKit 输入解释形成一个编辑批次，以一个事件调用 handler。范围为 UTF-8 字节区间且位于字符边界，平台 UTF-16 索引在 native 边界转换；非法 native 范围触发状态重同步，不越界访问。

提供 owner 线程上的 setImeAllowed 与 setImeCursorArea，均返回 RequestStatus。AppKit 采用 winit 0.30.13 的文本输入边界：NSTextInputClient 只保存当前 marked text 和候选窗矩形，不向系统发布完整编辑器文档，attributedSubstring 查询返回 nil。原生询问直接从这些 native 值回答，不进入 managed handler 或挂起。Linux 的 surrounding 发布见第 10 节；AppKit 文档桥接、重转换与 purpose 扩展后续按实际编辑器需求增加。

IME 接入默认关闭；setImeAllowed 成功改变开关时产生 Enabled／Disabled，重复设置相同值不重复通知，Enabled 先于该次开启产生的编辑。关闭接入先撤销组合；失焦也撤销组合，但保留接入开关。setImeCursorArea 接受有限、非负尺寸的内容区逻辑矩形，包含矩形端点溢出检查。候选窗使用当前 NSView／NSWindow 转换后的屏幕坐标；没有文档位置可报告时，characterIndex 和 actualRange 返回 NSNotFound。

接入开启后，每次 keyDown 先调用 AppKit 的 interpretKeyEvents，再决定是否交付 Keyboard。组合处理及确认消耗的键不再交给应用执行普通编辑；doCommandBySelector 可要求交付应用命令，但同次解释已经提交组合时不再次交付确认键。Command／Control 快捷键正常交付 Keyboard，不能产生第二次文本插入；keyup 只释放此前已交付的按下状态。

TextInputState 是应用可选用的纯值编辑辅助，包含 surroundingText、selection、markedRange?、caretRect、revision；它不是本阶段自动发布给 OS 的文档镜像。selection 保存 anchor／cursor，不能排序后丢失选择活动端；markedRange 为 Utf8Range。只修改候选窗位置不推进应用文本 revision。组合与真实编辑状态由上层编辑器持有，窗口库不负责排版或 grapheme 导航。

TextInputBatch 保存可选 baseRevision 与有序的 TextInputOperation 数组。Utf8Range 是非负、半开的 Long 字节区间，start 不大于 end；Utf8Selection 的两端均非负。操作如下：

| 变体 | 载荷与语义 |
| --- | --- |
| ClearPreedit | 清除当前组合状态 |
| Preedit | text、可选 selection（相对 preedit，保留活动端）、replacement；空 text 清除组合 |
| Commit | text、replacement；replacement 为 CurrentSelection 或 Range(Utf8Range)，范围引用该批前序操作产生的临时文本 |
| DeleteSurrounding | beforeBytes、afterBytes；相对当前 selection 两侧，须校验长度与字符边界 |

纯值批次开始时核对 baseRevision，按顺序在临时文本上解释范围，整批通过后发布一次新快照。任何范围或 revision 失效均拒绝整批，不能部分应用。AppKit 基础桥产生 baseRevision=None、CurrentSelection 的 Preedit／Commit；普通文本不依赖 surrounding 快照。取消组合产生空 Preedit，删除应用正在显示的组合内容；不能只用 ClearPreedit 清除标记后再重复插入 Commit。原生 UTF-16 选区必须合法且不切断 surrogate pair，非法批次拒绝并撤销组合。Linux surrounding 文档快照与协议 serial 见第 10 节，AppKit 当前桥不引入对应状态。

AppKit 的 NSNotFound replacement 表示替换当前 marked segment 或应用选区；显式 replacement 只能引用已知 native marked segment，归一为整段 CurrentSelection 替换，选区先平移再转 UTF-8。不能把未知文档位置伪装成可用范围。无效 native 批次丢弃全部待交付操作；仅在批次开始前应用已有 preedit 时产生一次空 Preedit 撤销它，不能因拒绝一个新组合而误删应用原选区。

`TextInputState.apply(batch)` 提供上述字节编辑规则的纯值实现，供输入归一和应用同步快照使用，不承担文档布局。CurrentSelection 在存在 marked range 时替换组合文本，否则替换 selection；ClearPreedit 只清除 marked 标记。Preedit 插入后设置 marked range，并把相对 selection 平移到新文本；Commit 清除组合，光标落在插入末尾。DeleteSurrounding 删除 selection 之前／之后指定字节，保留选区正文与活动端方向，清除组合标记。成功批次统一使 revision 加一，空批次保持原值；过期 revision 报 IllegalStateException，非法范围、尺寸或 revision 溢出报 IllegalArgumentException，原快照保持不变。

Pointer 区分位置、按钮、进入／离开、滚轮；滚轮区分行／像素并保留相位。DeviceId 不可得时为 None。失焦清理组合与按下状态，必要的补发 release 标记 synthetic。后续 raw motion 使用设备单位，Touch 保留 contact id、phase 和可选压力，Gesture 只报告原生手势；这些扩展不是当前交付接口。

后续光标捕获使用 None／Confined／Locked，warp 与相对输入分离；custom cursor 的 RGBA 与 hotspot 在创建时复制。移动／resize／系统窗口菜单请求需要有效输入上下文的平台，由后端保留原生 serial 并核对，不能靠定时回调伪造用户操作。

## 6. 绘制与 native 句柄

requestRedraw 合并 dirty，RedrawRequested 不递归触发；隐藏、遮挡或零尺寸时保留 pending redraw，恢复后再处理。prePresentNotify 是提交前通知，不能等价为 present、vsync 或 GPU 完成。图形设备与渲染管线由上层 renderer 管理。

native 子包提供 Unsafe 的 acquireNativeWindow，返回 NativeWindowLease；AppKit handle 为 `NativeWindowHandle.AppKit(nsWindow: Ptr<Unit>, nsView: Ptr<Unit>)`。取得 lease 和查询 handle 只在 owner 上进行，lease 保活窗口和 display connection；窗口 Closing 后不能取得新 lease。

Linux 实施加入 `Wayland(display: Ptr<Unit>, surface: Ptr<Unit>)` 与 `Xlib(display: Ptr<Unit>, windowId: ULong, visualId: ULong, screen: Int)`。整数 XID 与指针分别表达，renderer 仅消费实际后端的载荷。

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
- ApplicationHost 提供 `isCurrentThread()`、Darwin／Linux 的 `@Unsafe watchReadable(descriptor: Int, ready: () -> Unit): ApplicationWatch`；监听不读取或关闭借入 fd、不内联 ready。ApplicationWatch.remove 在 owner 上注销，当前 ready 退出、排队通知退休后回调一次。
- `postControl(block: () -> Unit): Boolean` 在关闭业务队列后继续接受收尾工作；控制闭包用 `requestExit()` 在 owner 开始关闭。`complete(failure: Throwable?)` 由 session.onClosed 在 owner 调用一次。无窗口时仍等待附着完成、watch 注销和已接受控制工作执行完。

WindowLoopHost 将这些实际操作适配为[asyncio 规范 11.3](SCOOP-ASYNCIO-SPEC.md#113-宿主接入合同)的 EventLoopNotification／watch。根取消订阅只排入控制队列，不跨线程直接操作窗口；集成 scope 仅在业务回调期间绑定。通知派发与纯 I/O／watch 维护唤醒不自行产生新一轮 NewEvents／AboutToWait，避免空闲自唤醒。

## 9. 构建与验收边界

Darwin 使用语言规范 12.2.1、实现规范 2.7 的普通 native Objective-C `.m`／ARC 与 framework 声明。AppKit backend 由 NSApplication 正常循环驱动，以 stop 和唤醒返回；不调用 terminate。通知监听以 dispatch READ source 观察 fd，通过 common／tracking／modal modes 的 CFRunLoopSource 在 owner 派发；等待消费时暂停重复通知，注销先平衡暂停并等待 cancel handler，再退休 owner source。渲染层自行管理实际 layer／surface。

所属 Cone 的 scoop.test 验证值、输入归一、API、真实窗口、native callback 与 asyncio 收尾；native 编译／链接／诊断使用必要的 compiler fixture。真实 surface 示例用单独 renderer Cone 消费 native lease。显示器热插拔、混合 DPI、输入法候选交互与真实用户拖动等依赖实际硬件／桌面输入的验收单独记录，不能用合成事件冒充。

## 10. Linux

Linux 扩展包 `scoop.window.platform.linux` 的 LinuxBackend 为 Automatic／WaylandOnly／X11Only，`ApplicationOptions.setLinuxBackend(backend, displayName?)` 在 Application 构造前选定并复制设置；Automatic 不接收显式 display name，依次尝试环境指定的 Wayland 和 X11。显式后端失败不回退，已经建立的连接失联后不迁移窗口。ActiveApplication.backend 报告实际后端；原生错误保留后端及错误码，未进入特定后端的普通状态错误可没有 backend。

Linux 原生等待同时服务显示连接、proxy eventfd、宿主借入通知、键重复和 ControlFlow deadline，使用普通 NativeSafe FFI。窗口库是显示连接的唯一事件消费者；asyncio eventfd 由其自己的 ready 回调消费。Wait 没有固定轮询周期，watch 注册不内联执行，remove 在回调退出与排队通知退休后完成。

Wayland 必需 wl_compositor v4 或更新版本与 xdg-shell；可选协议按 registry 版本协商。等待遵守 dispatch_pending、prepare_read、flush、poll、read_events／cancel_read 顺序，flush 的 EAGAIN 临时监听 POLLOUT；其他唤醒或错误出口必须取消已准备读取。首次无 buffer commit 后等待 configure／ack，宽高为零的 configure 维度采用有效尺寸请求、上次实际值或创建默认值。未配置时状态为 PendingConfigure、metrics=None，属性更新与销毁仍可用。隐藏以 null buffer commit unmap 并使配置失效；重新显示重新发送初始属性和无 buffer commit，等待新 configure 后再交付 GeometryChanged 与保留的 redraw。旧配置事件不能在重新映射后恢复过期 metrics。

Wayland 有 fractional-scale 与 viewporter 时按协议比例及向上取整后的像素尺寸配置 viewport；否则采用进入输出的整数 scale。native snapshot 分别报告配置有效与可重绘，frame callback 只节流下一次重绘；prePresentNotify 在 renderer 的实际 commit 前请求 callback，不替代提交。全局窗口位置、primary monitor 和 compositor 未提供的最小化／主题事实返回 None。要求装饰时使用 xdg-decoration 的服务端装饰或 libdecor 的 C 装饰插件（发行基线使用 cairo）；缺少两者不能创建一个伪装成功的有装饰窗口。

Wayland 显示器的 physicalSize 按 wl_output 当前 mode 与 transform 归一到桌面上的方向，90°／270° 及相应翻转变换交换像素宽高；desktopBounds 只采用 xdg-output 提供的逻辑坐标与尺寸。wl_output 移除后从当前快照及窗口关联中退休，重新公布的输出取得新 MonitorId，已交付的快照保持不变。

Wayland 的标题与 app id 必须能放入协议单条消息的 4096 字节上限；扣除消息头、字符串长度、结尾 NUL 与对齐后，UTF-8 正文最多 4083 字节。超限在创建或设置边界报告 Unsupported，不发送非法消息，不改变旧标题，也不截断应用文本。surrounding 使用下文定义的独立 4000 字节窗口。

X11 由 Xlib 读取事件，使用 XKB／libxkbcommon-x11、XIM／XIC、XRandR、Xcursor 和实际可用的 XI2 指针及平滑滚轮。Xlib-xcb 仅供 xkbcommon-x11 请求服务器 keymap，不接管事件读取。WM_DELETE_WINDOW 只产生 CloseRequested；尺寸、焦点、最小化／最大化和 EWMH 层级请求返回 Requested，实际状态从事件和查询取得。WM 不支持的协议返回 Unsupported，不写入伪造的 observed state。Xlib 进程级 error handler 在自有 Application 生命周期内安装和恢复，不接管其他 Display 的错误；异步请求错误及显示断开进入应用错误／关闭路径。

X11 主循环在每次阻塞前检查 Xlib 已读取的事件队列；交付 Scoop 回调不能消费掉该队列仍有工作的事实。队列非空时仍以非阻塞 poll 服务其他通知源，再继续分发 X 事件，不能只等待显示 socket 再次可读。

X11 原生调用期间局部处理连接写入产生的 SIGPIPE，使关闭竞争进入 Disconnected 收尾而非终止进程。调用退出或 I/O error 跳转后恢复调用线程原有信号 mask；不安装进程级 SIGPIPE handler，不消费调用方原先阻塞或已待处理的信号。

IME 使用 text-input-v3 或 XIM，普通输入使用 xkbcommon compose；只通过 TextInputBatch 提交文字。Wayland 每个 done 形成一个批次，依次撤销旧 preedit、删除 surrounding、commit、发布新 preedit，保留选择活动端。协议 serial 与应用文档 revision 独立；启用后不同时通过普通键再提交同一文字。缺少协议／输入上下文时 setImeAllowed 返回 Unsupported，普通键与 compose 仍可用。失焦、关闭输入法和销毁清理组合与按下状态。

Linux 扩展 `Window.setLinuxTextInputState(state)` 显式复制应用的 TextInputState 到 Wayland 输入上下文，返回 RequestStatus；XIM 没有对应接口时返回 Unsupported。快照须通过普通范围验证，发布内容不含 NUL。发布时移除 marked range，并保留选择活动端；按字符边界截取包含完整 selection 的最多 4000 字节上下文，这是 text-input-v3 的消息限制。selection 本身超过限制时返回 Unsupported。候选矩形改变不推进应用 revision。每次协议 commit 记录当时的应用 revision；done 的 serial 只用于关联该快照和协议状态，不直接充当 revision，不因 serial 与最新提交不同而丢弃输入。surrounding 删除必须对应已发布的快照并满足其边界；无效原生批次整体拒绝，撤销既有 preedit 并重新同步输入上下文。应用仍应通过 TextInputState.apply 核对 baseRevision，在实际编辑后重新发布快照。

Linux 文件拖放由 WindowEvent.FileDrop 的 Enter／Drop／Leave 表达；Enter／Drop 的载荷为自有 Path 数组，Leave 不带路径。只接受 text/uri-list 中的本地 file URI，按 URI 转义解码保留原始路径字节，不把远程 URL 当成本地文件。Wayland data offer 与 XDND selection 使用异步接收，保持事件顺序和退出时的 fd／offer 释放；不在用户 handler 中阻塞读取传输。

xdg-toplevel 提供 wm_capabilities 时，后续最大化／最小化请求遵守其实际集合；未收到首个 configure 的最大化快照为 None。尚未收到能力通知时可以排队请求，首次通知明确拒绝创建所需状态时报告 Unsupported。configure_bounds 是建议，不覆盖应用已提供的有效尺寸。surrounding 与候选矩形的一次发布使用同一次协议 commit，避免中间状态作为独立文档快照交给输入法。

GNU／musl 使用同一 Scoop 实现及测试源码，动态链接各自兼容的桌面库和装饰／输入法模块。固定 scanner／protocol 输入和生成结果由维护阶段准备，普通 Cone 构建及预编译消费者不执行扫描器、下载或依赖安装。Linux 验收包含真实 Wayland compositor、X11 WM、buffer 提交、输入／批次和 asyncio／lease 关闭；headless 自动化与人工桌面验证分别记录。
