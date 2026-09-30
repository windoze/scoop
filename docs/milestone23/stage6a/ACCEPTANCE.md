# M23-6a 实际验收

状态：已完成并验收（2026-10-01）。对应 [设计第 8 节](DESIGN.md#8-验证与完成门)；按功能提交的实现与中间证据保留在 [进度记录](PROGRESS.md)。最终验收的代码、测试与快照基线为 `af0b55e99`。

## 1. 交付结果

当前源码与依赖导出图共用 nominal application、带原 binder 的完整声明、已检查正文、调用与构造目标、局部捕获和默认值表达式。声明保留原 typed 身份；当前／依赖位置只负责定位存储，不参与原声明、完整 application 或具体化请求的身份。

- struct／enum／class／interface 各用一种 application 和定义。字段、父类型、上界、conformance、解构与模式查询使用完整实参，普通宿主的封闭泛型父类型和整数范围 iterator 进入同一实际物化过程。
- callable／nominal 候选使用共同完整参数视图、实参映射、约束环境、上下文固定点、MSC 和 literal 偏好。失败事务直接供诊断使用；函数引用共用完整签名适用性和声明比较。
- 直接／成员／限定 super／局部调用、构造、variant、单例、函数引用、lambda 和匿名函数使用共同语义节点。真实目标、dispatch、定义位置、求值位置和词法捕获保持。
- 默认值展开与正文具体化分别保留一个共同执行过程。wire 读取适配只恢复原类型、身份、局部 selector 与同结构正文；`materialize_imported_default` 已转交共同 `instantiate_default_expression`，不再另行解释默认值。
- 函数、构造和初始化请求按原声明、完整 owner／callable 实参及实际词法身份去重。未使用父构造器不因词法查询发射；普通外部存储、初始化和函数实现继续引用原提供方。
- 调用按 receiver／callee、源码显式实参、形参默认值的既定顺序执行。收尾修复了依赖 class 隐式属性查询的本地 arena 假设，以及函数值调用中后续前置语句越过此前求值的问题。

正式 HIR 接口使用已落实的 `hir/cross-cone-interface/43`；实际新增的 NoGC／pointee 条件、固定向量、旧格式拒绝和产物重建已经同步。只改变内存节点或处理过程的批次没有无理由提高 wire 版本。runtime C ABI 保持。

## 2. 完成门及实际证据

| 完成门 | 实际验证 |
| --- | --- |
| 同一导出图的内存／wire 等价 | `exported_graphs_have_identical_direct_and_wire_consumers` 使用四组真实 provider／consumer，精确比较声明和正文、消费方 HIR、具体 MIR、再次发布的 foundation／interface 与 MIR foundation 字节；不归一化原身份 |
| 合法声明位置变化 | `m23-shared-*` 的 standalone／combined 正例和成对诊断覆盖当前／依赖声明；收尾新增 host-properties、declaration-views、callable-order 检查隐式属性、引用签名和求值顺序 |
| 双向泛型组合 | 本地泛型配依赖 nominal、依赖模板配 consumer-local class／String／Int／Unit、不同依赖的类型与模板，以及再次发布；完整 owner／callable 实参、上界和原默认来源保持 |
| 设计中明确的缺口 | closed-generic parents、integer ranges、bindings／patterns、默认函数值各有真实产物和运行 fixture；core 的 13 个闭合应用逐项检查 exact ID、原 group、OdrWeak 和布局 |
| 真实语言组合 | 主／次构造和公共初始化、属性与委托、数组／vararg、Option、异常／finally、闭包／捕获、引用、指针／FFI 条件及已有 suspend 约束通过相关单元、阶段 golden 和产物回归 |
| 可见性和定义处绑定 | 私有／受保护访问、非法 public default、错误 bound、重载失败与歧义保留具体诊断；support 不成为公开 import，消费方名称不重新绑定已检查正文 |
| 物化和归属 | 原 application 去重、完整未使用宿主参数、后置构造器、独立捕获创建点与普通提供方状态得到实际检查；不同 producer 的 ODR 重叠及运行 closure 继续核对原身份 |
| 格式和运行 | 缺失或错误 owner／binder／引用的既有反例通过；正式编译源码、发布 `.slib`、移走源码、再次发布、链接及普通／moving GC 运行均覆盖通过 |

语义 HIR 的 `Type::Imported*`、`ExprKind::Imported*`、`ExprKind::LocalFunctionCall` 及 `ImportedConstructorKind` 已退役。保留的依赖记录和读取模块承担实际存储、原声明引用和 wire 转换，不作为第二套语言算法。

## 3. 最终验证

使用 LLVM 22.1，构建和测试关闭增量编译及调试符号；真实 driver 测试显式设置 `SCOOP_TEST_PAIRED_SCOOPC`，正式验证关闭全部 `SCOOP_UPDATE_*` 和 Insta 更新开关。

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --target-dir target/m23-6a
cargo build -p scoopc --all-targets --target-dir target/m23-6a
cargo test --workspace --no-fail-fast --target-dir target/m23-6a -- --test-threads=8
```

完整 workspace 首轮为 **5294 passed、17 failed、0 ignored**。逐项核对后同步旧节点名称、默认值局部名称、实际接收者／实参保存、物化输出和格式指纹的快照，将两条属性赋值错误收敛为同一位置的具体类型不匹配诊断，并修正普通 class 的封闭泛型父类型已物化后所需 `Unit` 的旧断言；关闭全部更新开关后，17 项失败测试完整复验通过。因此 **5311 项全部覆盖通过，最终未解决失败 0、忽略项 0**，不是将首轮日志改记为一次全绿运行。首轮与复验使用的生产 `scoopc` 文件 SHA-256 完全一致，测试二进制只更新上述物化断言。

覆盖包括编译器、driver、构建／缓存、产物边界和 runtime；collector、GC／stack-map、EH 的九项 C 回归实际编译并执行，完整 core 初始化、Link／Runtime 损坏输入和执行 fixture 均通过。此前 90 项泛型产物与完整 core 回归也已用同一配套编译器关闭更新开关覆盖通过，作为专项证据保留。

最终日志与汇总：

- `/tmp/scoop-m23-6a-workspace-fmt-json.log`、`clippy-json.log`、`build.log`（后两者同属该前缀）；
- `/tmp/scoop-m23-6a-workspace-tests.log`、`first-results.json`（同前缀），保存首轮的真实失败；
- `/tmp/scoop-m23-6a-workspace-repair-fmt-json.log`、`clippy-json.log`、`build.log`、`verified-results.json`（同 repair 前缀）；快照更新后的最终 fmt／clippy 另记于 `workspace-repair-final-fmt-json.log`、`workspace-repair-final-clippy-json.log`（同 `/tmp/scoop-m23-6a-` 前缀）；
- `/tmp/scoop-m23-6a-workspace-summary.json`、`compiler-pairs.json`、`snapshot-review.json`（同 workspace 前缀），记录合并覆盖、编译器一致性和逐文件差异；
- `/tmp/scoop-m23-6a-wire-equivalence-verified.log`；
- `/tmp/scoop-m23-6a-callable-order-all-verified-results.json`、`snapshot-review.json`（同前缀）。

阶段中先核对再更新必要的 golden；收尾求值修复涉及的 161 份旧快照保持全部类型／函数／callback 声明头一致，并在更新开关关闭后复验。完整 workspace 收尾另同步 75 份快照：39 HIR、15 MIR、13 LIR、5 份归档指纹、2 份属性赋值诊断和 1 份类型使用列表。函数／类型声明头除显示编号外保持，五份归档的 Code／Runtime 指纹及两份诊断的源码位置保持。真实语言反例没有改成成功，也没有以禁用测试消除失败。

## 4. 纪律与后续范围

实现按功能提交；文件按声明、存储、求解、替换或验证职责拆分。收尾共同参数视图为 42 行，函数值调用模块 123 行，调用分类主模块 252 行，core 应用检查模块 69 行。阶段中持续清理闲置 `target/debug` 和旧增量缓存；最终清理记录见 `/tmp/scoop-m23-6a-workspace-cleanup.json`，保留实际使用的 `target/m23-6a` 热缓存。

没有新增来源授权、防伪、通用资源预算、计费或只服务假想使用者的框架；既有不可变事实在其明确边界完成验证后复用，查询没有为验收额外发射机器根。

M23-7 继续完成其生成机器定义／引用闭包、完整 ODR 合并和泛型委托运行任务。收尾特别确认了**外来参数自由 NoGC 函数的首次 C 地址需求**：本地取地址和普通外部调用成功，但提供方未发布 C storage bridge 时，消费方直接取该地址尚未交付。真实 CLI／LIR 对照及所需发布／引用服务已写入 [M23-7 设计](../stage7/DESIGN.md)，此处不将它记作已支持，也不通过复制外来 Strong 函数或假源码 wrapper 绕过。M23-8／9／10／11 的后续合同保持。
