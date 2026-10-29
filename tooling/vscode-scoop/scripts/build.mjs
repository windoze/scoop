import { copyFile, mkdir, readdir, readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";

const root = fileURLToPath(new URL("../", import.meta.url));
const grammar = path.resolve(root, "../tree-sitter-scoop");
const runtimeEntry = createRequire(import.meta.url).resolve("web-tree-sitter");
const runtime = path.dirname(runtimeEntry);
const assets = path.join(root, "dist/assets");
await mkdir(path.join(root, "dist/debugger"), { recursive: true });
await copyFile(
  path.join(root, "debugger/scoop_lldb.py"),
  path.join(root, "dist/debugger/scoop_lldb.py"),
);
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

const licensed = new Set();
async function copyLicenses(require, name, entry = name) {
  let directory = path.dirname(require.resolve(entry));
  let manifest;
  while (true) {
    try {
      manifest = JSON.parse(
        await readFile(path.join(directory, "package.json"), "utf8"),
      );
      if (manifest.name === name) break;
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
    }
    const parent = path.dirname(directory);
    if (parent === directory) throw new Error(`Cannot locate ${name}`);
    directory = parent;
  }
  if (licensed.has(directory)) return;
  licensed.add(directory);
  const destination = path.join(
    root,
    "dist/licenses",
    `${name}-${manifest.version}`,
  );
  await mkdir(destination, { recursive: true });
  for (const file of await readdir(directory)) {
    if (/^(license|notice)(\.|$)/i.test(file)) {
      await copyFile(path.join(directory, file), path.join(destination, file));
    }
  }
  const dependencyRequire = createRequire(path.join(directory, "package.json"));
  for (const dependency of Object.keys(manifest.dependencies ?? {})) {
    await copyLicenses(dependencyRequire, dependency);
  }
}
await copyLicenses(
  createRequire(import.meta.url),
  "vscode-languageclient",
  "vscode-languageclient/node",
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
