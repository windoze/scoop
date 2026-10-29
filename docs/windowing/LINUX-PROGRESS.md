# Linux 窗口实施

状态：Linux 骨架实现与 GNU／musl 自动化验收完成（2026-10-30），分支 `codex/window-linux`。范围为 [DESIGN.md](DESIGN.md) 第 10 节和第 12 节的 Linux 阶段；Windows 另行实施。已有 macOS 基础与验收记录保留在 [PROGRESS.md](PROGRESS.md)。

交付顺序：

1. 固定协议输入和 native 依赖，补齐共享层的异步配置、可选快照及真实请求结果。
2. Wayland／X11 显示连接、主循环、窗口生命周期、属性、DPI／显示器及 native lease。
3. 键盘／compose、指针／滚轮／光标、IME 批次、文件拖放和真实 buffer 提交。
4. `scoop.window.asyncio` 宿主接入、取消／关闭组合、GNU／musl 的同一套 Cone 测试及文档。

功能、native 适配、示例和普通行为测试均属于 Scoop Cone，断言使用 `scoop.test`。必要的底层编译／链接缺口先独立修复；不在窗口库绕过。每批完成后格式化、检查、验证、提交和推送，再检查并合并远端 main。

自动化宿主采用真实的 Weston headless compositor 与 Xvfb／Openbox；协议事件测试与实际窗口、buffer 和 I/O 测试分别说明覆盖范围。混合实体显示器、真实输入法候选交互及人工拖动的验证不由 headless 结果代替。原始日志、临时显示宿主及运行产物放在仓库外。

## 当前证据

- 起点为 main 的 `2ae2534a1`（macOS 窗口合入）；新分支已建立，工作树无继承修改。
- 本机为 Linux x86_64，具有 LLVM 22.1、GNU／musl 工具链与现成系统库产物。配套 release 命令与 GNU 开发 sysroot 已完成准备。
- Linux 桌面开发依赖、Weston、Xvfb 和 Openbox 已安装；真实窗口验证见批次 2。

## 已完成批次

### 1. Linux 合同与协议输入

明确后端选择、异步 configure、可选快照、真实请求结果、Wayland 等待与重显、IME／拖放和 GNU／musl 动态依赖合同。固定 7 份 wayland-protocols 1.47 XML 及其许可，用 wayland-scanner 1.24.0 生成 14 个普通 native 输入；消费 Cone 不运行生成器。

验证：生成脚本通过 Ruff 0.16.10 format／check；再次生成的全部文件逐字节相同；7 份生成 C 源码通过 C11、Wall／Wextra／Werror 编译检查。此批验证仅覆盖协议输入，窗口功能仍在实施。

### 2. 双后端、共享生命周期与集成

两个普通 Cone 已选入 Linux 生产清单。共享层支持实际后端、typed native handle、PendingConfigure、配置代次、可选状态和真实 RequestStatus；Wayland 使用 xdg-shell／libdecor、frame callback、viewport／scale、xkbcommon、text-input-v3 和 data offer；X11 使用 Xlib／XKB／XI2／XIM／XRandR／XDND。文本与 URI 在 native 边界复制后交给 Scoop，编辑器值规则保留在 Scoop 层。软件 buffer 与应用分别是 renderer／consumer Cone。

验证环境为 Ubuntu 26.04.1、LLVM 22.1.2；显式准备 GNU 开发 sysroot。全部手写及生成 Linux C 通过 C11、Wall／Wextra／Werror 检查，Scoop 源码经格式化与 production／test check，成功生成预编译 `.slib`。真实桌面测试如下：

| 环境 | window | window.asyncio | 软件绘制 consumer |
| --- | --- | --- | --- |
| Xvfb＋Openbox，显式 X11 | 16／16 | 2／2 | 1／1 |
| Weston headless＋libdecor，显式 Wayland | 16／16 | 2／2 | 1／1 |

window 验证包含文本值规则、URI 原始路径字节、选项快照、实际窗口／buffer、隐藏重显、外线程投递、lease 关闭和 deadline。Wayland 窗口重显在收到新配置后提交真实 wl_shm buffer；软件 consumer 使用预编译 window，两个后端均完成五帧和 renderer→lease→window 关闭。

asyncio 组合测试发现并先修复了 LoopMailbox 的关闭竞争：根先取消并停止业务投递后，主动 requestClose 仍须记录关闭意图。独立宿主 6／6 通过，随后两套窗口集成各 2／2 通过；修复单独提交 `6f15d0c52`。多 Application 用例使用 scoop.test 现有过滤入口在独立进程主线程执行。

此批的协议边界与 musl 验收由批次 3 补齐。Linux 主机不能执行 AppKit／Metal，Darwin 已同步共享 ABI 与枚举调用方，未在本次 Linux 工作中重跑 macOS。

### 3. 协议边界、断连与 GNU／musl 验收

在现有 `test-native` Cone 中加入 libwayland-server 对端、XTest／XDND 来源和 xcb-imdkit 1.0.9 XIM server。生产后端始终使用真实 libwayland-client／Xlib 连接；测试断言位于 Scoop，协议 server header 来自同一组固定 XML。测试依赖及许可见[窗口 README](../../sysroot/src/scoop.window/README.md)和 [THIRD-PARTY.md](../../sysroot/src/scoop.window/THIRD-PARTY.md)。

新增 13 项协议测试覆盖：

- Wayland 零尺寸 configure、可选协议缺失、首次及变化后的 wm_capabilities、合法消息长度、fractional scale、旋转和输出移除／重连；快照拥有数据，重连不复用 MonitorId。
- 实际键位、compose、Ctrl 快捷键、左右修饰键、重复 deadline、指针／滚轮及失焦 synthetic release。
- text-input-v3 的 revision／serial、delete→commit→preedit 次序、逆向选区、非法 UTF-8 删除、过期批次及重新发布；XIM Unicode 局部修改、一次 commit 和非法 preedit 范围撤销。
- Wayland URI transfer、XDND 普通／INCR selection、原始路径字节；发送缓冲区背压期间 deadline 推进，恢复读取后确认全部排队请求送达。
- Wayland／X11 断连、持有 lease 时收尾，以及 X11 断连后写入返回 Disconnected。

这些测试同时修复了实际边界问题：surrounding 与候选矩形合并为一次 commit；Ctrl 的逻辑键保留布局字符；Wayland 标题／app id 在发送前遵守单条消息长度，初始状态和后续能力变化分别处理；显示器 mode 按 transform 归一。X11 阻塞前保留 Xlib 内部队列的就绪事实，局部处理 libxcb 写入产生的 SIGPIPE，并恢复调用线程的信号 mask。Wayland 生命周期与属性操作拆为两个模块。

协议对端的真实链接还暴露了底层 ELF 缺口：libwayland-client 与 libwayland-server 都合法导出 wl_array 等符号，原 linker 将两个共享库误判成重复静态强定义。已先独立提交 `f1a71b4ed`，按最终 ELF 链接顺序解析重叠共享导出，继续拒绝普通对象／archive 冲突和显式库绑定不一致。workspace Rust fmt／clippy、23 项 linker 测试通过，其中包含真实共享库及可执行程序。

最终 release 验收使用同一组源码、显式发布的预编译系统库和共享 Cone cache：

| 目标与真实桌面 | window | window.asyncio | 软件绘制 consumer |
| --- | --- | --- | --- |
| GNU，Weston headless＋cairo 装饰 | 29／29 | 2／2 | 1／1 |
| GNU，Xvfb＋Openbox | 29／29 | 2／2 | 1／1 |
| musl，Weston headless＋cairo 装饰 | 29／29 | 2／2 | 1／1 |
| musl，Xvfb＋Openbox | 29／29 | 2／2 | 1／1 |

每次 window 测试包含两种后端各自的协议用例，环境选择决定真实窗口／renderer 用例的后端。额外定向复核中，GNU XDND 连续 20 次通过；修复后的 musl 异步断连及断连后写入各连续 40 次通过。全部手写 Linux C 在 GNU／musl 头文件下通过 C11、Wall／Wextra／Werror 检查；变更的 Scoop 源码完成 fmt／check，协议生成脚本通过 Ruff 0.16.10。未将普通功能验收切换为 GC stress。

环境为 Ubuntu 26.04.1、Linux 7.0.0-42、LLVM 22.1.2。GNU 使用 glibc 2.43、libwayland 1.24.0；musl 使用 Alpine 3.24.3 的独立开发 root，包含 musl 1.2.6、libwayland 1.25.0。两套均使用 libxkbcommon 1.13.1、libdecor 0.2.5 及 C cairo 插件。musl 通过 GCC specs 选择该 root 的 libc headers、启动对象和 loader，以动态模式链接同目标桌面库；实际 loader 列表确认依赖来自该 root。27 个 musl 生产 Cone 及四种 runtime 已显式准备，window／window.asyncio 更新后重新发布，再运行消费者。

### 使用与验证边界

可重复命令、测试依赖和目标工具参数见[窗口 README](../../sysroot/src/scoop.window/README.md#linux-使用与验证)；示例入口见[软件绘制 README](../../sysroot/src/scoop.window/examples/software/README.md)。自动化使用独立的 Weston headless／pixman socket 与 Xvfb／Openbox DISPLAY，未向用户桌面注入输入。原始日志、SDK、测试宿主和运行产物均保留在仓库外。

协议测试已验证输出旋转、缩放变化、移除／重连、IME 和拖放的数据合同。实体混合 DPI、真实输入法候选界面及人工拖动仍需实际桌面／硬件验证；headless 结果不代替这些交互。Windows、全屏控制、raw device、触摸／高级手势和光标捕获保持后续范围；本次不声明已重跑 Darwin AppKit／Metal。
