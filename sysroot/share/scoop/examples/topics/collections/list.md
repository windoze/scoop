## 运行

在保存文件的目录执行：

```sh
scoop run --release main.scoop
```

## 预期结果

程序 stdout：

```text
snapshot: Ada
snapshot: Lin
current first: Grace
sizes=2,3
```

## 关键说明

ArrayList 用于动态增删；`toArray()` 取得只读 Array 快照。之后替换列表第一项、追加 Sam，都不改变快照的元素序列或大小。容器 size 与索引使用 Long，首项写 `[0L]`。

val 只保证 names 的 binding 不重绑定，不禁止修改其引用容器。Array 本身不提供元素写入；需要可变固定长度存储时选择 MutableArray。toArray 是浅快照，若元素引用可变对象，快照与列表仍可引用同一对象。

## 常见误用

不要把 val 当作深度不可变保证，也不要将 toArray 解释为递归复制所有对象。希望独立修改元素对象时，要按业务规则显式复制元素。
