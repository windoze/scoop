# Scoop 桌面窗口库规范

状态：基础架构与 macOS GUI 骨架已实现。日期：2026-10-30。本文件描述普通 Cone `scoop.window` 与 `scoop.window.asyncio` 的当前合同及后续平台设计方向；实际交付与验证范围见[实施记录](../windowing/PROGRESS.md)。依据 [设计](../windowing/DESIGN.md)与 [winit 能力映射](../windowing/WINIT-MAPPING.md)。

## 1. 分层与目标

window 依赖 core／std／platform；window.asyncio 另依赖 asyncio。Darwin ARM64／AppKit 已实现，本阶段增加三个 Windows x86_64 target 的 Win32 后端，窗口 API 要求 Windows 10 1703 起的 Per-Monitor V2。Linux Wayland／X11 及其他后端留待后续；不添加没有后端的 native handle 变体或假窗口实现。两个库及其原生薄层、测试、示例均以普通 Cone 组织，窗口逻辑不进入编译器或 runtime。

本轮范围按 winit 定位为 GUI 骨架：应用／窗口与基础属性、基础输入／IME、DPI／显示器快照、渲染句柄和 I/O 集成。下文的完整窗口层模型同时保留后续扩展方向；全屏控制／视频模式切换、全局 raw device、触摸／高级手势、光标捕获、文件拖放及复杂平台窗口交互不作为当前骨架验收项。未实施接口随实际需求再增加，不为占据 API 名称加入空实现。

公共应用对象不暴露 fd、HWND、XID、dispatch queue 或 ring。现有 Job／CoroutineScope、Context、ForeignCallback、时间、路径与异常规则继续适用；窗口库不承担布局、排版、绘制设备、I/O 调度或另一套任务树。

## 2. 应用与线程

Application 构造接收 ApplicationOptions 并取得独立快照，run 接收 ApplicationHandler。handler 只有 `onEvent(application: ActiveApplication, event: ApplicationEvent)` 一个分发入口。每进程一个 Application、一次 run；入口要求进程主线程和当前 Job 外。构造只建立 managed 状态，接受 run 后才建立原生应用；启动失败消费已取得资源且不能重用该 Application。基础库不认识 I/O Context，集成入口另由 PlatformEventLoop.attach 拒绝活动 I/O EventLoop binding。ActiveApplication 负责 createWindow、monitors、primaryMonitor、setControlFlow、requestExit。

ApplicationProxy 提供跨线程 post 与 requestExit；post 为 `(ActiveApplication) -> Unit`，返回 Boolean，异步排队。false 表示拒绝且不保留闭包；true 不保证之后关闭时仍执行。关闭停止业务投递，清理通知继续处理。所有原生窗口操作只在 owner 线程；WindowId／DeviceId／MonitorId 和不可变快照可跨线程保存。

ControlFlow 为 Wait／Poll／WaitUntil(TimeMark)，默认 Wait；TimeMark 必须来自 MonotonicTimeSource。每轮消费有限快照，新增事件进入后续轮；锁内不执行用户代码，不递归调用 handler。OS 必须立即得到的回答使用已发布 native 状态，不等待可挂起的用户处理。

run Context 在业务回调中恢复，外线程 post 不替换该 Context。用户异常终止业务派发并开始有序收尾；native bridge 捕获异常，不跨 C／ObjC 栈展开。最终清理通知继续执行，异常记录后继续关闭其他资源。

## 3. 事件模型

ApplicationEvent 类别为：NewEvents(StartCause)、Resumed、Suspended、MemoryWarning、Window(WindowId, WindowEvent)、MonitorsChanged、AboutToWait、ExitRequested(ExitReason)、Exiting。启动在创建业务窗口之前发送一次 Resumed；失焦不等于 Suspended，AppKit 不捏造没有收到的 suspended／memory warning。proxy 使用类型化闭包，不另设无类型 user-event payload。全局 Device 事件属于后续设备输入扩展。

WindowEvent 类别为：CloseRequested、Closing、Destroyed、GeometryChanged(WindowMetrics)、Moved(DesktopPosition)、Focused(Boolean)、Occluded(Boolean)、ThemeChanged(Theme?)、Keyboard(KeyEvent)、ModifiersChanged(Modifiers)、Pointer(PointerEvent)、TextInput(TextInputEvent)。RedrawRequested 为独立窗口事件。payload 为自有值，不携带 OS 回调栈的借出指针。触摸、手势、文件拖放和平台专有激活结果随对应能力后续增加。

NewEvents 原因区分 Init、Poll、Wake 与 Deadline；平台没有信息时不伪造输入硬件时间。允许合并 redraw、连续 geometry 和 pointer move，但不得跨过按键／按钮、IME 编辑、焦点、close 等顺序边界；Commit、drop、button、touch 的开始／结束及 lifecycle 不得丢弃。

## 4. 窗口与显示器

WindowAttributes、Window 的能力与方法分组见映射表，当前阶段只提供其中标为本期的已实施接口。请求返回 Applied／Requested／Unsupported；Requested 不保证系统采纳，实际状态由通知／快照决定。查询无法知道时返回 None；不把 desired state 写成 observed state。创建所需属性不支持时明确失败，不悄悄改变含义。

WindowException 保留 WindowErrorKind、operation、backend 和可选 native code；类别包含 Unsupported、Unavailable、PermissionDenied、InvalidState、ResourceBusy、Disconnected、NativeFailure。参数错误使用现有参数异常；backend 随实际后端提供，按目标为 AppKit／Win32，不假定原生 code 都是 errno；Win32 保留 DWORD／HRESULT 的 32-bit 数值。能力提示不替代实际操作错误。

WindowId、DeviceId、MonitorId 为不同值类型，只在一次 Application 生命周期内比较，不暴露原生身份，不复用活过的编号。本阶段 Monitor 为不可变快照，保存 id、可选 name／desktopBounds／refreshRateMilliHertz、physicalSize 与 scaleFactor；不持有原生指针。ActiveApplication.monitors／primaryMonitor 与 Window.currentMonitor 在 owner 上获取快照，后者及 primaryMonitor 可为 None。AppKit 用 NSScreen 的桌面矩形和 backing 转换，CoreGraphics 未给出正刷新率时返回 None，不把最大可用刷新率当作实际刷新率。显示器变更通知刷新当前身份集合，已观察到离开的身份退休；再次出现分配新编号。后续加入视频模式及控制请求时再定义对应活动句柄的失效行为。

LogicalPosition／LogicalSize／LogicalRect 采用有限 Double，内容坐标左上原点、向右／向下递增。PhysicalSize 为非负 Int 像素尺寸；PhysicalPosition 允许小数。DesktopPosition／DesktopRect 仅用于同一显示系统的全局桌面坐标，不允许通过单个窗口 DPI 在混合 DPI 桌面上换算。

WindowMetrics 包含 logicalSize、physicalSize、scaleFactor，scaleFactor 必须有限且为正；首个原生配置前 metrics 为 None，GeometryChanged 原子发布完整快照。请求与实际配置分离；渲染所需像素向上取整，零 drawable 大小不得申请或提交帧。

窗口级别为 Normal／AlwaysOnTop／AlwaysOnBottom。后续全屏控制采用 Windowed／Borderless(Monitor?)／Exclusive(VideoMode)，并与显示模式切换和恢复一起实施；相同平台语义复用现有表示。平台专有选项放在 platform 子包。

尺寸约束分别保存可选 min／max，非负且 min 不大于 max；创建尺寸必须满足约束。运行中收紧约束时请求调整当前内容大小，实际结果由 GeometryChanged 确认。resize increments 为可选的有限正尺寸，None 恢复平台默认。属性在创建返回前复制并应用，再按 visible／activation 请求显示窗口；不会先显示一个忽略所需属性的窗口。

AppKit 使用原生内容尺寸／桌面坐标、titlebar 按钮、window level、miniaturize／zoom、appearance 与 NSVisualEffectView。改变装饰、resizable 或 full-size-content 的 style mask 后恢复输入 view 为 first responder，保证键盘路由连续。模糊背景放在 renderer 宿主 view 后面，关闭模糊时恢复独立的 transparent 设置。content protection 使用系统分享提示，返回 Requested，不保证屏幕捕获系统遵守该提示。macOS 的 activation policy 和 titlebar／full-size-content 选项位于 platform.macos；应用标识只查询实际 main bundle，命令行进程没有 bundle identifier 时返回 None。

## 5. 输入与文本

KeyEvent 分离 PhysicalKey、LogicalKey、KeyLocation、state、repeat、modifiers 和 synthetic；LogicalKey 支持 Named／Character(String)／Dead／Unidentified，未知 native 键码保留实际编码来源，Darwin keyCode、Windows 扩展 scan code 或 Windows virtual key。修饰键含 Shift、Control、Alt、Super、锁定状态和可取得的左右侧信息。

AppKit 使用实际 NSEvent keyCode 和当前布局产生的字符；Command／Control 快捷键不产生普通文本提交。无装饰窗口仍可成为 key／main window，接收键盘不依赖原生标题栏。已交付的按下状态在失焦时补发 synthetic release，清空按住的修饰键并保留已知锁定状态；没有硬件来源的设备 id 为 None。CapsLock 通知表达锁定状态，不伪造物理按键释放。指针同时携带同次 native 事件的逻辑／物理位置与 scale，两种位置均以内容区左上为原点，物理位置由该次窗口 backing scale 换算；不能直接混入 backing surface 的底部原点。精确滚轮的 AppKit points 转换为物理 PixelDelta，普通滚轮保持 LineDelta，普通 phase 和 momentumPhase 分别保留。光标基础图标使用 NSCursor，可见性只在本窗口内容区域生效，退出该区域、失焦或销毁时平衡系统 hide／unhide。

文字统一由 `TextInputEvent.Edit(TextInputBatch)` 中的 Commit 操作交付；普通键、compose、IME 不得重复插入。TextInputEvent 只有 Enabled／Disabled／Edit。一次 AppKit 输入解释形成一个编辑批次，以一个事件调用 handler。范围为 UTF-8 字节区间且位于字符边界，平台 UTF-16 索引在 native 边界转换；非法 native 范围触发状态重同步，不越界访问。

本阶段提供 owner 线程上的 setImeAllowed 与 setImeCursorArea，采用 winit 0.30.13 的 AppKit 文本输入边界：NSTextInputClient 只保存当前 marked text 和候选窗矩形，不向系统发布完整编辑器文档，attributedSubstring 查询返回 nil。原生询问直接从这些 native 值回答，不进入 managed handler 或挂起。完整 surrounding 文档桥接、重转换与 purpose 扩展后续按实际编辑器需求增加。

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

纯值批次开始时核对 baseRevision，按顺序在临时文本上解释范围，整批通过后发布一次新快照。任何范围或 revision 失效均拒绝整批，不能部分应用。AppKit 基础桥产生 baseRevision=None、CurrentSelection 的 Preedit／Commit；普通文本不依赖 surrounding 快照。取消组合产生空 Preedit，删除应用正在显示的组合内容；不能只用 ClearPreedit 清除标记后再重复插入 Commit。原生 UTF-16 选区必须合法且不切断 surrogate pair，非法批次拒绝并撤销组合。未来需要 surrounding 的协议再接入文档快照与协议 serial，不为当前桥引入独立同步状态机。

AppKit 的 NSNotFound replacement 表示替换当前 marked segment 或应用选区；显式 replacement 只能引用已知 native marked segment，归一为整段 CurrentSelection 替换，选区先平移再转 UTF-8。不能把未知文档位置伪装成可用范围。无效 native 批次丢弃全部待交付操作；仅在批次开始前应用已有 preedit 时产生一次空 Preedit 撤销它，不能因拒绝一个新组合而误删应用原选区。

`TextInputState.apply(batch)` 提供上述字节编辑规则的纯值实现，供输入归一和应用同步快照使用，不承担文档布局。CurrentSelection 在存在 marked range 时替换组合文本，否则替换 selection；ClearPreedit 只清除 marked 标记。Preedit 插入后设置 marked range，并把相对 selection 平移到新文本；Commit 清除组合，光标落在插入末尾。DeleteSurrounding 删除 selection 之前／之后指定字节，保留选区正文与活动端方向，清除组合标记。成功批次统一使 revision 加一，空批次保持原值；过期 revision 报 IllegalStateException，非法范围、尺寸或 revision 溢出报 IllegalArgumentException，原快照保持不变。

Pointer 区分位置、按钮、进入／离开、滚轮；滚轮区分行／像素并保留相位。DeviceId 不可得时为 None。失焦清理组合与按下状态，必要的补发 release 标记 synthetic。后续 raw motion 使用设备单位，Touch 保留 contact id、phase 和可选压力，Gesture 只报告原生手势；这些扩展不是当前交付接口。

后续光标捕获使用 None／Confined／Locked，warp 与相对输入分离；custom cursor 的 RGBA 与 hotspot 在创建时复制。移动／resize／系统窗口菜单请求需要有效输入上下文的平台，由后端保留原生 serial 并核对，不能靠定时回调伪造用户操作。

## 6. 绘制与 native 句柄

requestRedraw 合并 dirty，RedrawRequested 不递归触发；隐藏、遮挡或零尺寸时保留 pending redraw，恢复后再处理。prePresentNotify 是提交前通知，不能等价为 present、vsync 或 GPU 完成。图形设备与渲染管线由上层 renderer 管理。

native 子包提供 Unsafe 的 acquireNativeWindow，返回 NativeWindowLease；handle 按目标声明为 `NativeWindowHandle.AppKit(nsWindow: Ptr<Unit>, nsView: Ptr<Unit>)` 或 `NativeWindowHandle.Win32(hwnd: Ptr<Unit>, hinstance: Ptr<Unit>)`。当前目标不声明其他后端的句柄变体。取得 lease 和查询 handle 只在 owner 上进行，lease 保活窗口和 display connection；窗口 Closing 后不能取得新 lease。

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
- ApplicationHost 提供 `isCurrentThread()`、`@Unsafe watchReadable(descriptor: Int, ready: () -> Unit): ApplicationWatch` 和 `@Unsafe watchWindowsEvent(event: Ptr<Unit>, ready: () -> Unit): ApplicationWatch`；选择与实际后端不匹配的通知在借用前报参数异常。监听不读取、复位或关闭借入 fd／HANDLE，不内联 ready。ApplicationWatch.remove 在 owner 上注销，当前 ready 退出、排队通知退休后回调一次。
- `postControl(block: () -> Unit): Boolean` 在关闭业务队列后继续接受收尾工作；控制闭包用 `requestExit()` 在 owner 开始关闭。`complete(failure: Throwable?)` 由 session.onClosed 在 owner 调用一次。无窗口时仍等待附着完成、watch 注销和已接受控制工作执行完。

WindowLoopHost 将这些实际操作适配为[asyncio 规范 11.3](SCOOP-ASYNCIO-SPEC.md#113-宿主接入合同)的 EventLoopNotification／watch。根取消订阅只排入控制队列，不跨线程直接操作窗口；集成 scope 仅在业务回调期间绑定。通知派发与纯 I/O／watch 维护唤醒不自行产生新一轮 NewEvents／AboutToWait，避免空闲自唤醒。

## 9. 构建与验收边界

Darwin 使用语言规范 12.2.1、实现规范 2.7 的普通 native Objective-C `.m`／ARC 与 framework 声明。AppKit backend 由 NSApplication 正常循环驱动，以 stop 和唤醒返回；不调用 terminate。通知监听以 dispatch READ source 观察 fd，通过 common／tracking／modal modes 的 CFRunLoopSource 在 owner 派发；等待消费时暂停重复通知，注销先平衡暂停并等待 cancel handler，再退休 owner source。渲染层自行管理实际 layer／surface。

本阶段以所属 Cone 的 scoop.test 验证值、输入归一、API、真实 AppKit、native callback 与 asyncio 收尾；`.m` 编译／链接／诊断使用必要的 compiler fixture。真实 surface 示例用单独 renderer Cone 消费 native lease。显示器热插拔、混合 DPI、输入法候选交互与真实用户拖动等依赖实际硬件／桌面输入的验收单独记录，不能用合成事件冒充。Linux／其他平台验收留待后续。

## 10. Win32 后端

Windows 窗口使用 Unicode HWND，创建前确认 Per-Monitor V2；已声明其他 DPI 模式时明确失败，不改变现有 UI 的坐标合同。每次 WM_DPICHANGED 使用建议窗口矩形并重新读取内容像素尺寸，原子发布完整 metrics。内容逻辑坐标以窗口 DPI 换算；DesktopPosition／DesktopRect 使用物理虚拟桌面坐标。最小化提供零 drawable，隐藏／最小化期间保留 dirty；Windows 不提供普通顶层窗口完整遮挡查询，isOccluded 只报告系统可确认的最小化或 DWM cloaked 状态。

窗口属性通过正常 Win32／DWM API 实施，原生返回值在边界保留并转换为 WindowException。native 薄层的创建结果同时返回资源和状态码；setter 以状态码报告失败，不使用另一个错误缓存。Windows 8 起 DwmEnableBlurBehindWindow 不产生模糊效果，setBlur(true) 返回 Unsupported 且无副作用；要求 blur 的创建失败。主题建议使用 DWM 支持的暗色标题栏属性；缺少该属性时返回 Unsupported。透明背景供实际 renderer 使用，不承诺所有渲染设备都有 alpha 支持。内容保护为 WDA_MONITOR 系统提示，仍返回 Requested。

键盘的物理位置来自扩展 scan code，逻辑文字查询当前布局且不改变系统 dead-key 状态。普通文字仅由 WM_CHAR／WM_UNICHAR 交付；IMM32 result 和 preedit 以一次 WM_IME_COMPOSITION 为批次，UTF-16 选区转换为 UTF-8 字节位置。确认结果不再通过 WM_IME_CHAR 重复提交；失焦／禁用取消组合并补发已交付按下键的 synthetic release。候选窗使用内容像素坐标，native 同步查询不进入用户 handler。surrounding 文档与 TSF 文档存储不属于当前基础 IME。

宿主使用 MsgWaitForMultipleObjectsEx 与 owner HWND。线程池 wait 一次只投递一个 HWND 就绪消息，owner 回调退出后重新监听；remove 停止 native wait 并等候 worker 退出，在 owner 排队消息和活动 ready 退休后报告完成，HANDLE 始终由 asyncio loop 持有。截止时间只安装所需原生 timer，Wait 不做周期轮询。真实系统 dialog、menu、move／size 循环继续派发 HWND 消息，因此能推进 asyncio；会进入模态的操作排在当前业务回调和 session pump 返回后。
