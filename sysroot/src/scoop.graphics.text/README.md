# scoop.graphics.text

独立的字体与段落排版 Cone，依赖 graphics 的几何、颜色与画布，不需要 window、surface 或 UI。字体来自显式数据和系统 fallback；shaping／排版复用 SkParagraph、HarfBuzz 与 SkUnicode／ICU。

```toml
[dependencies]
"scoop:scoop.graphics" = "0.1.0"
"scoop:scoop.graphics.text" = "0.1.0"
```

```scoop
import scoop.graphics.*
import scoop.graphics.text.*

FontCollection().use { fonts ->
    Paragraph(fonts, "Hello，是世界。مرحبا", TextStyle(fontSize = 20.0)).use { paragraph ->
        paragraph.layout(280.0).use { layout ->
            RasterSurface(320, 180).use { surface ->
                surface.draw { canvas ->
                    canvas.clear(Color.White)
                    canvas.drawText(layout, Point(12.0, 12.0))
                }
                val position = layout.hitTest(Point(30.0, 10.0))
                val caret = layout.caret(position)
                val selection = layout.selection(TextRange(0L, paragraph.text.byteLength))
            }
        }
    }
}
```

FontCollection 默认使用系统 fallback，也可以通过 `FontCollection(false)` 关闭并用 `register(bytes, family)` 登记自有字体。只读 `revision` 从 0 开始，每次成功登记增加一次，失败不改变版本。Paragraph 在构造时保存文字、样式和字体来源快照；`layout(width)` 与 `layoutUnwrapped()` 每次产生独立结果，后续登记字体、关闭来源或重新布局不改变旧结果。

当前内容输入为 String／TextStyle 或 `Array<TextRun>`。TextStyle 支持 family 顺序、字号、颜色、weight／width／slant、locale、字距、词距、行高及 OpenType feature／variation；ParagraphStyle 指定方向、对齐、maxLines 和 ellipsis。下一批按[文本排版设计](../../../docs/graphics/text/DESIGN.md)加入 StyledText／SpanStyle 和行内对象，届时由统一内容描述替换 run 作者接口；这些新类型目前尚未提供。

TextRange 和 TextPosition 使用 **UTF-8 字节偏移**，与 String 的标量索引不同。selection 的端点必须是标量边界，caret 的端点必须是 `graphemeBoundaries` 中的位置。hitTest 返回可见内容的 grapheme 边界；affinity 区分软换行和双向 run 的两侧。被截断的位置没有 caret；selection 可以返回多个带方向的矩形。各行保存 range、contentRange、visibleRange、baseline 与尺寸，查询结果相对布局原点。

字体、Paragraph 和 TextLayout 限创建线程使用及关闭，通过 Closeable／use 管理。文本库拥有自己的 native 资源；Canvas.drawText 只在同步回调内借用 graphics 的公开原生画布接口，正常返回或异常退出后恢复调用方状态，不依赖 graphics 的私有包装或 internal 成员。

准备[目标资源](../../../scripts/README.md)后运行 `scoop test sysroot/src/scoop.graphics.text --release --sysroot sysroot`。测试覆盖真实离屏文本、固定字体、Unicode／bidi、独立布局和资源边界；macOS 另验证真实 Metal 目标。字体来源与许可见 [FONTS.md](tests/data/FONTS.md)，运行结果和平台范围见 [UI 实施记录](../../../docs/ui/PROGRESS.md)。
