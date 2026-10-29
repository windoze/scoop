# 平台实现参考

窗口职责、键盘模型和 AppKit 事件规则参考 [winit 0.30.13](https://github.com/rust-windowing/winit/tree/v0.30.13)，由 winit contributors 维护，采用 [Apache License 2.0](LICENSE-winit)。相关来源是 macOS 后端的 [view.rs](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/view.rs)、[event.rs](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/event.rs) 和 [monitor.rs](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/monitor.rs)。

本库将这些平台语义应用于 Scoop 自有的事件载荷、文本批次和显式 native lease；不链接 winit，也不复制 Rust 对象或 ABI。Darwin 物理键码按 Apple SDK HIToolbox/Events.h 的实际虚拟键常量核对；不增加 Carbon 运行库依赖。

AppKit 无装饰窗口的 key／main 能力和 style mask 变更后的 first responder 恢复另参考 [window.rs](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/window.rs) 与 [window_delegate.rs](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/window_delegate.rs)。

Linux 协议输入固定为 [wayland-protocols 1.47](../../vendor/wayland-protocols/README.md)，由 wayland-scanner 1.24.0 生成普通 C 输入；XML 和生成文件保留上游许可。libwayland 1.24.0、libxkbcommon 1.13.1、libdecor 0.2.5 及其 C cairo 装饰插件通过系统动态库使用，采用各自上游 MIT 许可；不将 GTK 插件或 glibc 库装入 musl 程序。

Xlib／XKB、XInput2、XRandR、Xcursor 来自维护中的 X.Org 项目，沿其系统库 MIT/X11 许可动态链接。X11 基线版本分别为 libX11 1.8.13、libXi 1.8.2、libXrandr 1.5.4、libXcursor 1.2.3。键位采用 Linux input-event-codes 和 XKB key name／keysym 的公开编码，不自行实现键盘布局或 Wayland wire protocol。

Linux `test-native` Cone 另动态链接 MIT 许可的 libwayland-server、MIT/X11 许可的 libXtst／libxcb，以及 fcitx 维护的 [xcb-imdkit 1.0.9](https://github.com/fcitx/xcb-imdkit/tree/1.0.9)（[LGPL-2.1-only](https://github.com/fcitx/xcb-imdkit/blob/1.0.9/LICENSES/LGPL-2.1-only.txt)）。后者提供实际 XIM 服务端和 COMPOUND_TEXT 编码，不复制其源码。这些库不进入生产窗口 Cone。测试 server header 与 client header 由同一份固定 Wayland XML 生成，保留上游许可。
