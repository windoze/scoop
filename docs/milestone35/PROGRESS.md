# M35 实施记录

基线：`75a534ef2`，分支 `review-20261010`；设计见 [DESIGN.md](DESIGN.md)。

## 批次

| 批次 | 状态 | 实际交付与验证 |
| --- | --- | --- |
| M35-1 规范与 case 迁移 | 已完成 | 显式 case；core/JSON/fixture/Rust 源码迁移；29 个定向 CLI fixture 与模式 Rust 测试通过 |
| M35-2 普通 when | 待实施 | subject/解构、条件、smart cast、覆盖、正常结果 |
| M35-3 跳转与 Elvis | 待实施 | 跳转表达式、结果合并、求值与清理 |
| M35-4 完整尾随调用 | 待实施 | 后缀、参数映射、全部调用入口 |
| M35-5 注解数组 | 待实施 | 静态值、完整类型、共享格式 |
| M35-6 组合与回归 | 待实施 | 适用平台与正式 CLI 验收 |

## 验证与构建目录

每批先格式化与 lint，再运行相关 Rust 测试和文件 fixture；最终执行一次完整 workspace 与 CLI 验收。Linux GNU/musl 使用 `nuc12:~/repos/scoop`，只记录实际运行结果。

2026-10-10 开始时本机 target 约 21 GB（debug incremental 约 13 GB）；LLVM 使用已配置的 LLVM_SYS_221_PREFIX 指向的 22.1 工具。

M35-1 开发验证：解析器 459 项及 1 项 doctest、HIR 模式 34 项与 M22 103 项、MIR 模式 10 项通过；文件 fixture 验证进行中。2026-10-10 清理 debug incremental 后 target 从约 25 GB 降至 16 GB，保留已构建工具与依赖。Linux 已确认基线相同，按用户要求待本机全部完成后统一验证。

## M35-1 交付

- 同步语言与实现规范，保留原章节引用，路线图登记 M35。运行时合同未发生变化。
- when parser 独立为 108 行模块，case 仅在模式头部作为上下文关键字。原模式矩阵、guard 与类型检查保持；新反例锁定缺少前缀的位置与迁移提示。
- 迁移 233 个现有 Scoop 源码文件、Rust parser 测试与两处 fixture 动态源码。core、JSON、各历史里程碑中的绑定/模式保持原行为，声明位置不增加 case。
- `cargo fmt --all`、`cargo clippy --workspace --all-targets` 通过。`cargo test -p scoop-parser` 通过 459 项及 1 项 doctest；HIR 的 m4::when 34 项、m22_ 103 项，MIR patterns 10 项通过。
- 构建 debug/release 三个 CLI；通过 29 个定向文件 fixture，包含新 case 独立/组合/negative、M4、M22 完整整数域与递归覆盖、M26 字符、M30 浮点模式。新 fixture 及受警告诊断中断的 golden 已再次严格比较。完整记录见 [case-darwin.json](validation/case-darwin.json)。
- 首轮失败仅为添加前缀后的源码偏移与受其阻断的 golden，已按实际结果修正并仅重跑失败/新增用例。优化版编译器使相同组合 fixture 从约 26 秒降到约 6 秒。全部仓库回归留到功能完成后统一执行。
