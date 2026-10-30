# 桌面窗口与 asyncio 定时器

`timer.scoop` 是普通单文件 Cone，消费预编译的 window／asyncio／window.asyncio。它在主线程运行真实 AppKit 或 Win32 窗口，通过当前应用 scope 启动协程，每秒更新一次窗口标题。定时来自 asyncio 的实际后端，窗口主循环保持 Wait。

准备 Darwin sysroot 后，在仓库根运行：

```sh
export SCOOP_SYSROOT="$PWD/sysroot"
target/debug/scoop run sysroot/src/scoop.window.asyncio/examples/timer.scoop --release
```

Windows 使用准备好的对应目标 sysroot，在已登录桌面运行；MinGW 目标还需指定对应的 `--native-sysroot`：

```powershell
scoop run sysroot/src/scoop.window.asyncio/examples/timer.scoop --release --target x86_64-pc-windows-msvc-ucrt
```

关闭窗口会请求应用退出，取消定时任务，等待 I/O session 收尾后返回。窗口／任务／native watch 的实际退出顺序与模态、菜单 tracking 组合由所属库的 `scoop.test` 覆盖：

```sh
target/debug/scoop test sysroot/src/scoop.window.asyncio --release
```

入口必须在主线程、活动 Job 之外调用；不要在窗口事件内再次调用 `PlatformEventLoop.run`。涉及真实 surface 的显式 lease 顺序见 [Metal 示例](../../scoop.window/examples/metal/README.md)与 Windows [DXGI 示例](../../scoop.window/examples/dxgi/README.md)。
