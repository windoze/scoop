# Compile callable ABI 与 dispatch 读取

实现基线：`f147bf82`。本批只进行本机格式化、lint 与受影响测试；全部 M48 修改完成并通过本机完整复验后，统一 Linux 验收。

## 实现与集中清理

底层 callable ABI reader 直接读取保存的 signature、protocol、typed body／definition 与 symbol，使用已有 body key 和 definition plan 解析引用，不再调用生产构造器重新派生身份或重编码整个 ABI 表。单条／表级候选对照、仅为这些对照服务的错误类型与测试 API 一起删除。

Compile dispatch 直接读取保存的 table、owner、role、slot signature、implementation 与 ABI body 引用；reference-dispatch 的 receiver layout 借用已读取的本地或依赖 layout。移除 MIR schema 重建、callable ABI 查找对照及独占的候选修改工厂。保留格式、表顺序、entry position、typed 引用与 definition 关系；生产端 slot／receiver／ABI 检查及真实语言 fixture 保持。

现有 wire round-trip 改为调用实际 reader，覆盖四种 implementation、接收者适配、空表和完整表；物理导入组合测试登记实际消费者与 provider 的 typed 引用。ABI／dispatch 共用 definition 字段读取和 primary atom 查询，没有新增 wire 字段、profile 或 runtime ABI。44 个代码／规范文件新增 735 行、删除 1,131 行，净删 396 行。退役与改名清单列出专用重放模块和测试变化；改名不表示丢失真实用例覆盖。

## 本机验证

最终先格式化，再执行 workspace clippy，无警告。随后通过：

- 452 项 LIR 测试，含实际 reader round-trip 与物理导入组合。
- 527 项 slib 测试。
- 真实分派 standalone／combined 产物，61.82 秒。
- 真实泛型 ODR 产物，40.98 秒。

前半批 callable ABI 读取完成后，另有 ZST／间接返回／装箱 ABI 组合通过，59.62 秒；该结果先于 dispatch 清理，后半批由上述最终分派和 ODR 验证覆盖。一次误写的泛型测试过滤器匹配零项，未计入验收，随后使用实际测试名执行上述 ODR 用例。

## 后续与 target

本轮没有 Linux、整仓或文件 fixture 全量测试。检查 target 顶层，仅有 debug、release、flycheck0、tmp 和 Cargo 标记文件，没有新增待清理的已结束 fixture 工作目录；保留正在复用的增量缓存。前批已清理 9.43 GB fixture 工作目录。

剩余 layout／descriptor／shape／physical 消费重放、HIR 与 strong-profile 边界、writer、ProgramLink identity／foundation、dump 复用与最小阶段仍须处理。全部本机修改稳定后，再本机完整 Rust／runner／CLI 复验、统一 Linux GNU／musl 与至少五轮性能验收。M48 未完成，M38 未推进。用户既有 M38 文档修改未纳入提交。
