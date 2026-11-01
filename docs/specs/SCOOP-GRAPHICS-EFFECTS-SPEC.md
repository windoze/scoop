# Scoop 图像滤镜与图层规范

日期：2026-11-02。状态：macOS aarch64 已实现并通过真实 Raster／Metal 定向验证，见 [UI 实施记录](../ui/PROGRESS.md)。完成本期功能后统一补验 x86_64。本文扩展 [Surface 规范第 2 节](SCOOP-SURFACE-SPEC.md#2-几何与绘制)的 graphics 合同；UI 应用方式见 [公共样式规范](SCOOP-UI-STYLE-SPEC.md)。Windows 留待后续验证。

## 1. 职责与公开对象

实际 Gaussian blur、内容 alpha 阴影、混合和图层合成属于 `scoop.graphics`，离屏绘图也可直接使用，不依赖 UI、Surface 或窗口。实现复用当前 Skia 的 SkImageFilters 和 saveLayer；基础效果不以 SkSL／SkRuntimeEffect 接入为前置。

`ImageFilter : Closeable` 是 owner 线程上的不可变原生滤镜。首批提供以下工厂与组合：

| 入口 | 对源内容 S 的操作 |
| --- | --- |
| `ImageFilter.blur(sigmaX, sigmaY)` | 对预乘 RGBA 作 Gaussian blur；源外按透明黑采样 |
| `ImageFilter.dropShadow(offset, sigmaX, sigmaY, color)` | 取 S 的 alpha，模糊、着色并平移为阴影，然后把 S 以 SrcOver 合成到阴影上 |
| `ImageFilter.dropShadowOnly(offset, sigmaX, sigmaY, color)` | 仅输出上述阴影，不包含 S |
| `filter.then(next)` | 返回 `next(filter(S))`；列表中的先后顺序就是实际求值顺序 |

sigma 是标准差，sigmaX／sigmaY 为有限非负的局部逻辑单位；offset 为同坐标系的有限 Point。零 sigma 合法，仍可产生平移、着色或恒等结果。颜色沿用 Color 的非预乘 sRGB 输入，模糊与合成使用预乘 alpha；不能把 RGB 与 alpha 重复相乘。非法参数按 graphics 的既有异常规则拒绝。

组合结果独立保留所需 Skia 引用，关闭输入包装不使结果失效。close 幂等；关闭后使用、异线程使用仍遵守 GraphicsResource 的状态与 owner 检查。组合失败不接管或关闭输入。普通 Style 不保存这些原生对象，其使用 Element 的缓存才拥有它们。

## 2. 图层与混合

`Canvas.saveLayer(opacity = 1.0, filter = None, blendMode = BlendMode.SrcOver)` 在当前保存栈上建立透明图层；restore 时先处理完整图层内容，再应用整体 opacity 和指定混合模式合成回父目标。首批 BlendMode 提供 SrcOver、Src、DstIn 和 Plus：分别为通常覆盖、替换目标、以源 alpha 遮罩目标、预乘通道饱和相加。Plus 不代表 HDR。

filter 在调用时借用，图层保留实际需要的原生引用直到恢复／使用结束，因此调用者随后可以关闭该包装。组透明度不能替换成各条绘图分别乘 alpha。默认行为仍是普通 SrcOver 组透明度，既有 save／restore 和 Canvas.withState 的异常恢复合同不变。

图层不以调用方布局矩形作隐式 clip 或滤镜 crop。首批不提供容易被误认为精确输出范围的 bounds 参数；Skia 根据实际目标、当前 clip、滤镜及其采样范围安排图层。离开源内容范围的阴影、光晕和模糊只受真实目标与显式祖先裁剪限制。布局尺寸不是绘制像素的边界，不能据此截断 glyph overhang 或子项效果。

clip 建立在 saveLayer 之前时约束恢复后的输出；滤镜可能为该输出读取 clip 外所需的源样本。若要先裁源内容再产生向外效果，须在滤镜图层内部另建显式 clip。两种顺序不同。几何捕获继续使用绘制时的 transform／clip；滤镜的像素扩展本身不扩大区域几何。

`PathBuilder.addRoundRect(rect, radiusX, radiusY)` 补齐共用的圆角轮廓。半径有限非负，超过尺寸时按 Skia 圆角矩形规则收敛；同一 Path 用于填充、clip、contains 和输入几何，不另用 UI 的圆角近似。build 的快照及所有权合同不变。

## 3. 坐标、生命周期与能耗

Canvas 根部仍只应用一次 DPI。滤镜参数保持局部逻辑单位，由当前 CTM 进入设备空间；调用方和 style 均不得预乘 DPI。嵌套图层、变换与原生 Canvas 借用保持同一坐标及 alpha 合同，不为了滤镜读回 CPU 或逐帧等待 GPU。

滤镜对象没有时钟、后台任务或帧请求。静态对象可长期复用；动画调用方使用已经采样的参数建立／替换实际变化的滤镜，及时关闭旧包装。Skia 保留尚在使用的引用，不能通过积累旧包装来延长 GPU 资源寿命。

新资源必须在构造成功后发布。创建、组合或绘制失败沿现有 graphics／Surface 路径传播，关闭完整清理；withState 在异常退出时恢复保存层级和借用状态。该层不接管 ComponentScope、挂载身份或动画。

## 4. 定向验收与后续

所属 Cone 的 scoop.test 验证真实 RasterSurface 和 Metal 像素：透明内容的 alpha 投影、源与影的合成、滤镜顺序、零 sigma、整体 opacity／混合、边界扩展、clip 顺序、变换与一次 DPI，以及组合后关闭输入、异常层恢复和错误线程。使用带容差的像素／覆盖范围断言，不要求两个后端逐位一致，也不只检查编译成功。

Backdrop blur 采样此前已绘制的背后内容，具有不同输入、可见性及缓存合同，本批不实现。SkSL、任意 RuntimeShader、自定义 Blender、高级阴影形态和外部 GPU 图像也不作为上述基础效果的前置；按实际后续需求独立扩展。
