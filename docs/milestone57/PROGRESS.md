# M57 实施记录

状态：实施中。更新：2026-10-23。目标为 [DESIGN.md](DESIGN.md) 的普通类型与 companion 条件 interface 实现；整份类型特化不在本期。

## 1. 基线与工作方式

- 基线：`f3f8ace69`；分支：`codex/m57-conditional-interfaces`。
- 已同步语言 9.1.8、companion 投影与 codec 来源合同，以及实现／运行时规范的条件关系和接口表职责。
- 分功能完成并提交。本机先做相应格式化、lint、单元／Cone／必要 CLI 验证，再在 `nuc12:~/repos/scoop` 做 Linux 完整验收。
- 临时文件使用仓库外 `../scoop-work/m57-temp`，原始日志使用 `../scoop-work/m57-validation`，不使用 `/tmp`。
- 开始实施前用 Cargo 清理旧 debug 产物，target 从约 28 GB 降至约 7.2 GB；后续在无活动构建时按实际体积继续清理。

## 2. 功能状态

| 功能 | 状态 | 验证证据 |
| --- | --- | --- |
| 语法与普通类型条件接口 | 待完成 | 尚无实现验收 |
| Companion、HashMap 与 Json 闭环 | 待完成 | 尚无实现验收 |
| 继承、default、property、效果与 codec 合成 | 待完成 | 尚无实现验收 |
| 导出、跨 Cone、ODR、动态接口表与 Linux 验收 | 待完成 | 尚无实现验收 |

此表只在实际代码及对应验证完成后更新，不以规范已写入代替功能交付。
