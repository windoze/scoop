# Compile 来源重放与依赖闭包集中退役

实现基线：`51bf6639`。本批仅进行本机受影响测试；全部 M48 修改和清理完成后，先本机完整复验，再统一 Linux GNU／musl 验收。

## 变更范围

删除 Compile 中普通 callable bridge 的 HIR／MIR 重建、MIR→LIR ABI 对照和 MIR dependency closure／call-site 来源复核。MIR/LIR 已保存的依赖记录继续解析 typed 引用，真正使用时按 provider 选择外部定义；生产端依赖闭包和物理引用选择保留。普通 LIR bridge reader 不再规范化重编码整个候选表，也不再提供只服务对照测试的 `validate_against`。

进一步集中退役 HIR→MIR 类型、callable、constructor、object、dispatch、coroutine、equality 来源对照及专用候选修改测试。MIR 正常 decoder 保留本层格式、typed 引用和签名关系；初始化单元从保存的 strong callable unit／role 读取 initializer 与 ensure，不再重算 generated identity 或重建 HIR source-unit 清单。实际缺失调用对和两者签名不一致仍报错。

原有真实源码案例、普通语言 negative fixtures 及代表性的 ABI、visibility、equality、object 和初始化输出断言保留；退役候选修改工厂和无消费者的重放 API、错误变体、参数及 helper。新增初始化读取测试覆盖两组调用对及缺失任一角色；调整现有 wire 测试直接调用 reader，保留排序、重复和本地 provider 错误。测试函数改名与删除／移动文件详见清单，其中改名不等于删除功能覆盖。

109 个代码／规范文件新增 388 行、删除 7,729 行，净删 7,341 行。没有 wire／profile 或 runtime ABI 变化；M38 文档的既有用户修改未纳入本批。

## 本机定向验证

先 `cargo fmt --all`，再 workspace clippy，无警告。随后相关检查通过：

- 452 项 LIR 单元测试。初次改写排序测试时重复登记 `Unit`，属于测试初始化错误；修正后最终整组通过。
- 19 项 MIR type-bridge section 测试，含新增初始化 pair 读取检查。
- 527 项 slib 测试。
- 真实初始化单元 standalone／combined 产物，61.27 秒。
- 真实 core MIR／LIR 完整导出产物，196.86 秒。
- 真实跨库 function／getter 选择，28.24 秒；此项在后半批 HIR→MIR 对照退役前通过，后半批由上述初始化及 core 产物测试覆盖。

没有每次删除后反复运行整仓或文件 fixture 全量，没有执行本批 Linux 测试、release 构建或性能轮次。上批中断的 property initialization 长测试仍未计入通过，留待整体验收。

## 清理与剩余工作

清理记录：删除 10 个已结束的本机 fixture 工作目录，释放 9,426,731,008 字节磁盘分配空间，保留 `target/debug`、`target/release` 增量编译缓存及工具。清理前保存历史 fixture 报告，含旧 test-purpose 路径断言失败及修正后的成功报告；这些历史结果不作为当前代码验收。

本批完成的是 Compile 外层来源对照与依赖闭包重放清理。LIR layout／dispatch／descriptor／shape 和 physical 路径、底层 callable ABI record 的 definition／规范化重编码、剩余 HIR 与 strong-profile 对照、writer 复核、ProgramLink identity／foundation 边界、dump 复用和最小阶段仍须继续处理。完成全部修改后，再执行本机完整 Rust／runner／CLI 验收，通过后统一 Linux GNU／musl 及至少五轮性能验收。M48 仍在实施，M38 未推进。
