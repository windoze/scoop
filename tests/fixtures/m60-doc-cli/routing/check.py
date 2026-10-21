"""Check version routing using public pages and their search entries."""

import json
import sys
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit


class Page(HTMLParser):
    def __init__(self, path):
        super().__init__()
        self.ids = set()
        self.links = []
        self.feed(path.read_text())

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if "id" in attrs:
            assert attrs["id"] not in self.ids, attrs["id"]
            self.ids.add(attrs["id"])
        for key in ("href", "src"):
            if key in attrs:
                self.links.append(attrs[key])


root = Path(sys.argv[1]).resolve()
index = json.loads(
    (root / "assets/index.js")
    .read_text()
    .removeprefix("globalThis.SCOOP_DOC_SEARCH=")
    .strip()
    .removesuffix(";")
)
pages = {path.resolve(): Page(path) for path in root.rglob("*.html")}
versions = [row for row in index if row["name"] == "Versioned" and row["kind"] == "class"]
assert len(versions) == 2, versions
first = next(row for row in versions if "1.0.0" in row["scope"])
second = next(row for row in versions if "2.0.0" in row["scope"])
assert first["url"] != second["url"], versions


def address(url):
    parts = urlsplit(url)
    return (root / unquote(parts.path)).resolve(), unquote(parts.fragment)


for helper, expected, other in (("firstHelper", first, second), ("secondHelper", second, first)):
    item = next(row for row in index if row["name"] == helper)
    path, _ = address(item["url"])
    links = set()
    for link in pages[path].links:
        url = urlsplit(link)
        if not url.scheme and not url.netloc:
            links.add(((path.parent / unquote(url.path)).resolve(), unquote(url.fragment)))
    assert address(expected["url"]) in links, (helper, links)
    assert address(other["url"]) not in links, (helper, links)
for name in ("LeftVersion", "RightVersion", "ThroughLeft", "ThroughRight"):
    binding = next(row for row in index if row["name"] == name and row["kind"] == "re-export")
    assert binding["url"] == first["url"], binding
for path, page in pages.items():
    for link in page.links:
        url = urlsplit(link)
        assert url.scheme != "file", link
        if url.scheme or url.netloc:
            continue
        target = (path.parent / unquote(url.path)).resolve() if url.path else path
        assert target.is_file(), (path, link)
        if url.fragment and target in pages:
            assert unquote(url.fragment) in pages[target].ids, (path, link)
if sys.argv[2:] == ["resources"]:
    resources = list(root.glob("c/*/resources/*.txt"))
    assert len(resources) == 1, resources
    assert resources[0].read_text() == "Guide revision two.\n"
print("Versions, diamond reexports and local links verified.")
