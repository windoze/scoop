# M23-11 文件 fixture 收尾清理

正式发现入口 `python3 tests/run_fixtures.py --all --list` 已检查全部源码归属：2067 个 fixture、3040 份 Scoop 源码，未归属 0；原 M1–M22/M25 的 534 份与 M0 smoke 均保留。完整执行结果另记在最终 ACCEPTANCE.md，发现数量不代表运行通过。

最终这批删除 14 份重复／无调用者的源码快照及其 Rust 维护代码：reference-source-fields 两份、generic-body-production 四份、generic-initialization 四份、layout-publication 两份、type-slot interfaces 一份，以及 extension-property-rules 一份。六项 extension property 原错误的位置和文字均与现有完整 JSON 逐项一致；其余源码已经由统一 CLI fixture 覆盖，未删原始源码。

Rust 保留必要的内部不变量：字段身份、声明顺序、class/object kind、binder、私有支持与 wire roundtrip；泛型正文、初始化和 dispatch 的源身份／ODR 目标；实际产物发布、损坏和原子替换。发布测试改为直接检查 object count 和 typed error，不再将相同结果格式化成另一份文件快照。

七处依赖当前 target 的集成检查不再在目标缺失时提前 return，缺少必需工具会明确失败。本阶段已完成的 suspend 物化使 SharedUnitDeferred 的初始化与 ensure pair 正式存在；对应内部预期从源码 3／物化 2 改为 3／3，并核对两个集合的全部实际 unit ID，而不是保留旧的阶段限制。

保留的 35 份 tests/fixtures 下 .snap 均服务直接构造 typed 数据的内部测试：MIR stage 11 份、shared type uses 19 份、native boundary 3 份、external boxing 手工 LIR 2 份。它们不承担另一套语言源码 fixture 发现、执行或验收。不存在旧 source snapshot 的 fallback 或逐 case Python 分支。

格式、workspace Clippy 与受影响的 HIR 结构、driver 布局／发布、初始化单元、普通产物 pipeline 和依赖 preflight 检查通过。布局组先发现旧 3／2 预期，修正后该项单独只读复验通过；全仓结果以最终无筛选执行为准。
