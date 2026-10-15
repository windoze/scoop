# Compile shape-support 保存角色读取

实现基线：`f10d38cd`。继续按本机集中修改与清理、定向验证、最终统一完整复验的顺序推进。

## 实现与退役

param-free shape-support 直接读取保存的八种 role，解析 source nominal、exact、layout／scan／descriptor／registration 与 boxing／coroutine helper 的 typed 引用。不再从 MIR root 清单和源码 nominal 重新生成同一闭包，也不重编码候选与期望逐项比较。

删除 MIR→LIR shape-support 重建 API、单条／整表对照 reader、测试专用重复表构造器、失去调用的 source-ref 适配入口及错误变体。删除整组候选修改工厂，保留真实源码的 source、box、coroutine helper 断言和生产端 payload／descriptor／layout 规则；core 与普通产物 round-trip 均改走实际 reader。所需 source 与 descriptor role、typed 引用及 canonical source 顺序在读取边界处理，wire／profile 与 runtime ABI 未变。[退役清单](shape-reader-retirement.json.gz)。

25 个代码／规范文件新增 227 行、删除 450 行，净删 223 行。新增角色读取模块 55 行，fixture 身份登记 helper 35 行；通用 registration 字段读取复用上一批 definition reader。

## 本机验证

格式化和 [workspace clippy](shape-reader-darwin-clippy.log.gz) 通过，无警告。新读取路径通过 [452 项 LIR 测试](shape-reader-darwin-lir.log.gz)、[527 项 slib 测试](shape-reader-darwin-slib.log.gz)和[真实 shape-support standalone／combined 源码与产物](shape-reader-darwin-source.log.gz)，后者耗时 62.07 秒。最后删除无调用的 source-ref／排序适配函数后，再执行 fmt／clippy 和 [6 项直接受影响的 shape-support 测试](shape-reader-darwin-final-unit.log.gz)，没有重复整套组合验收。

检查 target：tmp 为空，flycheck0 只有编辑器 stdout／stderr，没有新增已结束 fixture 工作目录；保留 debug／release 增量缓存。本轮未执行 Linux、完整 workspace／CLI 或性能轮次。

## 剩余范围

本批仅处理 ordinary／core 共用的 param-free 导出读取。descriptor 和 layout 的重建、physical 导入对照、其余 HIR／strong-profile 及 writer 检查、ProgramLink identity／foundation、dump 复用和最小阶段仍未全部清理；现有 strong-production shape-support 路径也需随该边界继续处理。先完成全部本机修改和清理，再本机完整复验，通过后统一 Linux GNU／musl 与至少五轮性能验收。M48 继续实施，M38 未推进；既有 M38 文档修改不在此提交中。
