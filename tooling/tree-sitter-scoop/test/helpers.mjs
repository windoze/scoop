import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { Language, Parser, Query } from "web-tree-sitter";

export const root = fileURLToPath(new URL("../", import.meta.url));
await Parser.init();
export const language = await Language.load(
  path.join(root, "tree-sitter-scoop.wasm"),
);
export const parser = new Parser().setLanguage(language);
export async function query(name) {
  return new Query(
    language,
    await readFile(path.join(root, "queries", `${name}.scm`), "utf8"),
  );
}

export async function* sources(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) yield* sources(file);
    else if (entry.name.endsWith(".scoop")) yield file;
  }
}

export function errors(tree) {
  const result = [];
  const cursor = tree.walk();
  for (;;) {
    const node = cursor.currentNode;
    if (node.isError || node.isMissing)
      result.push(
        `${node.type} at ${node.startPosition.row + 1}:${node.startPosition.column + 1}: ${node.text.slice(0, 100)}`,
      );
    if (cursor.gotoFirstChild()) continue;
    while (!cursor.gotoNextSibling()) {
      if (!cursor.gotoParent()) {
        cursor.delete();
        return result;
      }
    }
  }
}
