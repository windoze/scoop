# Regex 实施记录

状态：实施中。分支：`codex/pcre2-regex`。依据：[设计](INVESTIGATION.md)、[公开规范](../specs/SCOOP-REGEX-SPEC.md)。

## 阶段

1. 公开合同：已写入独立规范。
2. PCRE2-8 准备、最小查询与捕获：待完成。
3. 惰性迭代、替换、分割与便利入口：待完成。
4. JIT、索引转换与生命周期检查：待完成。
5. 三目标交付及相关测试：待完成。

## 验证边界

先在 Darwin/AArch64 验证，再使用 `nuc12:~/repos/scoop` 完成 GNU/musl。仅运行 regex 本身与实际消费者的 release `scoop.test`；不新增 fixture、底层缺口测试或上游 Unicode 测试。原始日志与构建工作区放在仓库外，不使用 `/tmp`。
