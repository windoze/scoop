# Regex 实施记录

状态：实施中。分支：`codex/pcre2-regex`。依据：[设计](INVESTIGATION.md)、[公开规范](../specs/SCOOP-REGEX-SPEC.md)。

## 阶段

1. 公开合同：已写入独立规范。
2. PCRE2-8 准备、最小查询与捕获：已完成本机闭环，独立依赖准备与发行接入已提交；支持 Boolean、find/matchAt/matchEntire、组名／编号捕获、标量索引、结果快照及 close/use。
3. 惰性迭代、替换、分割与便利入口：已完成，分别提交。
4. JIT、索引转换与生命周期检查：已完成；JIT 在发布编译对象前尝试，锚定查询保留解释器路径，Boolean 不物化捕获与索引。
5. 三目标交付及相关测试：待完成。

## 验证边界

先在 Darwin/AArch64 验证，再使用 `nuc12:~/repos/scoop` 完成 GNU/musl。仅运行 regex 本身与实际消费者的 release `scoop.test`；不新增 fixture、底层缺口测试或上游 Unicode 测试。原始日志与构建工作区放在仓库外，不使用 `/tmp`。

Darwin/AArch64：PCRE2-8 准备、更新复用与生产 `.slib` 构建成功；19 个库测试和 3 个独立消费者测试通过，包含同一编译模式的并发只读使用。API 文档以 `--no-deps --deny-doc-warnings` 生成成功。Linux 验证尚未完成。

已使用 `cargo clean --profile dev` 清理旧开发产物，保留后续验证使用的 release 工具。
