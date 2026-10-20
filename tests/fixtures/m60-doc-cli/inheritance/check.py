"""Inspect the site through its public search index and rendered HTML."""

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
        self.text = []
        self.feed(path.read_text())

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if "id" in attrs:
            assert attrs["id"] not in self.ids, attrs["id"]
            self.ids.add(attrs["id"])
        for attribute in ("href", "src"):
            if attribute in attrs:
                self.links.append(attrs[attribute])

    def handle_data(self, data):
        self.text.append(data)


root = Path(sys.argv[1]).resolve()
mode = sys.argv[2]
index = json.loads(
    (root / "assets/index.js")
    .read_text()
    .removeprefix("globalThis.SCOOP_DOC_SEARCH=")
    .strip()
    .removesuffix(";")
)
pages = {path.resolve(): Page(path) for path in root.rglob("*.html")}


def page_for(name):
    matches = [row for row in index if row["name"] == name and row["kind"] != "re-export"]
    assert len(matches) == 1, matches
    return pages[(root / urlsplit(matches[0]["url"]).path).resolve()]


child = page_for("Child")
text = "".join(child.text)
for signature in (
    "fun copy(input: Int): Int",
    "fun other<U>(input: U, fallback: Int): U",
    "fun constrained<U>(input: U): Int where requires(x: U) { x.read(): Int }",
    "fun defaulted(value: Int = …): Int",
    "Base<Int>",
    "Overrides",
):
    assert signature in text, signature
assert "secret" not in text
diamond = "".join(page_for("Diamond").text)
assert diamond.count("fun read(value: Int): Int") == 1, diamond
assert any(
    "public class Child" in "".join(page.text)
    for path, page in pages.items()
    if path.parent.name == "source"
)
if mode == "no_deps":
    assert "no generated documentation" in text
else:
    assert any(row["name"] == "VisibleBase" and row["kind"] == "re-export" for row in index)
    origin = next(
        row["url"] for row in index if row["name"] == "copy" and row["kind"] == "function"
    )
    assert any(link.endswith(urlsplit(origin).fragment) for link in child.links)
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
print("Inheritance, signatures, source text and local links verified.")
