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
- 本机为 Linux x86_64，具有 LLVM 22.1、GNU／musl 工具链与现成系统库产物。正在重建配套命令并显式准备开发 sysroot。
- Linux 桌面开发依赖、Weston、Xvfb 和 Openbox 已安装；尚未据此宣称窗口测试通过。

## 已完成批次

### 1. Linux 合同与协议输入

明确后端选择、异步 configure、可选快照、真实请求结果、Wayland 等待与重显、IME／拖放和 GNU／musl 动态依赖合同。固定 7 份 wayland-protocols 1.47 XML 及其许可，用 wayland-scanner 1.24.0 生成 14 个普通 native 输入；消费 Cone 不运行生成器。

验证：生成脚本通过 Ruff 0.16.10 format／check；再次生成的全部文件逐字节相同；7 份生成 C 源码通过 C11、Wall／Wextra／Werror 编译检查。此批验证仅覆盖协议输入，窗口功能仍在实施。
