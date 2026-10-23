"""An override family and shared property parameters use one source rename."""

from rename_support import RenameWorkspace, manifest


def check(peer, root):
    project = root / "rename-members"
    manifest(project, "rename-members")
    file = "src/members.scoop"
    case = RenameWorkspace(
        peer,
        project,
        {
            file: (
                "package renamemembers\n"
                "interface Protocol<T> { public fun choose(input: T): T\n"
                "public val flag: Boolean }\n"
                "open class Base<T>(public override val flag: Boolean) : Protocol<T> {\n"
                "    public open override fun choose(value: T): T = value\n"
                "}\n"
                "class Child(public val payload: Boolean) : Base<Boolean>(flag = payload) {\n"
                "    public override fun choose(child: Boolean): Boolean = child\n"
                "}\n"
                "fun call(value: Protocol<Boolean>): Boolean = value.choose(value.flag)\n"
                "fun bound(value: Child): (Boolean) -> Boolean = value::choose\n"
            )
        },
    )
    case.clean()
    case.rename(file, "choose(child", "choose", "select", 5)
    case.rename(file, "value.flag", "flag", "enabled", 4)
    case.rename(file, "child: Boolean", "child", "answer", 2)
    case.rename(file, "public val payload", "payload", "input", 2)
    case.finish()
