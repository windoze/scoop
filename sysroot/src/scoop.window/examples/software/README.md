# Linux 软件绘制示例

`buffers` 是普通 renderer Cone，用 `wl_shm`／`XPutImage` 提交真实像素；`app` 是窗口应用 Cone。应用持有 `NativeWindowLease`，只按 `GeometryChanged` 发布的像素尺寸创建 buffer，在实际提交前调用 `prePresentNotify`。Closing 时先关闭 renderer，再释放 lease。此示例不要求 GPU。

准备 Linux 开发 sysroot 和桌面依赖后，在仓库根运行：

```sh
export SCOOP_SYSROOT="$PWD/sysroot"
SCOOP_WINDOW_BACKEND=wayland target/release/scoop run --release sysroot/src/scoop.window/examples/software/app
SCOOP_WINDOW_BACKEND=x11 target/release/scoop run --release sysroot/src/scoop.window/examples/software/app
```

默认使用自动选择。窗口显示棋盘像素；键盘输入改变标题，Escape 退出，本地文件拖放显示路径。IME 取决于 compositor 的 text-input-v3 或 XIM，缺少输入法协议时普通 compose 仍可用。

`scoop test --release sysroot/src/scoop.window/examples/software/app` 用同一应用提交五帧后关闭，验证普通预编译 window consumer 与 renderer 的 lease 生命周期。GNU 使用默认动态链接；musl 需显式 `--link-mode dynamic` 及匹配的 musl 桌面库。
