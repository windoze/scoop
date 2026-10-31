# scoop.graphics

普通 Scoop Cone，提供与窗口无关的 Skia 2D 绘图。共享接口使用左上原点的 Double 逻辑坐标；当前发行资源覆盖 macOS ARM64 与 x86_64，CPU 离屏使用不要求启动 Application。

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

Path、PathBuilder、PathMeasure、Image、RasterSurface 与底层目标均显式 close／use。原生资源限创建线程使用，Canvas 只在一次 draw 回调内有效，异常退出也会结束借用并恢复状态。Geometry 快照保留原生 Path 数据，可以在原 Path 或绘制作用域结束后继续使用；关闭快照后不可再查询。

颜色采用 sRGB，绘制内部为预乘 alpha；RGBA8 输入和 `readPixels()` 返回值采用紧密排列的直通 alpha 字节。PNG／JPEG decode 会复制输入并实际解码；surface snapshot 保留独立图像内容，后续绘制不会修改旧快照。

GUI renderer 通过 `scoop.graphics.native` 的 RenderTarget 和 geometry 捕获接入；macOS 的 Unsafe `scoop.graphics.native.macos.MetalContext` 只接受所属 device／queue 和 BGRA8 texture。DPI 缩放由目标根部施加一次，geometry 快照仍使用逻辑坐标。普通 submit 不等待 GPU；显式读回和 drain 是同步点。

文本使用 `FontCollection`、`TextStyle`、`Paragraph` 和 `TextLayout`。FontCollection 默认启用系统 fallback，也可以关闭后使用 `register(bytes, family)` 登记自有字体。Paragraph 在构造时保存文字、run 与字体来源快照，`layout(width)` 和 `layoutUnwrapped()` 每次返回独立结果；旧结果不受重新布局、字体登记或来源关闭影响。

```scoop
FontCollection().use { fonts ->
    Paragraph(fonts, "Hello，是世界。مرحبا", TextStyle(fontSize = 20.0)).use { paragraph ->
        paragraph.layout(280.0).use { layout ->
            RasterSurface(320, 180).use { surface ->
                surface.draw { canvas -> canvas.drawText(layout, Point(12.0, 12.0)) }
                val position = layout.hitTest(Point(30.0, 10.0))
                val caret = layout.caret(position)
                val selection = layout.selection(TextRange(0L, paragraph.text.byteLength))
            }
        }
    }
}
```

TextRange 和 TextPosition 使用 **UTF-8 字节偏移**，与 String 的标量索引不同。selection 的端点须是标量边界；caret 的端点须是 `graphemeBoundaries` 中的位置。hitTest 返回 grapheme 边界，affinity 区分软换行和双向 run 的两侧；未显示的截断文字没有 caret。selection 可返回多个带方向的矩形，各行另有 range、contentRange、visibleRange、baseline 与尺寸。查询几何相对布局原点，调用方自行应用绘制位置和 Canvas 变换。

TextStyle 支持基本富文本、family 顺序、字号／颜色、字体样式、locale、字距／词距、行高及 OpenType feature／variation；ParagraphStyle 提供方向、对齐、maxLines 和 ellipsis。HarfBuzz shaping、ICU 分段／bidi 与 SkParagraph 排行产生同一份绘制和查询结果。FontCollection、Paragraph 和 TextLayout 均遵守创建线程与显式 close／use 规则。

准备 [native 资源](../../../scripts/README.md)后运行 `scoop test sysroot/src/scoop.graphics --release --sysroot sysroot`。测试通过真实 Skia 像素与几何查询验证行为，不使用 fixture；固定测试字体及许可见 `tests/data/FONTS.md`。整体进展见 [GUI 实施记录](../../../docs/gui/PROGRESS.md)。
