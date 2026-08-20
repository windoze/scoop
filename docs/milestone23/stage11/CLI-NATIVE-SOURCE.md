# M23-11 原生边界源码闭环

四份原始 native-boundary 源码保持不变，先通过正式 CLI 发布 library 并记录 AST/HIR/MIR/LIR；工作副本只增加 C provider 的 lib 名称和执行断言，再由下游 Cone 调用。core、provider、增强 provider 和 program 都固定真实 artifact fingerprint。移走源码后，以不带配套 compiler 的独立 scoop 从产物重链，再分别在普通 GC 与 moving GC 下执行。

验收：4 个用例，36 个进程，52 份阶段／计划 golden，16 个产物指纹；只读运行全部通过。工作记录：`/tmp/scoop-m23-11-native-source-shapes/verified/report.json`。格式、Python lint、workspace Clippy、HIR native boundary 和 LIR native storage 测试通过。

| 用例 | 实际验证 |
| --- | --- |
| single | Scoop ABI 的 Int8 参数 17 与结果 18，C provider 确认调用次数 |
| combined | Scoop ABI byval 的 Wrapper／Choice，Data 的 packed Int8/Boolean、Text 的引用槽、Empty tag，以及八种有符号／无符号整数参数；既有 AArch64 存储约定由短汇编入口交给 C 检查 |
| handle-c | 从正式 core 导入 GcHandle 后，C uint64 参数与返回值双向转换，Scoop 侧比较真实结果 |
| handle-c-combined | NativeHandles 的 UInt8、PinnedPtr 和 GcHandle 字段，指向 handle 的原存储地址、nullable FunPtr 回调、CLayout 返回值与 extern global 的写入／读取 |

真实导入暴露的缺口已修复：首次加载声明时按完整 FFI protocol 的实际 nominal/field ID 保留 UInt64Field；native-boundary 持久化表只含已使用闭包，缺记录不再覆盖语言已有的 C 投影。LIR lowering 在 C 实参、返回、全局读写与 CLayout 字段边界显式拆包／封装。handle 的 Scoop aggregate ABI、指针存储、callback storage bridge 和产物格式保持原约定，codegen 继续要求准确的存储类型。

原 Rust 文件快照 combined.snap、handle-c-projections.snap 退役。其类型、完整字段、UInt64Field/NullablePointer、GC-free、MIR 投影和 callback 数量断言继续保留；新测试专门核对未被 core native-boundary 表记录的导入 handle。内部手工 MIR 的 C storage 与 Scoop ABI 测试继续保留。

全仓复验还同步历史 `legacy-m15-moving-handle-pin` 与 `native-link-runtime-handles` 的四份 HIR／MIR golden。每份 HIR 仅把导入 `GcHandle`／`PinnedPtr` 的 C 表示补为 `UInt64Field`，目标仍为原唯一字段的同一持久化 ID；MIR 仅补相应 C-ABI 标签。AST、Export 声明、LIR、运行结果与 native 输入均保持，未更改 Scoop 聚合布局或语言期望。
