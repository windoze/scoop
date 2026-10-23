"""Extern checks preserve independent facts after signatures have been registered."""

from diagnostics import published
from rename_support import RenameWorkspace, manifest


def check(peer, root):
    project = root / "late-declarations"
    manifest(project, "late-declarations")
    file = "src/externs.scoop"
    consumer = "src/use.scoop"
    independent = "fun independent(value: Boolean): Boolean = value\n"
    original = '@Extern(name = "late_bad", captureErrno = true)\nfun broken(): Int\n' + independent
    case = RenameWorkspace(
        peer,
        project,
        {file: original, consumer: "fun use(): Boolean = independent(true)\n"},
    )

    def available():
        assert case.query(file, "fun independent", "independent", "hover") is not None
        result = case.query(consumer, "independent(true)", "independent", "definition")
        assert result["uri"] == case.paths[file].as_uri(), result
        assert published(peer, case.paths[consumer].as_uri(), 1) == []
        result = case.query(
            file,
            "fun independent",
            "independent",
            "references",
            context={"includeDeclaration": True},
        )
        assert result is not None and len(result) == 2, result

    diagnostics = published(peer, case.paths[file].as_uri(), 1)
    assert any("must return `(R, Int)`" in item["message"] for item in diagnostics), diagnostics
    assert case.query(file, "fun broken", "broken", "hover") is None
    available()
    case.query(file, "fun independent", "independent", "prepareRename", error=-32803)

    case.change(file, original.replace("broken(): Int", "broken(): (Int, Int)"))
    case.clean()
    assert case.query(file, "fun broken", "broken", "hover") is not None
    available()

    variable = '@Extern(name = "late_bad", captureErrno = true)\nval broken: Int\n' + independent
    case.change(file, variable)
    diagnostics = published(peer, case.paths[file].as_uri(), case.versions[file])
    assert any("not allowed on an extern variable" in item["message"] for item in diagnostics), (
        diagnostics
    )
    assert case.query(file, "val broken", "broken", "hover") is None
    available()
    case.change(file, variable.replace("captureErrno = true", "captureErrno = false"))
    case.clean()
    assert case.query(file, "val broken", "broken", "hover") is not None
    available()
    case.close(file)
    assert case.query(file, "fun broken", "broken", "hover") is None
    available()
    case.finish()
