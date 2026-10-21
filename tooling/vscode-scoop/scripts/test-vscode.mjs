import { mkdir, mkdtemp, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { runTests } from "@vscode/test-electron";
import { build } from "esbuild";

const root = fileURLToPath(new URL("../", import.meta.url));
const extensionTestsPath = path.join(root, "out/test/integration.cjs");
await build({
  entryPoints: [path.join(root, "test/host/integration.ts")],
  outfile: extensionTestsPath,
  bundle: true,
  platform: "node",
  format: "cjs",
  target: "node20",
  external: ["vscode"],
});

const temporary = await mkdtemp(path.join(os.tmpdir(), "scoop-vscode-test-"));
const workspace = path.join(temporary, "workspace");
await mkdir(workspace);
try {
  await runTests({
    extensionDevelopmentPath: root,
    extensionTestsPath,
    vscodeExecutablePath: process.env.VSCODE_EXECUTABLE_PATH,
    version: process.env.VSCODE_VERSION,
    launchArgs: [
      workspace,
      "--user-data-dir",
      path.join(temporary, "profile"),
      "--extensions-dir",
      path.join(temporary, "extensions"),
      "--disable-extensions",
      "--disable-workspace-trust",
      "--skip-welcome",
      "--skip-release-notes",
    ],
  });
} finally {
  await rm(temporary, { recursive: true, force: true });
}
