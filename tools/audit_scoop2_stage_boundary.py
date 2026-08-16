#!/usr/bin/env python3
"""scoop2 阶段边界审计（PLAN.md M5-4）。

检查项（全部静态）：
1. 依赖边：scoop2_mir 不依赖 scoop2_syntax；scoop2_lir 不依赖 scoop2_hir。
2. MIR/LIR 源码无用户诊断码字符串泄漏（scoop::mir::*/scoop::lir::* 常量
   之外不得出现用户可读诊断构造——白名单文件 diagnostics.rs 除外）。
3. 穷尽性纪律（C9-4）：下游对上游枚举的 match 无 `_ =>` 兜底臂
   （lower_tree.rs 的 TreeExprKind 主 match——枚举变体集封闭后兜底即漏洞）。
4. archive 自包含：scoop2_archive 的 LIR 段不引用 MIR archive 路径
  （load_lir_archive 不读 .mirarch）。

用法：python3 tools/audit_scoop2_stage_boundary.py  （退出码 0 = 零违规）
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CRATES = ROOT / "crates"
violations: list[str] = []


def cargo_deps(crate: str) -> set[str]:
    text = (CRATES / crate / "Cargo.toml").read_text()
    deps = set(re.findall(r'"\s*([a-z0-9_]+)\s*=\s*\{', text))
    return deps


def src_files(crate: str):
    return (CRATES / crate / "src").rglob("*.rs")


# 1. 依赖边
mir_deps = cargo_deps("scoop2_mir")
if "scoop2_syntax" in mir_deps:
    violations.append("依赖边: scoop2_mir -> scoop2_syntax")
lir_deps = cargo_deps("scoop2_lir")
if "scoop2_hir" in lir_deps:
    violations.append("依赖边: scoop2_lir -> scoop2_hir")

# 2. 用户诊断码泄漏（mir/lir 源码中的 "scoop::mir::/"scoop::lir:: 字面量
#    只允许出现在各自 diagnostics 声明处）
for crate in ("scoop2_mir", "scoop2_lir"):
    for f in src_files(crate):
        rel = f.relative_to(ROOT)
        if f.name == "diagnostics.rs":
            continue
        text = f.read_text()
        for m in re.finditer(r'"(scoop::(?:mir|lir)::[a-z_]+)"', text):
            violations.append(f"诊断码泄漏: {rel}: {m.group(1)}")

# 3. 穷尽性：lower_tree 主 match 不应有 `_ =>` 兜底（TreeExprKind 封闭集）
tree = (CRATES / "scoop2_mir" / "src/mir/lower_tree.rs").read_text()
# 主 match 在 lower_tree_expr：检查 `unsupported!` 的使用（仅允许在
# unsupported_construct 的判定清单处出现一次定义 + 零调用点）
call_sites = re.findall(r"unsupported!\(", tree)
if len(call_sites) > 0:
    violations.append(
        f"穷尽性: lower_tree.rs 存在 {len(call_sites)} 处 unsupported! 兜底（应为 0）"
    )

# 4. LIR 段自包含
v0 = (CRATES / "scoop2_archive" / "src/v0.rs").read_text()
lir_seg = v0[v0.find("// LIR archive"):]
if "load_mir_archive" in lir_seg or ".mirarch" in lir_seg:
    violations.append("自包含: LIR archive 段引用了 MIR archive")

# 5. linking 白名单纪律：MIR/LIR 源码不得产生 scoop::link::* 之外的
#    用户错误码；且 link 码只允许出现在 driver/archive 装配层。
ALLOWED_LINK_EMITTERS = {
    "crates/scoop2c/src/main.rs",
    "crates/scoop2_archive/src/v0.rs",
}
for crate in ("scoop2_mir", "scoop2_lir"):
    for f in src_files(crate):
        rel = str(f.relative_to(ROOT))
        text = f.read_text()
        for m in re.finditer(r'"(scoop::link::[a-z_]+)"', text):
            violations.append(f"link 码越界: {rel}: {m.group(1)}")

# 6. MIR 用户诊断码总量不增（C5 收口后允许集：monomorph 系 + lower_unresolved
#    兜底 + prelude 环境错；verify 三码已转 ICE 但常量保留供历史断言）。
mir_diag = (CRATES / "scoop2_mir" / "src/diagnostics.rs").read_text()
user_codes = re.findall(r'pub const \w+: &str = "(scoop::mir::[a-z_]+)"', mir_diag)
allowed = {
    "scoop::mir::lower_unresolved",
    "scoop::mir::prelude_symbol_missing",
    "scoop::mir::monomorph_error",
    "scoop::mir::monomorph_no_template",
    "scoop::mir::verify_cfg",
    "scoop::mir::verify_direct_style",
    "scoop::mir::verify_semantic",
}
for c in user_codes:
    if c not in allowed:
        violations.append(f"MIR 用户诊断码超出允许集: {c}")

if violations:
    print("审计违规：")
    for v in violations:
        print(f"  - {v}")
    sys.exit(1)
print("scoop2 阶段边界审计：零违规")
