# M23-11 sibling 泛型、字符串与 adapter 合并

六个用例保留原 provider、left、right、consumer 源码和坐标。每个 Cone 通过公开 CLI 发布并移走源码，再构建普通 executable；只保留产物、runtime index、native archive 与独立链接工具后，按正序／反序显式依赖各链接一次，两份程序均执行普通和 moving GC 变体。GC epoch 与原数值、引用身份断言均保留。

实际 `.slib` 逐字重现预审产物，并执行原 typed ODR 检查：候选成员完整、owner 正确、正反输入的 key／ABI／definition 一致且候选次序相反。完整四阶段 golden 与正式 link plan 固定对应结构，24 份原阶段快照逐字相同。adapter 的原 `addresses.c` 继续作为输入，由 TOML 精确替换已经审核的 descriptor 符号并注册普通 `atexit` 检查；没有 C main 或手拼对象链接。

| 原功能 | 用例 | 只读进程／golden | 保留的检查 |
| --- | --- | --- | --- |
| `sibling_generic_instances_merge_through_artifacts_and_run_with_moving_gc` | `standalone`、`combined` | 32／44 | 每例共享正文和 registration 均至少 2；真实符号只保留一份 weak 定义 |
