# Linux 窗口实施

状态：实施中（2026-10-30），分支 `codex/window-linux`。范围为 [DESIGN.md](DESIGN.md) 第 10 节和第 12 节的 Linux 阶段；Windows 另行实施。已有 macOS 基础与验收记录保留在 [PROGRESS.md](PROGRESS.md)。

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

此批尚未以协议事件验证全部 IME、拖放、能力差异和断连分支；musl 依赖与同源验收继续实施。Linux 主机不能执行 AppKit／Metal，Darwin 已同步共享 ABI 与枚举调用方，尚未在本批重跑 macOS。headless 结果不替代真实输入法候选界面、实体混合 DPI 或人工拖动。
