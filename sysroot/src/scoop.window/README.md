# scoop.window

面向上层 GUI／renderer 的窗口骨架，能力边界参考 winit 0.30.13。本阶段实现 macOS AppKit；Linux、Windows 和其他窗口后端留到后续。

库负责窗口、原生事件、重绘请求、显示器快照和显式生命周期。控件、文档布局、菜单模型、剪贴板、无障碍语义和图形设备由上层实现。窗口库和 `scoop.window.asyncio` 都是普通 Scoop Cone，AppKit 薄层通过普通 `.m`／ARC native 构建接入。

## 依赖与入口

```toml
[dependencies]
"scoop:scoop.window" = "0.1.0"
```

在主线程、活动 Job 之外创建 `Application`，以 `run(handler)` 驱动 `ApplicationHandler.onEvent`。一个进程只能创建一个 Application，并运行一次。`Resumed` 中使用 `ActiveApplication.createWindow`；原生关闭按钮产生 `CloseRequested`，由应用决定销毁窗口或退出。

默认关闭最后一个窗口后继续运行；使用 `ApplicationOptions(LastWindowPolicy.Exit)` 可在最后窗口实际销毁后退出，也可以主动调用 `ActiveApplication.requestExit()`。运行结束会返回调用方。

[Metal 示例](examples/metal/README.md)给出了完整可运行入口和独立 renderer Cone。准备好[开发 sysroot](../../../scripts/README.md)后，在仓库根运行：

```sh
export SCOOP_SYSROOT="$PWD/sysroot"
target/debug/scoop run sysroot/src/scoop.window/examples/metal/app --release
```

## 当前能力

| 能力 | 接口与行为 |
| --- | --- |
| 主循环 | Wait、Poll、单调 WaitUntil；跨线程 `ApplicationProxy.post`／`requestExit`；回调串行交付 |
| 窗口 | 标题、显示／隐藏、焦点请求、尺寸及约束、桌面位置、装饰及按钮、窗口级别、最小化／最大化、主题、透明／模糊、内容共享提示、注意力请求 |
| macOS | activation policy、titlebar 属性、bundle identifier 查询 |
| 显示器与 DPI | 拥有数据的显示器快照、可选名称／桌面位置／刷新率；窗口完整 logical／physical／scale metrics |
| 输入 | 物理键、当前布局逻辑键、重复、左右修饰键、未知键码、指针位置／按钮、行／像素滚轮及 momentum、基础光标和区域内隐藏 |
| 文本 | 普通文字与 IME 共用编辑批次；marked／preedit、commit、候选窗位置、失焦及禁用时撤销组合 |
| 绘制 | 合并重绘请求；隐藏／遮挡／零尺寸时保留 dirty；显式 native lease |

`Window` 操作和原生句柄查询都要求 owner 线程。`RequestStatus.Requested` 表示已向系统提交请求，实际结果由事件或快照确认；例如焦点请求不保证系统一定授予焦点。

内容区坐标以左上为原点。`LogicalPosition`／`LogicalSize` 使用窗口逻辑单位，`PhysicalPosition`／`PhysicalSize` 使用像素。使用同一次 `WindowMetrics` 或输入事件的 scale 换算；桌面坐标不能乘某个窗口的 scale。未知的原生信息保留为 `None`。

全屏控制／显示模式切换、全局 raw device、触摸／高级手势、光标捕获／自定义光标、文件拖放和复杂平台窗口交互属于后续范围。完整映射见 [WINIT-MAPPING.md](../../../docs/windowing/WINIT-MAPPING.md)。

## 文本输入

文本输入默认禁用 IME；普通可打印键仍通过 `TextInputEvent.Edit` 交付。调用 `setImeAllowed(true)` 接入 AppKit 输入法，`setImeCursorArea` 发布候选窗矩形，坐标为内容区逻辑坐标。Enabled／Disabled 表示窗口的 IME 接入开关变化，重复设置相同值不重复发事件。

应用只从编辑批次插入文字，不再从 `KeyEvent.logicalKey` 额外插入字符。输入法消耗的候选选择／确认键不会再作为普通按键交付；普通快捷键保留 Keyboard 事件。失焦会撤销 active preedit，并为已交付的按下状态补发 synthetic release。

`TextInputState.apply(batch)` 是可选的纯值编辑辅助：按次序应用整批操作，全部合法后才发布一次新 revision。范围使用 UTF-8 字节边界；AppKit 的 UTF-16 范围在 native 边界转换。取消组合的空 Preedit 会删除当前显示的组合文本；ClearPreedit 只清标记，不能代替取消。

本阶段的 AppKit 桥只保存当前 native marked text 和候选窗矩形，不保存应用文档。它产生 `baseRevision=None`、`CurrentSelection` 的 Preedit／Commit；已知 marked segment 内的原生替换先归一为整段替换。无效范围拒绝整批，只在应用已收到旧 preedit 时发一次撤销。文档 surrounding、重转换、purpose 和编辑器布局后续按实际需要扩展。

## renderer 与关闭顺序

`requestRedraw()` 合并请求；在 `RedrawRequested` 中确认 metrics 可绘制，调用 `prePresentNotify()` 后由 renderer 提交帧。窗口库不创建 Metal／Vulkan／OpenGL 设备，也不把 WaitUntil 当作显示器时钟。

renderer 在 owner 上通过 `scoop.window.native` 的 Unsafe `acquireNativeWindow()` 取得 `NativeWindowLease`。本阶段的 AppKit handle 包含 NSWindow 和内容 NSView。lease 保活原生窗口；`close()` 可跨线程且幂等，实际释放回到 owner。调用 close 后不得继续使用句柄，GC 不代替显式 close。

`requestDestroy()` 开始 Closing；Closing 后不再接受业务输入和重绘。renderer 在 Closing 中停止提交、销毁 surface 并 close lease；等全部借用和回调退休后，库才销毁原生窗口并发出最终 Destroyed。

## asyncio

同时声明 `scoop:scoop.asyncio` 和 `scoop:scoop.window.asyncio` 依赖，导入对应包，使用 `Application.runWithAsyncio(handler)`。业务回调可取得 `currentEventLoop()` 和 `currentApplicationScope()`。跨线程业务通过 proxy 返回 owner。

退出／根取消先开始窗口收尾，并继续 AppKit 循环直至子任务清理、native lease、通知 watch 和 session 全部结束。Exiting／Closing／Destroyed 在收尾 Context 中交付，没有活动业务 scope。dispatch READ 通知在 AppKit common、tracking 和 modal modes 中服务；不靠固定 tick 轮询。

## 验证

在已登录的 macOS 图形桌面及准备好的 Darwin sysroot 上运行：

```sh
target/debug/scoop test sysroot/src/scoop.window --release
target/debug/scoop test sysroot/src/scoop.window.asyncio --release
target/debug/scoop test sysroot/src/scoop.window/examples/metal/app --release
```

这些测试使用 `scoop.test`，包括真实 AppKit 窗口、NSEvent 与 NSTextInputClient 协议调用、UTF 范围／编辑批次、跨线程投递和 lease、真实 modal／menu tracking 中的 I/O，以及实际 Metal drawable 提交。Metal 验收需要可用图形设备。

显示器热插拔、混合 DPI、不同键盘布局、系统输入法候选交互和真实用户拖动仍需实际桌面／硬件验收；协议调用测试不等同于这些手工验证。实现与验证记录见 [PROGRESS.md](../../../docs/windowing/PROGRESS.md)。
