"use strict";
(() => {
  const input = document.getElementById("search-input");
  const results = document.getElementById("search-results");
  const status = document.getElementById("search-status");
  if (!input || !results || !status) return;
  const normalize = text => text.normalize("NFKC").toLocaleLowerCase();
  const entries = (globalThis.SCOOP_DOC_SEARCH || []).map(entry => ({
    ...entry,
    normalizedName: normalize(entry.name),
    normalizedAliases: entry.aliases.map(normalize),
    searchable: normalize([entry.name, entry.kind, entry.scope, entry.summary, ...entry.aliases].join(" "))
  }));

  function render() {
    const query = normalize(input.value.trim());
    const terms = query.split(/\s+/).filter(Boolean);
    results.replaceChildren();
    if (!terms.length) {
      status.textContent = "Search by name, kind, summary, package, Cone, or alias.";
      return;
    }
    const matches = entries.filter(entry => terms.every(term => entry.searchable.includes(term)));
    const rank = entry => entry.normalizedName === query ? 0
      : entry.normalizedAliases.includes(query) ? 1
      : entry.normalizedName.startsWith(query) ? 2 : 3;
    matches.sort((a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name) || a.scope.localeCompare(b.scope));
    status.textContent = `${matches.length} result${matches.length === 1 ? "" : "s"} for “${input.value.trim()}”`;
    const fragment = document.createDocumentFragment();
    for (const entry of matches) {
      const item = document.createElement("li");
      const link = document.createElement("a");
      link.href = entry.url;
      link.textContent = entry.name;
      const kind = document.createElement("span");
      kind.className = "kind";
      kind.textContent = entry.kind;
      const scope = document.createElement("small");
      scope.textContent = entry.scope;
      item.append(kind, " ", link, scope);
      if (entry.summary) {
        const summary = document.createElement("p");
        summary.textContent = entry.summary;
        item.append(summary);
      }
      fragment.append(item);
    }
    results.append(fragment);
  }
  input.value = new URLSearchParams(location.search).get("q") || "";
  input.addEventListener("input", render);
  input.form.addEventListener("submit", event => {
    event.preventDefault();
    render();
  });
  render();
})();
