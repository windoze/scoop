# scoop.graphics

普通 Scoop Cone，提供与窗口无关的 Skia 2D 绘图。共享接口使用左上原点的 Double 逻辑坐标；目标资源覆盖 macOS ARM64／x86_64 和 Windows x64 的 Microsoft UCRT、LLVM-MinGW UCRT／MSVCRT。CPU 离屏使用不要求启动 Application。

```toml
[dependencies]
"scoop:scoop.graphics" = "0.1.0"
```

```scoop
import scoop.graphics.*

RasterSurface(320, 180).use { surface ->
    PathBuilder().use { builder ->
        builder.moveTo(Point(20.0, 120.0))
            .quadraticTo(Point(160.0, 0.0), Point(300.0, 120.0))
        builder.build().use { path ->
            surface.draw { canvas ->
                canvas.clear(Color.White)
                canvas.drawPath(path, Paint(Color(0.1, 0.4, 0.8),
                    PaintingStyle.Stroke, 4.0, StrokeCap.Round))
            }
        }
    }
    val rgba = surface.readPixels()
}
```

Canvas 支持 save／restore、仿射变换、矩形／Path 裁剪、基本形状、描边、线性／径向渐变、Image 及组透明度。`saveLayer(opacity)` 对整个组的合成结果应用透明度，`restore()` 结束该组。PathBuilder 的 `build()` 返回不可变快照；Path 的 contains、bounds 和 PathMeasure 使用相同几何与填充规则。

SkSL runtime effect、uniform／输入 shader、Paint 着色及子树图层效果列为[后续增强设计](../../../docs/graphics/sksl/DESIGN.md)，目前未提供 Scoop 接口。计划复用 Skia 现有后端，程序与参数分开管理，动画显式驱动 uniform；持有效果不自动启动帧循环。

Path、PathBuilder、PathMeasure、Image、RasterSurface 与底层目标均显式 close／use。原生资源限创建线程使用，Canvas 只在一次 draw 回调内有效，异常退出也会结束借用并恢复状态。Geometry 快照保留原生 Path 数据，可以在原 Path 或绘制作用域结束后继续使用；关闭快照后不可再查询。

颜色采用 sRGB，绘制内部为预乘 alpha；RGBA8 输入和 `readPixels()` 返回值采用紧密排列的直通 alpha 字节。PNG／JPEG decode 会复制输入并实际解码；surface snapshot 保留独立图像内容，后续绘制不会修改旧快照。

GUI renderer 通过 `scoop.graphics.native` 的 RenderTarget 和 geometry 捕获接入。macOS 的 Unsafe `scoop.graphics.native.macos.MetalContext` 接受所属 device／queue 和 BGRA8 texture；Windows 的 Unsafe `scoop.graphics.native.windows.D3DContext` 接受 adapter／device／direct queue，以及同设备的单采样 BGRA8 render target。D3D12 resource 在包装前须处于 PRESENT／COMMON 状态，submit 和读回结束后恢复 PRESENT。绑定保留所需 COM 引用，原生对象仍由调用方按 GPU 使用顺序管理。

DPI 缩放由目标根部施加一次，geometry 快照仍使用逻辑坐标。普通 submit 不等待 GPU；显式读回、drain 和 context 关闭是同步点。字体、样式、Paragraph／TextLayout 和 Canvas.drawText 由独立 [scoop.graphics.text](../scoop.graphics.text/README.md) 提供；使用文本时声明该 Cone 依赖并导入其 package。

文本等原生绘图适配通过 `scoop.graphics.native` 的 `@Unsafe Canvas.withSkiaCanvas` 同步借用实际 SkCanvas。借用保留现有坐标／裁剪，正常返回和异常退出都恢复原生状态，不增加 DPI 缩放。指针仅限回调内使用；回调不能重入同一 Canvas 的 Scoop 操作、越过借用的保存边界、提交目标或变更 context。详细合同见 [Surface 规范](../../../docs/specs/SCOOP-SURFACE-SPEC.md#2-几何与绘制)。

准备 [native 资源](../../../scripts/README.md)后运行 `scoop test sysroot/src/scoop.graphics --release --sysroot sysroot`。测试通过真实 Skia 像素、几何和跨 Cone 借用验证行为；文本与字体测试归 graphics.text。整体进展见 [Surface 实施记录](../../../docs/surface/PROGRESS.md)及 [UI 实施记录](../../../docs/ui/PROGRESS.md)。
