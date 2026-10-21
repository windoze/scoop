import { copyFile, mkdir } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";

const root = fileURLToPath(new URL("../", import.meta.url));
const grammar = path.resolve(root, "../tree-sitter-scoop");
const runtimeEntry = createRequire(import.meta.url).resolve("web-tree-sitter");
const runtime = path.dirname(runtimeEntry);
const assets = path.join(root, "dist/assets");
await mkdir(assets, { recursive: true });
await mkdir(path.join(root, "dist/licenses"), { recursive: true });
await copyFile(
  path.join(grammar, "tree-sitter-scoop.wasm"),
  path.join(assets, "tree-sitter-scoop.wasm"),
);
await copyFile(
  path.join(runtime, "web-tree-sitter.wasm"),
  path.join(assets, "web-tree-sitter.wasm"),
);
for (const query of ["highlights", "folds", "indents"]) {
  await copyFile(
    path.join(grammar, "queries", `${query}.scm`),
    path.join(assets, `${query}.scm`),
  );
}
await copyFile(
  path.join(runtime, "LICENSE"),
  path.join(root, "dist/licenses/web-tree-sitter.txt"),
);
await build({
  entryPoints: [path.join(root, "src/extension.ts")],
  outfile: path.join(root, "dist/extension.cjs"),
  bundle: true,
  platform: "node",
  format: "cjs",
  target: "node20",
  external: ["vscode"],
  // Use the runtime's Node entry so its loader retains CommonJS path handling.
  alias: { "web-tree-sitter": runtimeEntry },
  legalComments: "external",
  sourcemap: true,
});
