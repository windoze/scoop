import {
  access,
  mkdir,
  mkdtemp,
  realpath,
  rm,
  writeFile,
} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { runTests } from "@vscode/test-electron";
import { build } from "esbuild";

const root = fileURLToPath(new URL("../", import.meta.url));
const repository = path.resolve(root, "../..");
const server = path.resolve(
  process.env.SCOOP_TEST_LSP ??
    path.join(
      repository,
      "target/release",
      process.platform === "win32" ? "scoop-lsp.exe" : "scoop-lsp",
    ),
);
const sysroot = path.resolve(
  process.env.SCOOP_SYSROOT ?? path.join(repository, "sysroot"),
);
await access(server);
await access(path.join(sysroot, "lib"));
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

const temporary = await mkdtemp(
  // macOS's per-user temporary path can exceed Electron's Unix socket limit.
  path.join(
    await realpath(process.platform === "darwin" ? "/tmp" : os.tmpdir()),
    "scoop-vscode-test-",
  ),
);
const workspace = path.join(temporary, "workspace");
await mkdir(path.join(workspace, ".vscode"), { recursive: true });
await writeFile(
  path.join(workspace, ".vscode/settings.json"),
  JSON.stringify({
    "scoop.server.enabled": false,
    "scoop.server.path": server,
    "scoop.sysroot": sysroot,
    "scoop.trace.server": "verbose",
  }),
);
let succeeded = false;
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
      "--log",
      "trace",
      "--disable-workspace-trust",
      "--skip-welcome",
      "--skip-release-notes",
    ],
  });
  succeeded = true;
} finally {
  if (succeeded) await rm(temporary, { recursive: true, force: true });
  else console.error(`Extension host test logs and workspace: ${temporary}`);
}
