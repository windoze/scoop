# 平台实现参考

窗口职责、键盘模型和 AppKit 事件规则参考 [winit 0.30.13](https://github.com/rust-windowing/winit/tree/v0.30.13)，由 winit contributors 维护，采用 [Apache License 2.0](LICENSE-winit)。相关来源是 macOS 后端的 [view.rs](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/view.rs)、[event.rs](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/event.rs) 和 [monitor.rs](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/monitor.rs)。

本库将这些平台语义应用于 Scoop 自有的事件载荷、文本批次和显式 native lease；不链接 winit，也不复制 Rust 对象或 ABI。Darwin 物理键码按 Apple SDK HIToolbox/Events.h 的实际虚拟键常量核对；不增加 Carbon 运行库依赖。

AppKit 无装饰窗口的 key／main 能力和 style mask 变更后的 first responder 恢复另参考 [window.rs](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/window.rs) 与 [window_delegate.rs](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/window_delegate.rs)。

Windows set-1 扫描码映射参考同版本的 [keyboard.rs](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/windows/keyboard.rs)，将 E0 扩展位编码为 bit 8；未命名的键保留原始 scan code。Win32 原生接口遵循 Microsoft 的 [Virtual-Key Codes](https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes)、[WM_IME_COMPOSITION](https://learn.microsoft.com/en-us/windows/win32/intl/wm-ime-composition)、[进程 DPI awareness](https://learn.microsoft.com/en-us/windows/win32/hidpi/setting-the-default-dpi-awareness-for-a-process) 和 [WaitForThreadpoolWaitCallbacks](https://learn.microsoft.com/en-us/windows/win32/api/threadpoolapiset/nf-threadpoolapiset-waitforthreadpoolwaitcallbacks) 文档。
