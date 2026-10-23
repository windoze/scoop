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
| 语法与普通类型条件接口 | 本 Cone 基础闭环已完成；继承等组合另行验收 | 4 个 parser 用例、11 个 M57 HIR 用例、8 个诊断 fixture、4 个 core consumer 原生用例通过 |
| Companion、HashMap 与 Json 闭环 | 已完成 | 6 个 JSON Cone 原生用例覆盖四种能力组合、嵌套、显式工厂及重复 key 路径 |
| 继承、default、property、效果与 codec 合成 | 待完成 | 尚无实现验收 |
| 导出、跨 Cone 与 ODR | 基础闭环已完成；继承组合另行验收 | 83 个泛型导入回归、条件记录 wire 往返／再次发布与双 consumer 原生链接运行通过 |
| 完整组合与 Linux 验收 | 待完成 | 尚无完整验收 |

此表只在实际代码及对应验证完成后更新，不以规范已写入代替功能交付。

## 3. 基础闭环

- AST 保留独立实现块；普通类型和 companion 复用原宿主参数，方法／计算属性的条件不改变类型布局或单例身份。
- 支持条件关系、直接调用、泛型转发、方法引用、属性访问、接口转换和动态接口判断；普通 override 可满足多个条件接口义务。未证明的条件不成立为接口事实，也不能作为 M55 负向约束的证据。
- 实现重叠、重复签名、无关 override、非法存储和负向条件有正式 CLI 诊断覆盖。循环／扩张查询被拒绝，有限嵌套递减和独立成功证明有 HIR 覆盖。
- Concrete 接口关系从声明求值，不读取尚未构造完成的接口表；递归基类实例化顺序有原生覆盖。
- 本机 workspace Clippy 通过。HIR 回归首次 1,526 通过、15 失败；失败均为导入 intrinsic 的独立存储路径，修正后相关 128 个回归用例通过。后续关系求值调整通过 M55／M57 共 29 个用例。
- 已显式准备 Darwin 的 21 个系统 Cone 与 runtime；正式验证消费共享 sysroot／Cone cache。跨 Cone 条件元数据、复杂继承／default、codec 合成及 Linux 验收仍待后续步骤。

## 4. 条件关系的跨 Cone 消费

- `cross-cone-interface/79` 保存原宿主 binder 下的条件声明、成员适用性及带完整 owner application 的 slot 选择；导入后沿用定义处目标，互斥条件下的 default 选择保持独立。
- 参数的 public companion 声明在具体化前随实际泛型实参保留，避免仅使用 Any／动态判断时漏掉接口关系；查询不执行 companion 初始化。
- 无泛型的实现块同时导出普通继承边和接口槽位，class／struct／enum／object 均有从源码到 MIR 分派表的验证。
- 本机 workspace Clippy、867 个 HIR 格式测试、83 个泛型导入回归通过；16 个 M57 定向用例覆盖本地、导入及产物分派。格式版本变更导致的一项 slib 期望向量失败已更新，相关 17 个 profile 测试通过。
- 正式 ODR fixture 由 provider、两个 library consumer 和 root 构成，验证条件真／假、非泛型实现块、单例共享及不同实参独立；原链接与逆序依赖重链接均运行成功。Darwin sysroot 已按新格式显式重建。
- HashMap 默认 codec、继承／合成组合、完整 Linux 验收仍未完成。

## 5. HashMap 的默认 codec

- HashMap 原 companion 分别声明 ValueEncoder 与 ValueDecoder 条件实现，使用 K／V 的 public companion；保留原存储、键约束、显式工厂及 map helper。
- JSON Cone 的 6 项原生测试通过：编码／解码独立，两个组件均须满足对应条件；没有 companion 仍能正常使用容器及显式 codec；覆盖嵌套 HashMap、泛型转发、原 entry 表示、新建解码结果和重复 key 的 `/1/0` 路径。
- 明确验证显式 codec 优先于已有默认能力。其他容器未自动迁移，条件 codec 合成及继承组合继续实施。
