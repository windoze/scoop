# scoop.ui

普通 Scoop Cone 实现的声明式自绘框架。Component 是保留身份的 class，以成员 State 保存状态，通过 render 返回不可变 View 描述。Element 使用公开的约束、子项测量／放置、Canvas、InputArea 和 Surface 动画绘制自身。框架不提供 Label、Button、默认样式或容器布局策略。

当前已在 macOS aarch64 实现独立自绘闭环及公共文本子包；独立控件库按批次接入，x86_64 留待本期功能完成后补验，Windows 后续实施。公开合同见 [UI 规范](../../../docs/specs/SCOOP-UI-SPEC.md)，当前验证范围见 [实施记录](../../../docs/ui/PROGRESS.md)。

## 组合与状态

- `component(create, key)`／`component(props, create, key)` 在挂载时创建组件；`element` 以相同方式创建自绘元素，并接收显式 children。
- `state(initial)` 支持成员属性委托，也可以由普通业务 Store 持有并供多个组件共享。render 中读取的 State 建立依赖；写入通知在 owner 上合并，条件读取改变时替换订阅。
- `Props<P>.value` 提供最新父级输入；`State.binding()` 或 `binding(get, set)` 读写原状态，不创建另一份值。
- key 在直接父级内生效，节点类别及组件和 Props 的完整类型参与匹配。新挂载必须返回新的 class 实例；普通更新保留实例与本地状态。
- 业务依赖可通过语言 Context 在实际运行／挂载入口安装。临时创建 View 不捕获 Context，属性委托在组件初始化时绑定一次。

State 在创建它的线程读写；render／measure／paint 不允许写入 State。后台结果通过窗口 ApplicationProxy 或现有 EventLoopSession 投递到 owner，再修改状态。逻辑更新不依赖 drawable，隐藏窗口仍可更新 Props、卸载子树及关闭资源。

## 自绘与资源

Element.measure 返回 LayoutMetrics，包含满足 Constraints 的尺寸与可选基线。每个直接子项测量一次，然后放置零次或一次；未放置子项保留挂载与状态，本帧不绘制。Element.paint 取得局部坐标的独立 Canvas 借用，在背景与前景之间调用 paintChildren。坐标与 clip 同时用于 InputArea 放置，成功呈现后才发布命中几何。

在 onMount 中使用 ComponentScope.resource 登记 Path、Image 或缓存，使用 createArea 和 animation 登记稳定的输入区域与 Surface Animatable。应用不为每个组件 use；作用域按子先父的顺序退休输入／动画、调用 onUnmount，再逆序关闭资源。可替换缓存应自行及时关闭旧值，不能每帧登记一个新资源。

最外层用 `UiHost.use` 包住 Application.run，或使用 `runUiApplication(title, create)`。SurfaceContent 拥有 UI 根，原生窗口关闭、异常和显式关闭都释放挂载树。UiWindow.close 只关闭所附着的 UI／Surface 内容，原生 Window 的归属仍由调用者决定。绘制或逻辑回调内请求关闭时，先停止新工作，回调结束后再释放资源。

无状态失效、显式帧请求或活动动画时，框架不遍历树、不投递空任务；Surface 停止请求原生帧。动画通过 Surface 同一时间轴采样，直接参与本帧测量与绘制，不逐帧重跑未失效的 render。

## 公共文本适配

`scoop.ui.text` 用 `InlineView(key, view, size, alignment)` 连接 graphics.text 内容中的 InlineKey 与普通 UI 子项。size 为 None 时采用自然尺寸，可指定固定 Size；对齐默认采用子项的 alphabetic baseline，缺失时取底边，也可以选择 ideographic baseline 或显式 InlineAlignment。

描述阶段把 `inlineChildren(content, inlines)` 作为 Element 的 children，Element 在 onMount 中登记 `scope.resource { ParagraphHost() }`。measure 调用 `host.measure(context, constraints, fonts, content, settings, inlines)`，paint 调用 `host.paint(context, parent)`。这组接口完成子项测量、段落布局、可见对象放置、文字绘制和子项绘制；图片同样使用普通 Element 包装。

ParagraphHost 只拥有 Paragraph／TextLayout，不拥有字体或子组件。它按同一内容对象、字体身份及 revision、段落设置和实际宽度／metrics 复用缓存。布局动画改变 metrics 时重排，纯子项换色可复用布局；新内容描述可以触发重建。`host.layout` 借出最近成功测量的结果，供 hitTest／caret／selection 查询，调用方不关闭它。缓存替换或宿主关闭后旧布局资源失效。

行内出现的 key 与 View 自己的 key 分别参与普通匹配。插字、RTL 顺序或换行不丢失子组件状态；截断项保留挂载，成功帧后退出命中和焦点，恢复后继续使用原实例。宿主不隐式裁剪，调用方可用同一个 Canvas 的 clip 包住整段文字及子项。

## 示例与定向测试

[自绘 Counter](examples/custom/README.md) 只使用框架及下层公开接口，实现点击、Path 绘制、状态和宽度动画，不依赖控件库。

```sh
scoop test sysroot/src/scoop.ui --release --target aarch64-apple-darwin --sysroot /path/to/sysroot
scoop test sysroot/src/scoop.ui/tests-cones/drawing --release --target aarch64-apple-darwin --sysroot /path/to/sysroot
scoop test sysroot/src/scoop.ui/tests-cones/errors --release --target aarch64-apple-darwin --sysroot /path/to/sysroot
scoop test sysroot/src/scoop.ui/tests-cones/text --release --target aarch64-apple-darwin --sysroot /path/to/sysroot
```

普通测试验证状态／Binding、线程规则和约束，并以隐藏原生窗口验证多根组合；drawing Cone 从公开接口验证真实绘制、输入、动画、可选放置和回调关闭；errors Cone 验证构造、挂载、更新、布局和绘制失败下的清理。原生窗口测试顺序运行，避免焦点和遮挡相互影响。开发时先按仓库 scripts 说明准备目标 sysroot，并显式使用同一套 scoop／scoopc。

UiTextCacheTest 使用真实字体／段落验证缓存替换与失败恢复；text Cone 通过框架公开 API 验证富文本、固定尺寸图片、自然尺寸交互子项、动画重排、截断／恢复、RTL 和内部 key 更换。固定字体由 [text-fonts 测试数据 Cone](../scoop.graphics.text/tests/data/FONTS.md) 提供，不加入生产默认字体。
