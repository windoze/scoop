# 桌面窗口实施记录

状态：基础架构／macOS 已完成；Windows 骨架实施中（2026-10-30）。使用方式见[窗口库 README](../../sysroot/src/scoop.window/README.md)。实际硬件／输入法交互的验证边界在各批次与文末记录。

基础阶段交付 `scoop.window`、`scoop.window.asyncio` 与真实 AppKit 后端；当前 Windows 阶段复用共享架构接入 Win32。Linux／其他后端后续实施，不添加空实现。窗口逻辑、平台适配与测试均归属普通 Cone；编译器只补齐实际需要的 `.m` native 能力及实施中暴露的底层缺口。

当前交付按用户进一步明确的 winit 定位收敛为 GUI 骨架，先完成窗口、基础属性／输入／IME、DPI／显示器、renderer 与 asyncio。完整能力地图中的高级平台扩展分批补充，不把完整 AppKit GUI 或全部 winit 扩展变成当前验收门槛。

实施顺序：

1. Darwin `.m`／ARC／独立 `objc_flags`，验证正常 framework 与 Objective-C 链接。
2. 同步窗口规范，迁入统一的 asyncio notification／watch 接口。
3. 公共值类型、Application／Window 生命周期、proxy、native lease 与真实 AppKit 主循环。
4. 窗口属性、输入／文本批次、重绘、显示器及 macOS 平台能力。
5. 窗口宿主与 asyncio 接入、取消和退出收尾。
6. Cone 测试、真实 surface 示例、回归验收与文档。

每批完成后先格式化／lint，再运行相应测试，提交并推送分支，检查并合并远端 main。原始日志与临时验收产物保存在仓库外；本文只记录范围、结果与实际限制。

## Windows 阶段

### 1. COFF import 选择

真实窗口依赖 kernel32，platform 依赖 ntdll；两者都提供 RtlLookupFunctionEntry。修复默认 namespace 将这些惰性导入误报为重复强定义的问题，按既有 native 输入顺序选择，并让最终 PE link 保持相同顺序。显式库归属和普通对象强定义冲突仍按原规则验证。

验证：Rust fmt／workspace clippy 与 linker 的 4 项测试通过；新增 import-overlap 和现有 native-data fixture 在 Microsoft UCRT、LLVM-MinGW UCRT／MSVCRT 全部通过，共 6 次 fixture、33 次进程调用。先修复底层链接器，再继续窗口库，没有删除正常系统依赖绕过问题。提交：`df0d24ef9`。

### 2. Win32 主循环、窗口与 asyncio

- 新增普通 Cone 的 Unicode Win32 C 后端，复用共享 Application／Window owner、proxy、关闭队列和 lease。WindowException 保留实际 Win32 错误码；native handle 按当前目标声明 AppKit 或 Win32，未知键码保留来源。
- 创建 HWND 前设置／确认 Per-Monitor V2；窗口属性、实际尺寸／DPI、显示器快照、基础键盘／指针／文本、IMM32 marked／commit 与候选窗矩形接通。真正不受支持的 blur 返回 Unsupported，不引入 undocumented DWM 调用。
- 一次性 threadpool wait 只向 HWND 投递通知；Scoop ready 在 owner 执行，remove 等 native callback 和活动 ready 退出后完成。取消退休通过已有 wake 路径推进，不关闭或 reset 借入 HANDLE。
- window.asyncio 使用 WindowsEvent 连接现有 IOCP session。自动化测试进入真实 MessageBox、TrackPopupMenu 和系统尺寸调整循环，验证 timer 继续推进及根取消 → Closing → 异步 finally → lease 释放 → Destroyed。

验证：Scoop fmt／check、native 警告检查通过；三个 Windows target 各通过 window 的 12 项 release 测试、窗口 asyncio 的 1 项测试、独立 DPI 失败 Cone 的 1 项测试，共 42 项。测试覆盖双窗口、Unicode 标题和输入、Context、跨线程投递／lease、重绘合并、非法状态和不兼容 DPI 模式。预编译依赖来自配套 Windows sysroot，窗口生产库以普通 release build 显式更新。Mac 的共享 native 状态返回 ABI 已同步，本 Windows 宿主未执行 AppKit 回归。

DXGI renderer、扩展输入／收尾组合和最终交付审查仍在本阶段后续批次；下文为已完成的 macOS 历史记录，不能将其人工验收限制视为 Windows 验收结果。

## 已完成

### 1. Darwin Objective-C native 管线

- `.m` 使用所选 Apple Clang、ARC 与独立 `objc_flags`；条件筛选先于目标限制，禁止以 flags 覆盖 ARC／输入语言，`.mm` 明确拒绝。参数、头文件与对象复用沿现有 native 构建记录处理。
- PM／scoopc 展开配置同步到 package protocol 3／compiler protocol 23；未增加语言语法、runtime ABI 或窗口专用产物。
- 实际 AppKit 链接暴露并修复两项底层缺口：多个 framework 重导出同一最终定义不再误报歧义；Apple Clang 的 selector stub 交给 Darwin linker 合成，并正常解析真正的 `objc_msgSend` 运行库依赖。
- 同步窗口规范中已过时的文本编辑批次、native lease 与跨 Cone 宿主接口；Linux／其他后端不属于本阶段。

验证：workspace fmt／clippy 通过；manifest／protocol／toolchain 共 176 个 Rust 测试通过，linker 测试通过；fixture runner 公共规则测试通过。Objective-C 3 个 fixture、4 个变体、12 次真实进程调用通过，覆盖 debug／release、ARC 弱引用、AppKit／Foundation、混合 C、直接复用、flags 变化及错误位置。现有 native C++ 10 个适用 fixture（13 个变体、40 次进程调用）通过；其余 3 个目标不适用项未计为通过。开发 sysroot 的 25 个 Cone 与 runtime 已显式准备。

### 2. 普通窗口 Cone 的值基础

- 新增 `scoop.window`，提供独立实体 id、窗口错误／状态／能力类型、逻辑与物理坐标、完整 metrics，以及向上取整与溢出检查。
- 文本输入提供带方向的 UTF-8 选区、快照、有序编辑批次及纯值应用；整批校验、一次 revision 更新，覆盖临时文本范围、组合替换与 surrounding 删除。
- sysroot 维护构建与文档共用目标源码清单；Darwin 专用 Cone 不进入 Linux 清单，依赖不可用仍正常报错，不增加编译器配置或平台空实现。

验证：Scoop 格式化／check 通过，所属 Cone 的 9 个 release 测试通过；Ruff 0.16.10 格式化／lint 通过，脚本的 12 个测试通过。AppKit 后端与应用对象尚在下一批实施。

### 3. asyncio 统一通知接口

迁入 `EventLoopNotification`／`EventLoopHost.watch`，同步 Unix descriptor 提供方、Darwin MainDispatchHost 与实际测试宿主，删除旧 asyncio watchReadable 接口；未迁入 Windows I/O 实现。Darwin 在借用资源或派发前明确拒绝 Windows event 类型。

验证：Scoop 格式化／check 通过；asyncio 完整 release Cone 测试 62 个容器、152 项全部通过，包含现有网络、文件、TTY、取消及宿主注销组合。

### 4. 真实 AppKit 主循环与显式窗口寿命

- 普通 Cone 的 `.m` 薄层拥有 NSApplication／NSWindow／NSView，使用可返回的 run／stop 与 CFRunLoop 唤醒；原生通知交给 Scoop 串行派发。
- Application 单进程／单次／主线程入口、run Context、跨线程 FIFO proxy、Wait／Poll／单调 deadline、完整窗口 metrics、可见性／激活及重绘合并已接通。
- Closing／Destroyed 与 native lease 采用显式寿命；跨线程 close 原子结束借用，由控制队列在 owner 销毁原生窗口。退出原子关闭控制入口，已接受清理不能落在停止循环之后。
- 加入供集成 Cone 使用的 runAttached／post／complete 接入面；通知 watch 和实际 asyncio 适配仍在下一批。

验证：Scoop 格式化／check、native clang-format／Wall／Wextra／Werror 通过；真实 AppKit 与值测试共 10 项 release 测试通过。真实窗口覆盖隐藏后显示、两轮非递归重绘、跨线程错误／投递、run Context 保留、双窗口显式关闭及跨线程 lease 退休，并在过程中执行 GC。尚不把此测试视为 IME、真实拖动或完整平台能力验收。

### 5. 窗口宿主与 asyncio 收尾

- 每个借入 descriptor 由独立 dispatch READ source／CFRunLoopSource 驱动；待 owner 消费期间暂停重复通知。remove 平衡暂停、等待 cancel handler、注销 owner source，再报告完成；不读取或关闭借入 fd。
- 新增普通 `scoop.window.asyncio` Cone，复用 PlatformEventLoop.attach、根 Context 与 Job；业务绑定 currentApplicationScope，根取消经控制队列开始窗口关闭，最终通知不提供活动 scope。
- 循环同时等待窗口、lease、watch、session 与已接受控制工作退休。补齐最后窗口策略触发退出时也必须先交付 Exiting 的边界。

验证：两库 Scoop 格式化／check 与 native 警告检查通过；window 的 10 项 release 回归通过。集成 Cone 的真实 AppKit 测试通过，确认 NSApplication 模态窗口、NSMenu 跟踪期间 asyncio timer 均推进，并覆盖根取消 → Closing → 子任务异步 finally → lease 释放 → Destroyed／退出。测试没有用自制 poll 循环替代 AppKit；真实鼠标拖动仍待单独验收。

### 6. 真实 Metal surface 闭环

新增两个普通示例 Cone：renderer 通过 native lease 安装 CAMetalLayer、自行创建 Metal queue／drawable，应用在 RedrawRequested 提交一帧，Closing 时先恢复 layer 再释放 lease。没有把图形设备或帧调度放进窗口库。

首帧验收暴露并修复 AppKit 通知遗漏保存计算后载荷的问题：GeometryChanged 现在交付完整实际尺寸／scale，焦点与遮挡通知也保存实际状态；增加事件与已公布 metrics 一致的断言。mailbox 直接复用现有 platform Mutex。

验证：格式化／check 与 native 警告检查通过；Metal 的 release Cone 测试实际提交并等待 GPU 完成后关闭，window 的 10 项回归与 asyncio 的模态／取消组合复核通过。示例仍是单帧 renderer，不承担多帧 GPU 调度。

### 7. 基础窗口属性与 macOS 选项

新增尺寸约束／resize increments、桌面位置、装饰与按钮、窗口层级、miniaturize／zoom、主题、透明与模糊、content-sharing 提示和用户注意请求。创建先复制并应用属性，再显示／激活；运行中以实际原生快照和通知确认结果。AppKit activation policy、titlebar／full-size-content 与 main bundle 标识位于 platform.macos。

按用户明确的 winit 定位收敛当前范围，并核对 winit 0.30.13 的 AppKit IME 边界：本轮只接入 marked／commit 和候选窗位置，不引入完整编辑器文档镜像。TextInputState 保留为应用纯值辅助。

验证：Scoop 格式化／check、native 格式与警告检查通过；window 的 10 项 release 测试、Metal 的实际 GPU 首帧及 asyncio 模态／取消测试通过。Metal 复核曾因桌面遮挡而停在等待；自动关闭的 GPU 测试显式使用置顶窗口，正常示例保持普通层级，并保留超时状态诊断。

### 8. 显示器与 DPI 快照

新增 monitors／primaryMonitor／currentMonitor。AppKit 枚举复制名称、桌面矩形、backing 像素尺寸与比例；刷新率仅使用 CoreGraphics 实际给出的正值。Scoop 分配 MonitorId，屏幕变化时退休已离开的身份，不暴露原生指针或编号，也不提供未实施的视频模式控制。

验证：格式化／check 与 native 警告检查通过，window 的 10 项 release 测试通过。真实显示器枚举覆盖重复查询身份稳定、主显示器／窗口显示器一致、尺寸与缩放，以及 native 快照释放和 GC 后在工作线程读取自有值；工作线程调用原生枚举明确失败。热插拔和混合 DPI 跨屏拖动仍是硬件交互验收项。

### 9. 基础键盘、指针与滚轮

新增物理／逻辑键、位置、repeat／synthetic、未知 native 键码、左右修饰键及可选锁定状态。AppKit 失焦补发已按下键的释放并保留 Caps Lock；Command／Control 快捷键不生成普通字符提交。指针提供同次窗口坐标与 scale、按钮／点击次数、行／像素滚轮和独立 momentum phase；系统基础 cursor 及区域可见性使用平衡的 NSCursor hide／unhide。

原生事件在离开队列前转换为自有 Scoop 事件，UTF-8 文本复制后才释放 native owner。移除没有实现入口的 WindowFeature、触摸身份及激活结果预留。记录 winit 平台参考与许可证，硬件键码另按 Apple SDK 核对。

验证：格式化／check 与 native 警告检查通过，4 个容器的 12 项 release 测试全部通过。真实 NSEvent 调用实际视图，覆盖按键重复、未知键码、左右 Shift、synthetic release、普通文本唯一通道、GC 后载荷、指针位置、滚轮单位／相位及光标隐藏恢复；发现并修正 backing 点原点与公开左上内容坐标的混用。AppKit 自动化窗口置顶以排除桌面遮挡；协议调用不视为真实键盘布局或输入法交互验收。

### 10. 同步远端 main 的 Windows 基础能力

合入 main 的 `b0de037002`，保留其 Windows native driver、统一 native I/O 状态及通知实现，同时保留本分支的 Darwin `.m`／ARC 和窗口 Cone。compiler protocol 更新为 24，package protocol 保持 3；失败版本测试按实际 CBOR 整数长度构造，不再假设版本只有一个编码字节。开发 sysroot 和文档清单同步全部目标筛选，消除合并后的重复 Unix source 声明；修正 macOS 临时目录规范路径的测试期望。

验证：workspace fmt／clippy、184 项 manifest／protocol／toolchain 测试及 14 项 linker 测试通过；脚本与 fixture runner 公共规则测试通过，目标不适用项正常跳过。Objective-C 3 个 fixture／4 个变体／12 次进程调用和 C++ 10 个适用 fixture／13 个变体／40 次进程调用通过。显式重建 Darwin 的 27 个 Cone 与四种 runtime 产物；asyncio 的 64 个容器／153 项 release 测试、window 的 12 项测试、窗口 asyncio 模态／取消测试及 Metal 首帧测试全部通过。Windows 窗口后端仍留待后续阶段。

### 11. AppKit IME 与本阶段交付

新增 setImeAllowed／setImeCursorArea 和真实 NSTextInputClient。普通文字与 IME 使用同一组自有编辑批次；marked segment 内的 UTF-16 替换及选区转为 UTF-8，支持同批提交后继续组合。无效批次全部拒绝，只撤销先前已交付的组合，保留应用原选区；失焦、禁用及销毁清理 native marked 状态。Enabled 先于本次开启产生的编辑，重复设置不重复通知，确认组合的按键不会再执行普通编辑。

按 winit 的 AppKit 规则补齐无装饰窗口的 key／main 能力，以及 style mask 变更后的 first responder 恢复。Metal 示例在标题栏显示普通／IME 文字结果；增加窗口 asyncio 定时标题示例。公共 README、规范与能力映射明确当前骨架、后续平台与高级功能的边界，移除未实施的 purpose 预留。

验证：Scoop 格式化／check、native clang-format／Wall／Wextra／Werror 通过。window 的 12 项 release 测试通过，其中真实 AppKit 集成覆盖无装饰窗口焦点、interpretKeyEvents 命令键、11 个有序文本批次、组合内替换、emoji 边界、非法范围整批拒绝、重复 unmark／开关、候选窗坐标及 GC 后载荷。更新 27 个 Cone 的开发 sysroot 后，窗口 asyncio 模态／取消测试和 Metal 实际 GPU 首帧复核通过；定时标题示例通过 release 检查和链接。

本阶段提供可用的 macOS 窗口／输入／renderer／asyncio 骨架。Linux、Windows 和其他窗口后端，以及全屏控制、raw device、触摸／高级手势、捕获和文件拖放留待后续。显示器热插拔、混合 DPI、各键盘布局、系统输入法候选交互及真实用户拖动未做人工验收；原生协议调用和实际 modal／menu tracking 测试不替代这些硬件／桌面交互验证。
