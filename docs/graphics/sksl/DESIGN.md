# scoop.graphics：SkSL 与组件效果的后续设计

日期：2026-11-01。状态：调研与后续设计，尚未实现 Scoop 接口或运行效果原型；下方 SkSL 仅为用法示例。本设计不增加首期 UI 的交付前置。现有绘图合同见 [Surface 规范](../../specs/SCOOP-SURFACE-SPEC.md)，组件接入见 [UI 设计第 14 节](../../ui/DESIGN.md#14-后续增强sksl-与组件效果)，帧需求遵守 [UI 静止合同](../../specs/SCOOP-UI-SPEC.md#61-静止时停止渲染工作)。

## 1. 当前基础与职责

固定 Skia revision `f326a1e25fc736704063922b9ce76b527b1c5efe` 提供 SkRuntimeEffect，可将 SkSL 编译为 SkShader、SkColorFilter 或 SkBlender；SkImageFilters::RuntimeShader 另提供图像滤镜接入。已核对公开源码，具体后端行为仍需运行验证。当前 Scoop 的 [Paint](../../../sysroot/src/scoop.graphics/src/drawing/paint.scoop) 仅开放颜色、渐变和描边等参数，[Canvas](../../../sysroot/src/scoop.graphics/src/drawing/canvas.scoop) 的 saveLayer 仅接受透明度，没有 runtime effect 或子树滤镜入口。

计划在 scoop.graphics 提供 effect、uniform／输入 shader 绑定和 Paint 接入。SkSL 源码可由普通字符串或资源提供，通过 graphics 的 native 薄层交给 Skia 编译；编译诊断、参数检查和资源包装由该库处理。Skia C++ ABI 保留在绑定内部，不增加 Scoop 语法、编译器特判、runtime 通道或 UI 内置 Shader 节点，公开类型和签名在实施时确定。

现有[构建配置](../../../scripts/build_skia.py)启用 Ganesh Metal，Windows 切换为 D3D12；SkSL 接入可从这些后端开展，不以前置实现 GL／GLES／Vulkan 为条件。这里使用的是 Skia 绘制流程中的 runtime effect，具体 3D 场景仍由[独立渲染接入](../../ui/DESIGN.md#133-在自绘组件内显示-3d-场景)负责。

## 2. 三种使用方式

| 使用方式 | 典型场景 | 接入要求 |
| --- | --- | --- |
| shader 填充形状 | 扫光、动态渐变、波纹、程序化背景 | 在 Paint 中使用着色来源，经普通 Rect／Path 绘制和裁剪 |
| shader 处理图像 | 图片转场、颜色处理、图像局部扭曲 | 将 Image 对应的 shader 绑定为输入；SkSL 通过输入的 eval 取得颜色 |
| 组件子树整体效果 | 卡片溶解、页面波动、组合内容滤镜 | 取得子树的图层结果，经过图像滤镜后合成，需增加相应图层接入 |

输入 shader 可以来自图片、渐变或另一个 runtime effect，组合不必一律先绘成纹理。给背景 Paint 设置 shader 只改变该次绘图的着色，不会自动处理后面绘制的文字或子组件。先完成形状填充和已有 Image 输入，再按实际场景接入整体效果；ColorFilter／Blender 也按实际颜色处理或混合需求推进。

视频帧和 3D 颜色纹理以后可以作为效果输入，但仍依赖[外部 GPU 图像接入](../../ui/DESIGN.md#134-gpu-接入的共同边界)。普通程序化填充和已有 Image 的效果可以独立交付，不等待视频播放或离屏 UI 宿主。

## 3. 组件、uniform 与资源

编译后的 effect 与每次使用的参数分开。相同程序可以在同一 owner 上复用，各组件维护自己的 uniform 与输入绑定；多组件共用程序不能共用一份不断被覆盖的参数。提交给一次绘制的数据保存当次快照，不因后续修改 builder 或动画值而改变已记录的绘图。

编译和 uniform／输入名称解析在加载、挂载或源码确实变化时完成，不在每次 render 或 paint 中重复编译。绑定根据 Skia 反射检查名称、类型、数组长度及所需输入，复用解析结果，调用方不手工计算字节偏移。SkSL 编译失败保留诊断；源码未变时只更新参数。Skia 仍可能在首次实际绘制时创建 GPU 管线，不能把源码编译完成等同于所有后端准备已结束。

组件自有的 effect 与缓存通过 ComponentScope 登记；应用共享的程序由其长期所有者管理，单个组件卸载只结束自己的绑定和借用。绘制调用产生的短命原生绑定由 graphics 接入处理，所需 GPU 引用保持到实际使用结束；可复用缓存替换旧值时及时释放，不逐帧向 scope 追加旧 shader。Canvas 仍只在当前 paint 回调内借用，编译结果或快照也不自动获得跨线程使用许可。

基础框架只提供 Element、Props、ComponentScope 和 Surface 动画接入。扫光按钮、波纹面板等成品效果可以由控件库或第三方库实现，使用相同公开能力。

## 4. 动画示例与停帧

下面的 SkSL 在元素上绘制一道从左向右移动的光带。元素宽度为零时跳过效果绘制；size 使用元素逻辑尺寸，progress 取 0 到 1，tint 使用普通颜色输入。

```glsl
uniform float2 size;
uniform float progress;
layout(color) uniform half4 tint;

half4 main(float2 p) {
    float x = p.x / size.x;
    float center = -0.25 + 1.5 * progress;
    half strength = half(max(0.0, 1.0 - abs(x - center) / 0.2));

    half alpha = tint.a * strength;
    return half4(tint.rgb * alpha, alpha);
}
```

组件的使用流程为：

1. onMount 时取得或编译 effect，准备绑定，通过 scope.animation(0.0) 创建进度动画，并登记自有资源。
2. 点击等事件发生时，将进度按所选动画从 0 推进到 1；不在 paint 中启动或重启动画。
3. 每个实际帧由 Surface 先采样动画；Element.paint 使用当前尺寸、Animatable.value 与 Props 中的颜色更新 uniform，随后绘制效果。
4. 动画结束后呈现最终画面并撤销帧需求；组件卸载时按 scope 的正常顺序释放动画、绑定和自有缓存。

业务 State 仍在 Component.render 中读取并传入 Props。单纯的动画参数在 paint 中读取已经采样的 Animatable，不为每帧 uniform 更新再写 State，也不要求父组件逐帧重建描述。只改变 uniform 不重新编译 SkSL；程序或输入结构变化按实际需要更新相应缓存。

effect 本身没有时钟。声明 time uniform 不会自动提供时间或安排帧，包含正弦、噪声等函数也不会自发驱动绘制。持续效果由调用方显式启动动画或提出帧需求；暂停、取消、完成和卸载后撤销需求，隐藏状态沿宿主既有规则停止无效绘制。没有失效和活动动画时，保留 shader／uniform 缓存即可，不运行检查时间的 timer，不遍历组件树，也不持续提交 GPU 工作。

## 5. 坐标、颜色与图层边界

SkSL main 收到 shader 的局部坐标；没有额外 shader 局部矩阵时，可以对应 Element 的局部逻辑坐标。图像 shader 的采样位置按图像像素尺寸表达，不能直接当成 0 到 1 的 UV；尺寸适配与矩阵转换由调用方和绑定明确，DPI 根变换仍只施加一次。

颜色 uniform 使用 layout(color) 标记时，按 Skia 约定传入非预乘 sRGB 颜色并转换到绘制工作色彩空间；普通数值 uniform 不自动按颜色转换。shader 返回预乘 alpha，示例中先算 alpha 再乘 RGB；已经通过图像 shader 取得的预乘颜色不能重复乘 alpha。效果继续遵守当前 sRGB 输出合同，不因 SkSL 可以计算任意数值而自动扩展为 HDR 输出。

对子树整体施加效果时，扩展图层接入并使用 SkImageFilters::RuntimeShader 等实际入口，将子树绘制结果作为输入。采样范围、输出边界、裁剪与缓存失效需要明确；影响像素范围和采样次数的成本随真实场景验证。仅处理自己的子树不自动取得组件背后的背景。当前 saveLayer(opacity) 的存在不代表这些能力已经实现，也不需要先建立通用 render graph。

着色器改变像素，不自动改变组件布局、子项位置或 InputArea 命中。扫光、变色等效果通常沿用原几何；涉及位移或非线性扭曲时，要明确保留原输入区域，或由效果适配提供对应坐标映射。不能从任意 SkSL 程序自动推导可交互几何、caret 位置或逆变换；普通几何动画优先沿现有 Canvas／Surface 共享变换处理。

## 6. 后续批次与验证

| 批次 | 交付 | 必要验证 |
| --- | --- | --- |
| effect 与 Paint 接入 | 编译、uniform／输入绑定、程序化填充及已有 Image 效果 | 实际后端输出、参数与编译诊断、颜色／alpha／坐标、多实例参数隔离、源程序复用及资源关闭 |
| 自绘组件与动画组合 | 普通 Element 通过 Animatable 驱动效果，scope 管理资源 | 扫光或波纹的真实输入触发、尺寸／DPI 变化、动画结束／暂停后停帧、隐藏恢复和卸载 |
| 图层与子树效果 | 明确输入、采样范围和输出边界的图像滤镜接入 | 文字／图片／子控件整体效果、裁剪边缘、命中策略、缓存失效和失败时资源收尾 |

使用成熟 Skia 能力和现有后端先完成代表性闭环，再扩展效果类型；不先导出空 shader／filter 接口。普通行为归 graphics，UI 组合归相应组件或集成测试。记录实际验证的后端与目标，不把上游接口存在或示例源码检查当成 Scoop 已运行验证。

性能与能耗验证观察源程序编译次数、参数更新、实际绘图和提交：连续动画复用程序，不逐帧重新编译或读回 CPU；静止区间既没有新增 GPU 提交，也没有空的帧回调和 UI 遍历。GPU 资源完成实际使用后释放，不能用提前关闭或强制每帧同步等待来满足测试。

## 7. 调研依据

- Skia 官方：[SkSL 与 Runtime Effects](https://skia.org/docs/user/sksl/)，包括输入 shader、局部坐标、颜色 uniform 与预乘 alpha。
- 固定版本源码：[SkRuntimeEffect.h](https://github.com/google/skia/blob/f326a1e25fc736704063922b9ce76b527b1c5efe/include/effects/SkRuntimeEffect.h)，包含编译结果、uniform／child 反射与 shader／color filter／blender 创建入口。
- 固定版本源码：[SkImageFilters.h](https://github.com/google/skia/blob/f326a1e25fc736704063922b9ce76b527b1c5efe/include/effects/SkImageFilters.h)，包含 RuntimeShader 图像滤镜、输入绑定与采样范围约定。
- 本仓库：[绘图绑定](../../../sysroot/src/scoop.graphics/native/shared/drawing.cpp)、[Skia 依赖说明](../../../sysroot/vendor/skia/README.md)、[UI 公共自绘协议](../../specs/SCOOP-UI-SPEC.md#7-自绘布局输入与动画)。
