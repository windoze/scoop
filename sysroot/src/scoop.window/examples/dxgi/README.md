# Win32 / DXGI surface 示例

`renderer` 与 `app` 是两个普通 Cone。renderer 用 C 接入系统 D3D11／DXGI，优先硬件设备，无法创建时使用 WARP。flip-discard swapchain 属于 renderer；窗口库只提供 HWND lease、metrics 和生命周期。

先在开发 sysroot 准备 `scoop.window.slib`，再执行：

```powershell
scoop run app --release --target x86_64-pc-windows-msvc-ucrt
scoop test app --release --target x86_64-pc-windows-msvc-ucrt
```

也支持两个 LLVM-MinGW target，使用各自的 native sysroot。示例程序以 GUI subsystem 链接，输入文字显示在标题栏，Escape 或关闭按钮退出。库负责设置／确认 Per-Monitor V2；应用也可用标准 Windows RC manifest 预先声明相同 DPI 模式。

renderer 的 `present()` 检查窗口状态、可见性和 drawable 大小；按实际像素尺寸重建 back buffer，提交清屏帧并等待 D3D11 event query 完成。`close()` 先排空 GPU 工作、释放视图和 swapchain，再结束窗口 lease。它是展示资源寿命的小型同步 renderer，不是完整帧调度器。

release Cone 测试提交首帧、隐藏时拒绝提交、重新显示并调整 back buffer 后再提交，随后验证 Closing → surface 释放 → Destroyed，以及关闭后的调用和线程约束。关于 flip model 与 resize 的系统合同见 [DXGI swap effect](https://learn.microsoft.com/en-us/windows/win32/api/dxgi/ne-dxgi-dxgi_swap_effect) 和 [ResizeBuffers](https://learn.microsoft.com/en-us/windows/win32/api/dxgi/nf-dxgi-idxgiswapchain-resizebuffers)。
