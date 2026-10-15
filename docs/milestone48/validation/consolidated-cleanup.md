# M48 本机集中清理与统一验收

基线：`9edf9060`。本记录覆盖此前 layout／descriptor 批次及本次合并清理；改动尚未提交，M48 尚未完成。M38 未推进，用户的 M38 文档及整份 M45 草稿不纳入本批提交；收尾仅同步 M45 草稿一处旧隔离验收措辞。

## 完成的本机修改

- layout、descriptor、callable ABI、dispatch、shape support 和 physical import 采用保存记录；删除语义重建、完整重编码比对及合同副本。physical import 格式升至 `cross-cone-layout-abi/16`，Code contribution 继续采用实际合同，runtime ABI 保持。
- 删除旧 single-cone strong 和 cross-cone semantics strong Compile 流水线、专属候选工厂及 native ABI 消费重放。当前 shared reader 继续承接源码依赖与显式产物。
- 删除 LIR 完整 production、registration、digest、image、entry、generated bridge、symbol 等消费者复核 API。生产端构造、目标对象、GC、ABI 与必要格式／引用检查保留。
- 删除正常 Compile 中 HIR 声明、默认参数、const、intrinsic 与调用点语义重放，以及不再使用的 HIR authority、route authority。实际源码 HIR lowering、类型需求计算、typed 引用解析与别名展开保留。
- 删除仅供测试调用的 shared type foundation／继承／表示重放和相应改写工厂；代表性 MIR 结构断言直接消费真实生产数据。core protocol 与 MIR 保存记录不再重复比对原始签名、完整生产清单或入口重编码结果。
- dump 按编译输入键保存并复用。缺失观察只执行到最高请求阶段；AST 不进入 HIR，HIR／MIR／LIR 观察均在机器对象、归档与链接生产前结束。machine protocol 升至 11，增加独立 observation 响应，删除 dump 内容摘要与目录审计。
- 新增独立 dump CLI fixture，扩充多 Cone dump 组合：逐阶段补算、全部复用零 child、来源范围选择、源码变化失效、机器产物 metadata 不变及实际运行。

代码、规范及测试的已跟踪部分涉及 512 个文件，新增 1,432 行、删除 42,092 行；统计不含新建文件与本记录。删除 253 个旧文件。新增 reader、协议和观察编排按实际职责拆分；ordinary execution 240 行、production 编排 200 行、observation 响应 47 行。

[退役与改名清单](../CLEANUP-TESTS.tsv)按相对于基线的 Rust `#[test]` 名称记录：261 个原名称移除或改名，19 个新增或改名。它是源码清单，不是执行数量；混合测试中的生产／格式断言继续保留，正式语言 fixture 未因本批消费重放清理而删除。

## 统一本机验收

- `cargo fmt --all`、整仓 Clippy 通过且无警告；期望更新后再次 fmt check／Clippy／diff check 通过。
- Ruff 0.16.10 格式化与检查通过；fixture runner 公共规则 49 项通过。
- 完整 Rust 首轮发现内嵌存储读取和版本固定向量问题。集中修正后合并复跑受影响的六个 package，最终合计 5,314 项通过、0 失败、0 忽略。
- 正式 CLI 全量发现 4,107 项，使用当前 release 工具、LLVM 22.1.8 和 10 并发。首轮 4,085 项通过、4 项失败、18 项平台不适用；集中更新 dump 复用、不同物理源码路径和 UTF-8 报错阶段的旧期望后，4 项合并复验全部通过。最终 4,089 项适用用例全部通过，4,613 variants、17,040 processes、88 项 stage／plan golden。两轮使用相同编译器二进制，没有再次重跑整套 CLI。
- 本机五组各五轮性能复测完成，共 55 次构建。暖构建中位数 0.058–0.061 秒、零 compiler build child；native 头文件变化中位数 0.691 秒，仅编译一个 Cone，见[性能记录](../PERFORMANCE.md)。本机工作全部稳定后开始统一 Linux GNU／musl 验收；本批尚未提交，M48 尚未完成。

正式 runner 保留汇总报告，外部一次性验收脚本在每个用例通过并结束后清理该用例目录；失败目录保留至复验通过。最终证据保存后，两个已结束的 fixture 工作目录及其公共缓存全部清理，释放约 70 GiB；debug／release 编译缓存继续复用。CLI 首次因 llc 未在 PATH 中而在配置阶段退出，显式配置 LLVM 22.1.8 后运行；4 并发试运行通过 19 项，随后改为 10 并发完成整套验收。

统一复验修正：表示中的内嵌值并不一定拥有独立 layout symbol。现保存字段、MaybeUninit、box 和数组元素的 typed layout key 与完整存储，不要求额外导出、不恢复 MIR 重放；新增保存／读取与字段范围回归。协议编码尾部索引、profile 版本、缓存键及语义摘要固定向量同步本批版本变化。

## 本批 Linux 验收记录

nuc12 使用保留的增量缓存，fmt、整仓 Clippy、完整 Rust 5,350 项和公共 runner 49 项全部通过。GNU CLI 后因 MORE-CLEANUP 集中修改而中断；本批结果不替代[最终验收](README.md)。已清理此前结束的 33 个 fixture 目录，保留编译缓存与日志。
