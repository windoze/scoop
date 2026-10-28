"""Inspect the published binding contexts and their real local page links."""

import sys
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit


class Page(HTMLParser):
    def __init__(self, path):
        super().__init__()
        self.links = []
        self.title = ""
        self.in_title = False
        self.feed(path.read_text())

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == "title":
            self.in_title = True
        for key in ("href", "src"):
            if key in attrs:
                self.links.append(attrs[key])

    def handle_endtag(self, tag):
        if tag == "title":
            self.in_title = False

    def handle_data(self, text):
        if self.in_title:
            self.title += text


root = Path(sys.argv[1]).resolve()
scopes = sorted((root / "r").iterdir())
assert len(scopes) == 3
home = Page(root / "index.html")
versions = []
shared_paths = []
for scope in scopes:
    assert f"r/{scope.name}/index.html" in home.links
    shared = []
    for page in scope.rglob("*.html"):
        parsed = Page(page)
        for link in parsed.links:
            url = urlsplit(link)
            if url.scheme or url.netloc:
                continue
            target = (page.parent / unquote(url.path)).resolve() if url.path else page
            assert target.is_file(), (page, link)
            assert target == root / "index.html" or target.is_relative_to(scope)
        if parsed.title.startswith("dev.pm:shared:1.0.0"):
            shared.append(page)
    assert len(shared) == 1
    shared_paths.append(shared[0])
    text = shared[0].read_text()
    versions.append(1 if "scoop:scoop.pm.value:1.0.0" in text else 2)
    assert f"scoop:scoop.pm.value:{versions[-1]}.0.0" in text
    assert f"scoop:scoop.pm.value:{3 - versions[-1]}.0.0" not in text
    index = (scope / "assets/index.js").read_text()
    assert "dev.pm:shared:1.0.0" in index
assert sorted(versions) == [1, 1, 2]
assert len(set(shared_paths)) == 3
print("Root bindings and local links verified.")
