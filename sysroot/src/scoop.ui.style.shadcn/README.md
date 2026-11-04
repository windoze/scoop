# shadcn 主题

`shadcnTheme(ThemeMode.Light／Dark) { ... }` 返回完整 ShapeTheme，通过同一 ThemeSource 接入现有控件。颜色采用 [官方 Neutral token](https://ui.shadcn.com/docs/theming)，部件采用 [Base Nova registry](https://ui.shadcn.com/r/styles/base-nova/button.json)（2026-11-04）；源 OKLCH 与转换后的 sRGB 值保存在 `src/palette.scoop`。

一逻辑单位对应一 CSS px；基准圆角为 10，默认操作高度 32，文字 14／20。主题只提供样式与几何，控件状态和输入仍由 `scoop.ui.component` 维护。显式减少动态：`tokens { motion { preference = MotionPreference.Reduced } }`。

配色／样式派生自 shadcn/ui，采用随附 MIT 许可证。当前不包含第三方图标路径或字体文件。
