# 平台实现参考

窗口职责、键盘模型和 AppKit 事件规则参考 [winit 0.30.13](https://github.com/rust-windowing/winit/tree/v0.30.13)，由 winit contributors 维护，采用 [Apache License 2.0](LICENSE-winit)。相关来源是 macOS 后端的 [view.rs](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/view.rs)、[event.rs](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/event.rs) 和 [monitor.rs](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/monitor.rs)。

本库将这些平台语义应用于 Scoop 自有的事件载荷、文本批次和显式 native lease；不链接 winit，也不复制 Rust 对象或 ABI。Darwin 物理键码按 Apple SDK HIToolbox/Events.h 的实际虚拟键常量核对；不增加 Carbon 运行库依赖。
