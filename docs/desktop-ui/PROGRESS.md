# 原生桌面能力实施记录

开始日期：2026-11-07。开发分支：`codex/native-desktop`。目标为完整实施[主设计](DESIGN.md)和[窗口／对话框设计](WINDOWS-AND-DIALOGS.md)，最终交付三平台安装包；本文件只记录实际进度与必要限制。

## 1. 工作约定

- 每个完整大项先在本地实现、格式化、检查并验证，再到适用的其他平台验证；不为每个小改动重复全量测试。
- 每项完成后提交、推送，再 fetch 并合并 origin/main；若合并影响已验证范围，执行对应的必要复验。
- Linux 使用 ssh nuc12 下 ~/repos/ 内的新目录；Windows 使用 /usr/bin/ssh 192.168.2.67 下 E: 内的新目录。Windows 经 SSH 执行构建和可运行测试，GUI 交互由用户随后验证。
- 原始日志、临时探针与安装包保存在仓库外；每轮结束清理已经不再需要的临时文件，保留后续构建需要的正常增量缓存。
- scoop.tray 与 scoop.ui.component.tray 独立交付，普通 window／UI 不引入额外托盘依赖。

## 2. 批次状态

| 批次 | 范围 | 状态 |
| --- | --- | --- |
| 准备 | 设计基线、开发分支、验证与提交约定 | 已建立分支；设计进入版本管理 |
| D1 | 公共合同、跨 Cone 原生使用、平台关键组合 | 已实施菜单身份、图像、能力、NativeMenuLease 与应用资源登记；平台验证限制见下 |
| D2 | Menu／Popup、快捷键、原生菜单借用 | 已实施；macOS／Linux 运行验证及 Windows 三目标构建完成，桌面交互限制见下 |
| D3 | 独立 Tray Cone、定位、服务恢复与关闭 | 已实施；macOS／Linux 运行验证及 Windows 三目标构建完成，桌面交互限制见下 |
| D4 | Frame／Region、原生外观与系统操作 | 已实施；macOS／Linux 运行验证与 Windows 三目标构建、非 GUI 测试通过，桌面交互限制见下 |
| D5 | Owned／Tool／Raise／Modal 与关闭依赖 | 已实施；macOS／Linux 运行验证与 Windows 三目标构建、纯值测试通过 |
| D6 | 原生文件／目录对话框 | 已实施；macOS、Linux 本地选择器及真实 portal 验证完成，Windows 三目标构建和路径 ABI 测试通过 |
| D7 | UI 应用根、资源事件、Surface 发布边界 | 已实施；macOS／Linux 运行验证及 Windows 三目标构建、非 GUI 测试通过 |
| D8 | 通用组件及独立托盘组件 | 已实施；macOS／Linux 运行验证及 Windows 三目标构建、非 GUI 测试通过 |
| D9 | 综合 desktop demo | 已实施；macOS 两架构、Linux 两后端的完整流程及 Windows 三目标构建、无 GUI 启动验证通过 |
| 发行 | macOS、Linux、Windows 安装包及脚本修复 | 三平台归档及安装验证完成；Windows GUI 留待桌面手工验证 |

## 3. 验证记录

设计基线已检查本地文档链接、章节、D1～D9 依赖顺序和格式。开发分支已在更新后的 main（2bd06a47，包含 Linux GUI 与 Windows TSF 合入）上重建。

2026-11-07，D2：已加入[菜单规范](../specs/SCOOP-WINDOW-MENUS-SPEC.md)与三个原生后端，包含稳定身份、CPU 图像、标准角色、实际快捷键、菜单栏、popup session、tracking 中更新及跨 Cone 菜单借用。关闭立即停止原生动作，解绑和销毁等待原生使用退休。原生 popup 从应用控制回调启动，保证 watch 先返回并重新挂接通知源。

- macOS aarch64：Scoop 格式化、原生严格编译检查和窗口库构建通过；菜单 Cone 2 passed、1 ignored；asyncio 的真实 popup tracking／I/O／关闭组合 1 passed。忽略的是独立编辑焦点用例：当前桌面处于 loginwindow，须在解锁的前台桌面单独运行。
- Linux GNU：窗口库和测试构建通过；X11 与带输入 seat 的嵌套 Wayland 各 2 passed。覆盖实际菜单栏展开、tracking 中更新、GTK 快捷键只派发一次、原生模型／借用、popup 选择和目标窗口关闭。
- Windows：MSVC UCRT、LLVM-MinGW MSVCRT、LLVM-MinGW UCRT 的窗口库和完整测试程序均编译通过；各目标默认测试 1 passed、1 ignored。SSH 会话无法创建测试所需的原生 Application，原生窗口／菜单用例保留到桌面会话执行；这些结果不代表 GUI 交互验收通过。

测试入口、所需桌面条件及独立焦点／Windows 用例的执行方式见[菜单测试说明](../../sysroot/src/scoop.window/tests-cones/menus/README.md)。macOS amd64 构建与三平台完整桌面组合继续随后续批次和最终打包验收推进。

2026-11-07，D3：已加入独立 [scoop.tray](../../sysroot/src/scoop.tray/README.md) 与[托盘规范](../specs/SCOOP-TRAY-SPEC.md)，实现 AppKit status item、Win32 通知图标及专用消息 HWND、Linux SNI／DBusMenu。应用资源登记与原有 asyncio attachment 共存，退出等待原生使用和异步回调退休。菜单取消立即退休旧动作，程序化 session 结束后可重新借用同一 Menu。Win32 外部菜单保存打开前的窗口／编辑焦点，避免托盘宿主改变标准命令的目标。

新增实际图标锚点、Monitor.workArea、系统栏查询能力和邻近窗口定位。X11 同时补齐 WM 外框查询与桌面单位转换；Wayland 保留无普通窗口全局定位／工作区查询的边界，SNI 只报告宿主提供的 PointHint。libdbusmenu-glib 的准备、头文件、链接输入和许可单独归托盘，普通 window／UI 不依赖它；目标生产清单已确认 GNU 包含托盘、musl 不包含托盘。

- macOS aarch64：格式化、原生严格编译、窗口／托盘／window.asyncio 构建通过；托盘 5 passed、1 ignored，包含无窗口资源与 asyncio 延迟清理组合；菜单复验 2 passed、1 ignored，原生 popup／asyncio 组合复验 1 passed。真实状态栏 popup 的取消／再次展示／选择用例已编译，当前 loginwindow 下不执行，须在解锁桌面以 `--ignored` 单独验证。
- Linux GNU：独立准备 libdbusmenu-glib 开发资源及增量复用通过；窗口、window.asyncio、托盘与测试构建通过。X11 与带 seat 的嵌套 Wayland 各 5 passed，覆盖实际总线协议的多分辨率 ARGB、DBusMenu 图像／快捷键／状态／子菜单、更新后稳定命令身份、退休动作、Watcher 消失与返回、激活提示、关闭中的异步请求。X11 验证基于实际外框和工作区的位置请求，Wayland 验证 Unsupported。菜单复验各 2 passed。这是协议与软件显示会话验证，不替代具体桌面 Shell 中的图标呈现验收。
- Windows：MSVC UCRT、LLVM-MinGW MSVCRT、LLVM-MinGW UCRT 的窗口／window.asyncio／托盘库及完整测试程序均编译通过；各目标托盘 4 passed、1 ignored，菜单复验 1 passed、1 ignored。忽略项需要交互桌面和 Explorer，包含通知回调、图像与自己图标的重新注册；SSH 结果不代表 GUI 验收通过。
- 打包入口与许可收集已接入托盘；相关 Python 检查通过，包组装 5 项、许可收集 4 项测试通过。完整发行包继续在 D9 后统一构建。

2026-11-07，D4：已加入[窗框规范](../specs/SCOOP-WINDOW-FRAME-SPEC.md)，以 WindowFrame 替代 decorations，提供 System(Standard／Extended)、Client、实际窗框／系统按钮几何、阴影与 Windows DWM 外观属性。原生区域区分默认、空集、窗口裁剪和独立输入覆盖；尺寸／DPI／窗框改变后撤销旧快照。系统拖动／缩放／窗口菜单保留真实输入使用，等业务回调退栈再运行，接管时取消原按下序列；退出取消尚未执行的操作并等待使用退休。

graphics 使用 Skia 提供 Path／CanvasGeometry 的显式二进制区域转换，Surface 收到 FrameChanged 后重绘；UI 对 Accepted 帧发布窗框区域仍归 D7／D8。GTK 直接使用的 Cairo 链接输入已进入准备与打包清单，托盘依赖仍保持独立。Windows graphics 显式声明 Skia D3D 对象需要的 uuid 链接输入，独立消费不再依赖 window 间接带入它。

- macOS aarch64：Scoop 格式化、原生严格编译和相关库构建通过；窗框 3 passed，graphics 区域 3 passed，菜单复验 2 passed、1 ignored，asyncio 收尾组合 1 passed。覆盖真实 Surface CAMetalLayer、菜单栏、运行中 opaque 同步、输入使用与关闭顺序；当前 loginwindow 下不将合成 NSEvent 准备请求等同于前台实际拖动／缩放或屏幕透明呈现验收。
- Linux GNU：当前分支 CLI、基础依赖、Window／graphics／Surface／window.asyncio 构建和原生严格检查通过。无合成器 X11、有合成器 X11、带 seat 的嵌套 Wayland 各 3 passed；有合成器 X11 与 Wayland 在 2 倍缩放下再次各 3 passed。覆盖真实 GTK 模式与按钮几何、菜单栏下的客户区尺寸、X Shape 输入区域、XTest 输入取消，以及改尺寸后的 Surface Accepted。菜单复验 X11／Wayland 各 2 passed，graphics 区域 3 passed。真实用户拖动及具体桌面 Shell 的呈现仍需桌面验收。
- Windows：MSVC UCRT、LLVM-MinGW MSVCRT、LLVM-MinGW UCRT 均完成当前分支所需库、完整窗框／DWM GUI 用例及透明 Surface 呈现用例的编译；每个目标窗框 2 passed、1 ignored，菜单 1 passed、1 ignored，graphics 区域 3 passed。SSH 不运行 GUI 用例，桌面执行方式见[窗框测试说明](../../sysroot/src/scoop.window/tests-cones/frame/README.md)。
- GTK 准备及增量复用通过，Python 包组装测试 5 passed。完整发行包仍在 D9 后统一构建。

2026-11-07，D5：已加入[窗口归属与模态规范](../specs/SCOOP-WINDOW-MODAL-SPEC.md)，实现 Owned／Tool、raise、实际层级查询、owner 隐藏／最小化时的显示意图恢复，以及 Owner／Application 模态 session。不同模态的输入限制取并集，新的窗口立即遵守范围；旧 popup／系统操作在目标受阻时取消，原有禁用状态在最后一个限制解除后恢复。AppKit 使用普通及嵌套 sheet 和真正的 application-modal 循环；GTK 每窗口原生组隔离 modal grab。关闭等待后代展示、Surface／native lease 退休，再按子到父顺序销毁。

- macOS aarch64：格式化、原生严格检查和相关库构建通过；模态 3 passed，窗框复验 3 passed，菜单 2 passed、1 ignored，asyncio 1 passed，主窗口测试程序编译通过。覆盖两个独立 Owner 模态、嵌套 sheet／Application 模态、实际 loopback TCP 和定时取消、被阻塞窗口的原生关闭资格、隐藏／最小化恢复，以及父窗口等待子窗口 lease。当前 loginwindow 下仍不宣称前台焦点、实际用户交互与视觉叠放验收通过。
- Linux GNU：原生严格检查与 Window／window.asyncio／Surface 构建通过；X11 和带 seat 的嵌套 Wayland 各模态 3 passed、窗框 3 passed、菜单 2 passed。直接查询 GTK transient、独立窗口组和 sensitive；X11 查询实际 UTILITY 属性，Wayland 明确验证 Tool／raise 的平台限制。原生组和软件显示会话结果不能替代具体桌面 Shell 的视觉叠放验收。
- Windows：MSVC UCRT、LLVM-MinGW MSVCRT、LLVM-MinGW UCRT 的 Window／window.asyncio／Surface 和完整 GUI 用例均编译通过；各目标模态 2 passed、1 ignored，窗框 2 passed、1 ignored，菜单 1 passed、1 ignored。SSH 只执行纯值测试，不代表真实 HWND 焦点／叠放及交互验收通过；桌面执行入口见[模态测试说明](../../sysroot/src/scoop.window/tests-cones/modal/README.md)。

2026-11-07，D6：已加入[原生文件对话框规范](../specs/SCOOP-WINDOW-DIALOGS-SPEC.md)，实现打开单文件／多文件、保存和选择目录，返回类型化 FileDialogSession<Path>／FileDialogSession<Array<Path>>。Owner 与 Application 输入范围复用窗口模态并集；原生命令延后展示，取消、owner 关闭及应用退出等待实际面板退休。保存接受只返回路径，POSIX 原始字节和 Windows UTF-16 保持无损。AppKit 内层文件循环结束后继续退出已取消的外层窗口模态；Linux 复用现有 GTK/GIO 的本地选择器及 portal，不新增托盘依赖。

- macOS aarch64：格式化、原生严格检查和 Window／window.asyncio 构建通过；文件对话框 2 passed，模态复验 3 passed，原生路径 ABI 1 passed。覆盖真实 NSOpenPanel／NSSavePanel 的 Owner sheet、Application 模态、两独立 owner、嵌套取消、无窗口应用，以及实际 TCP／定时器。当前 loginwindow 下自动组合验证展示与取消，不将其记作前台选择、编辑或覆盖确认验收。
- Linux GNU：原生严格检查、Window／window.asyncio 构建和路径 ABI 1 passed；X11 与带 seat 的嵌套 Wayland 各文件对话框 2 passed、模态复验 3 passed。本地 GtkFileChooser 通过公共选择 API 接受真实单选／多选模式／保存／目录结果，保存后目标文件仍不存在。两后端另各真实 portal 1 passed，观察系统 FileChooser 请求、返回的 request handle 与 Close，覆盖四种入口、面板存活时的 TCP 和无窗口 Application。portal 观察器不替换服务或生成选择结果；跨进程面板实际选择和具体桌面 Shell 呈现留待交互验收。
- Windows：MSVC UCRT、LLVM-MinGW MSVCRT、LLVM-MinGW UCRT 的 Window／window.asyncio 与完整文件对话框、模态程序均编译通过；每目标文件对话框 1 passed、1 ignored，模态 2 passed、1 ignored，原生路径 ABI 1 passed。ABI 测试从 C 返回含非配对 surrogate 的真实 UTF-16 路径，经公共对话框转换后再核对原始字节与字节长度。SSH 不运行 GUI，桌面执行方式见[文件对话框测试说明](../../sysroot/src/scoop.window/tests-cones/dialogs/README.md)。

2026-11-07，D7：已加入 [UI 桌面规范](../specs/SCOOP-UI-DESKTOP-SPEC.md)与 [Surface 区域规范](../specs/SCOOP-SURFACE-FRAME-SPEC.md)。ApplicationComponent／ApplicationElement 在没有业务 Window、Surface 或字体的情况下复用现有 State、Props、key 协调与逻辑更新队列；菜单、popup、模态和文件 session 按实际身份登记事件并沿所属 scope 处理错误。Surface 在 Accepted 后发布窗口形状、输入覆盖及命中区域，保留 clip／变换／比例；隐藏的失败帧、输入边界和卸载撤销均已覆盖。Window.frameRevision 同步标记原生几何失效，防止 paint 内改变窗框时，在异步通知到达前重新发布旧候选。

- macOS aarch64：Scoop 格式化、原生严格检查及相关库构建通过；无窗口应用根 1 passed，桌面区域组合 1 passed，既有 UiCompositionTest 1 passed、InputComparisonTest 3 passed、独立描述复用 1 passed。实际查询 AppKit 命中区域和 CAMetalLayer opaque；帧提交、空区域／默认恢复、应用与窗口错误隔离，以及最后窗口关闭后的应用更新均通过。当前 loginwindow 条件下仍不宣称前台输入与视觉呈现验收通过。
- Linux GNU：原生严格检查及相关库构建通过；X11 与带 seat 的嵌套 Wayland 各应用根 1 passed、桌面区域组合 1 passed、描述复用 1 passed。X11 查询真实 ShapeInput；Wayland 不伪造其无法查询的系统命中区域。尺寸请求使用高于 GTK 最小窗口宽度的值，继续按实际 metrics 和 Accepted 验证。
- Windows：MSVC UCRT、LLVM-MinGW MSVCRT、LLVM-MinGW UCRT 均完成相关库、应用根和完整桌面区域程序的编译；每目标 InputComparisonTest 3 passed，桌面 GUI 组合 1 ignored。应用根程序也需要原生 Application，在 SSH 中只编译。桌面执行说明见[应用根](../../sysroot/src/scoop.ui/tests-cones/application/README.md)与[桌面组合](../../sysroot/src/scoop.ui/tests-cones/desktop/README.md)，未执行的 GUI 交互不记作通过。
- 跨 Cone 描述复用暴露了 HIR 具体化遗漏导入虚方法签名类型的问题。修复限定于实际分派条目的参数、receiver 与返回类型保留，不改变语言／runtime ABI；新增独立的控制台编译运行 fixture。本地相关 Rust 组 93 passed、5 failed；对照提交 08223b1e 的原实现为 92 passed、相同 5 failed，均为原有测试依赖的 MissingCoreFunction，新回归由失败变为通过。正式 CLI fixture 在 macOS、Linux 及 Windows 三目标均为 1 passed；Rust 格式化与对应 crate 的 clippy 通过。

2026-11-07，D8：已实现[原生菜单组件](../specs/SCOOP-UI-NATIVE-MENUS-SPEC.md)、[窗口与文件面板控制器](../specs/SCOOP-UI-NATIVE-WINDOWS-SPEC.md)、[窗框组件](../specs/SCOOP-UI-WINDOW-CHROME-SPEC.md)和独立 [scoop.ui.component.tray](../specs/SCOOP-UI-TRAY-SPEC.md)。菜单按完整 key 路径保留原生身份，Binding 在 render 求值；应用默认菜单与窗口覆盖由 UiHost 协调。WindowChrome 保留标题栏／正文槽位，区域仍在 Accepted 后发布。工具、模态和文件控制器拥有自己的请求及窗口资源，卸载后停止业务回调。

标准编辑角色从当前输入焦点取得可用命令并沿 Surface 路由；原生编辑控件继续使用系统验证，同时遵守显式 enabled。popup 在请求时同步检查原生输入限制，应用退出先停止菜单协调再清理窗口；菜单栏和托盘菜单以真实解绑通知完成退休。依赖闭包检查确认普通 window／ui.component 不包含 scoop.tray，只有显式托盘组件引入它。

- macOS aarch64：格式化、原生严格编译与相关库构建通过；桌面组件组合 2 passed、独立托盘组件 2 passed、输入命令 2 passed；底层菜单复验 2 passed、1 ignored，托盘复验 5 passed、1 ignored。覆盖实际菜单动作与稳定身份、纯菜单更新不绘制、窗框切换、工具窗口复用、Owner／Application 模态及四类文件面板取消、错误隔离、无窗口更新和退出清理。当前 loginwindow 下仍保留原生编辑焦点、真实托盘 popup、拖动／缩放及视觉呈现的前台验收限制。
- Linux GNU：所有相关库及完整 GUI 程序构建通过。X11 和带 seat 的嵌套 Wayland 各桌面组件 2 passed、独立托盘组件 2 passed、输入命令 2 passed；底层菜单各 2 passed、托盘各 5 passed。组合测试等待两个窗口均完成初始配置后再验证菜单动作，避免把尚未可交互窗口捕获到的动作当作有效输入。实际 GTK 菜单、软件显示会话和 SNI 协议结果不代替具体桌面 Shell 的视觉／交互验收。
- Windows：MSVC UCRT、LLVM-MinGW MSVCRT、LLVM-MinGW UCRT 均完成相关库及完整 GUI 程序的构建；各目标桌面组件 1 passed、1 ignored，独立托盘组件 1 passed、1 ignored，输入命令 2 passed，底层菜单 1 passed、1 ignored，托盘 4 passed、1 ignored。SSH 只执行可运行的非 GUI 用例；原生菜单、编辑焦点、窗框、控制器及托盘交互留待登录桌面执行，入口见[桌面组件测试](../../sysroot/src/scoop.ui.component/tests-cones/native-desktop/README.md)与[托盘组件说明](../../sysroot/src/scoop.ui.component.tray/README.md)。

2026-11-07，D9：已加入[原生桌面综合 demo](../../sysroot/src/scoop.ui.component/examples/native-desktop/README.md)。普通 Cone 显式依赖托盘及对应组件，组合应用／窗口菜单、编辑命令、popup、共享菜单状态、三种窗框模式、系统操作、平台外观、异形／鼠标穿透窗口、owned 工具、Owner／Application 及嵌套模态、四种文件面板和有序退出。托盘定位的提示点、完整屏幕替代工作区及无锚点窗口各自显式选择；只有主动选择保留托盘后，关闭全部文档才继续无窗口运行。

- macOS aarch64 与 x86_64：完整构建、`--help` 和 `--smoke` 通过；x86_64 在 Rosetta 下运行。按现有准备脚本更新 x86_64 Skia 和 no_std 文本归档，解决旧 native 资源缺少 PathOps、仍携带 Rust unwind 的链接失败，没有新增 Rust 依赖或修改异常机制。最终程序只含 Scoop、C++、Objective-C personality。首次 x86_64 运行有一次首帧超时，随后干净版完整复验通过；前台拖动、焦点、托盘及冷启动视觉表现仍由真实桌面验收。
- Linux GNU：完整构建，X11 与带 seat 的嵌套 Wayland 各一次完整 `--smoke` 通过；实际 X11 点击、滚动、Owner 确认及椭圆窗口截图检查通过。交互暴露 GTK 滚轮方向与 UI 内容位移约定相反，统一 GTK 两轴及 Win32 水平轴的方向；新增真实 XTest 滚轮断言在旧后端失败、修复后 LinuxApplicationTest 1 passed。不改变滚轮单位、相位或用户自然滚动设置。
- Windows：MSVC UCRT、LLVM-MinGW UCRT、LLVM-MinGW MSVCRT 的最终 demo、更新后的窗口库和原生输入测试程序均编译通过；三个 demo 在开发环境及普通 SSH 环境下的 `--help` 均通过。原生输入程序只编译，GUI／Explorer 场景及 `--smoke` 留给登录桌面验证；没有把无 GUI 启动记作 GUI 通过。
- 示例运行说明包含完整体验顺序及平台依赖；格式化、变更文档的本地链接与差异检查通过。安装包使用已更新资源，发行验收见下。

发行验证：版本为 `0.0.0-native-desktop.20261107`，安装包代码基线为 `4a7cec6e`，与 main 合入提交 `86034415` 的源码一致。各宿主通过正式打包入口生成组件归档，在带空格的独立安装目录解压，并使用随包工具和系统原生开发环境验证；原始日志及归档保存在仓库外。

- macOS ARM64 宿主：六个归档生成成功，包含 ARM64／x86_64 两个目标。两目标的安装后 API 文档、消费程序、TLS／GC 及两个独立链接入口均通过；五个命令的版本、私有 LLVM 22.1.8 和 LLDB／随包 Python 3.12 检查通过，普通 `scoop.idna` Cone 测试 11 passed。两架构均使用安装资源重新构建完整 desktop demo 并通过 `--help`；完整 GUI smoke 结果见 D9，安装验证期间保留原程序供用户手工体验。
- Linux GNU 宿主：六个归档生成成功，包含 GNU／musl 两个目标。两目标的安装后文档、消费程序和 TLS／GC 均通过；GNU 默认动态、musl 静态及动态模式均通过两个链接入口运行，普通 `scoop.idna` Cone 测试 11 passed。五个命令、私有 LLVM 22.1.8、LLDB／随包 Python 3.14 检查通过；安装资源构建的完整 demo 在 X11／Wayland 各一次 `--smoke` 通过。musl 使用已配置的 musl C 工具链及 Linux UAPI 头文件；结束后关闭任务创建的临时显示服务，保留手工程序和日志。
- Windows x86_64 宿主：七个归档生成成功，包含 MSVC UCRT／LLVM-MinGW MSVCRT／LLVM-MinGW UCRT 三个目标。三目标的安装后 API 文档、消费程序与缓存复用、TLS／GC 及两个独立链接入口均通过；五个命令的版本、私有 LLVM 22.1.8、LLDB／随包 Python 3.14 检查通过，普通 `scoop.idna` Cone 测试 11 passed。三目标均使用安装资源重新构建完整 desktop demo 并通过 `--help`；SSH 不执行 GUI，桌面交互仍由用户手工验证。最终归档已复制到本机，旧安装临时文件已清理，手工程序和原始日志保留。

首轮 GNU／Windows 发行资源中混入了只更新桌面依赖闭包后留下的旧基础 `.slib`，安装文档生成在 `scoop.idna` 报告格式 profile 不匹配。已按当前发行工具完整准备目标系统库及 runtime；GNU／Windows 的重新打包与安装验证均已闭合。musl 初次消费验证的 C 编译器误选为读取 glibc 头文件的通用 Clang，改用已有 musl 工具链后完成消费及静态／动态链接验证。
