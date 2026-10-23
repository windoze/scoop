"""Apply actual versioned rename edits and inspect the updated language service."""

from diagnostics import change, published


def manifest(project, name, dependencies=()):
    project.mkdir(parents=True, exist_ok=True)
    text = (
        f'schema = 1\n[cone]\ngroup = "test"\nname = "{name}"\n'
        'version = "1.0.0"\nkind = "library"\n[dependencies]\n'
    )
    for dependency, path in dependencies:
        text += f'"test:{dependency}" = {{version = "1.0.0", path = "{path}"}}\n'
    (project / "Cone.toml").write_text(text)


class RenameWorkspace:
    def __init__(self, peer, project, texts):
        self.peer = peer
        self.texts = texts
        self.versions = dict.fromkeys(texts, 1)
        self.paths = {name: project / name for name in texts}
        self.folder = {"name": project.name, "uri": project.as_uri()}
        for name, path in self.paths.items():
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(texts[name])
        peer.notify(
            "workspace/didChangeWorkspaceFolders",
            {"event": {"added": [self.folder], "removed": []}},
        )
        for name, path in self.paths.items():
            peer.notify(
                "textDocument/didOpen",
                {
                    "textDocument": {
                        "uri": path.as_uri(),
                        "languageId": "scoop",
                        "version": self.versions[name],
                        "text": texts[name],
                    }
                },
            )

    def position(self, file, needle, name):
        text = self.texts[file]
        prefix = text[: text.index(needle) + needle.index(name)]
        return {
            "line": prefix.count("\n"),
            "character": len(prefix.rsplit("\n", 1)[-1].encode("utf-16-le")) // 2,
        }

    def query(self, file, needle, name, method, error=None, **extra):
        request = self.peer.send_request(
            f"textDocument/{method}",
            {
                "textDocument": {"uri": self.paths[file].as_uri()},
                "position": self.position(file, needle, name),
                **extra,
            },
        )
        return self.peer.response(request, error_code=error)

    def clean(self):
        for name, path in self.paths.items():
            if self.versions[name] is not None:
                diagnostics = published(self.peer, path.as_uri(), self.versions[name])
                assert diagnostics == [], (name, diagnostics)

    def change(self, name, text):
        self.texts[name] = text
        if self.versions[name] is None:
            self.paths[name].write_text(text)
            self.peer.notify(
                "workspace/didChangeWatchedFiles",
                {"changes": [{"uri": self.paths[name].as_uri(), "type": 2}]},
            )
        else:
            self.versions[name] += 1
            change(self.peer, self.paths[name].as_uri(), self.versions[name], text)

    def apply(self, result, count):
        documents = result["documentChanges"]
        assert sum(len(document["edits"]) for document in documents) == count, result
        updated = {}
        for document in documents:
            identifier = document["textDocument"]
            name = next(
                name for name, path in self.paths.items() if path.as_uri() == identifier["uri"]
            )
            assert identifier["version"] == self.versions[name], result
            text = self.texts[name]
            lines = text.splitlines(keepends=True)

            def offset(position, lines=lines):
                line = position["line"]
                prefix = (
                    lines[line].encode("utf-16-le")[: position["character"] * 2].decode("utf-16-le")
                )
                return len("".join(lines[:line])) + len(prefix)

            edits = [
                (offset(edit["range"]["start"]), offset(edit["range"]["end"]), edit["newText"])
                for edit in document["edits"]
            ]
            for start, end, replacement in sorted(edits, reverse=True):
                text = text[:start] + replacement + text[end:]
            updated[name] = text
        for name, text in updated.items():
            self.change(name, text)
        self.clean()

    def rename(self, file, needle, name, replacement, count):
        result = self.query(file, needle, name, "rename", newName=replacement)
        assert result is not None, (file, needle)
        self.apply(result, count)

    def close(self, name):
        self.peer.notify(
            "textDocument/didClose", {"textDocument": {"uri": self.paths[name].as_uri()}}
        )
        self.versions[name] = None
        self.texts[name] = self.paths[name].read_text()

    def finish(self):
        for name in self.paths:
            if self.versions[name] is not None:
                self.close(name)
        self.peer.notify(
            "workspace/didChangeWorkspaceFolders",
            {"event": {"added": [], "removed": [self.folder]}},
        )
