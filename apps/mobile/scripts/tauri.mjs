import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { loadEnvFile } from "node:process";
import { fileURLToPath } from "node:url";

const args = process.argv.slice(2);
const projectDir = join(dirname(fileURLToPath(import.meta.url)), "..");
const localEnv = join(projectDir, ".env");
if (existsSync(localEnv)) loadEnvFile(localEnv);
const env = { ...process.env };

if (!env.RUSTC) {
  const rustc = spawnSync("rustup", ["which", "rustc"], { encoding: "utf8" });
  if (rustc.status !== 0) throw new Error("Could not locate rustc through rustup.");
  env.RUSTC = rustc.stdout.trim();
}

if (args[0] === "android") {
  const sdkDir = env.ANDROID_HOME
    ?? env.ANDROID_SDK_ROOT
    ?? join(homedir(), process.platform === "darwin" ? "Library/Android/sdk" : "Android/Sdk");
  const ndkRoot = env.ANDROID_NDK_HOME
    ?? env.ANDROID_NDK_ROOT
    ?? env.NDK_HOME
    ?? newestDirectory(join(sdkDir, "ndk"));
  const host = process.platform === "darwin"
    ? "darwin-x86_64"
    : process.platform === "win32" ? "windows-x86_64" : "linux-x86_64";
  const extension = process.platform === "win32" ? ".exe" : "";
  const toolchain = join(ndkRoot, "toolchains/llvm/prebuilt", host, "bin");
  const ar = join(toolchain, `llvm-ar${extension}`);
  const ranlib = join(toolchain, `llvm-ranlib${extension}`);

  if (!existsSync(ar) || !existsSync(ranlib)) {
    throw new Error(`Could not locate the Android NDK archiver under ${toolchain}.`);
  }

  // libsodium's configure script otherwise picks the macOS archiver while
  // cross-compiling, producing an empty archive and a launch-time crash.
  env.AR = ar;
  env.RANLIB = ranlib;
}

const executable = join(
  projectDir,
  "node_modules/.bin",
  process.platform === "win32" ? "tauri.cmd" : "tauri",
);
const result = spawnSync(executable, args, { cwd: projectDir, env, stdio: "inherit" });
if (args[0] !== "ios" || args[1] !== "xcode-script") sanitizeAppleTeam();
if (result.error) throw result.error;
process.exit(result.status ?? 1);

function newestDirectory(parent) {
  if (!existsSync(parent)) throw new Error(`Android NDK directory does not exist: ${parent}`);
  const versions = readdirSync(parent, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort((left, right) => right.localeCompare(left, undefined, { numeric: true }));
  if (!versions.length) throw new Error(`No Android NDK is installed under ${parent}.`);
  return join(parent, versions[0]);
}

function sanitizeAppleTeam() {
  const project = join(
    projectDir,
    "src-tauri/gen/apple/volum-companion.xcodeproj/project.pbxproj",
  );
  if (!existsSync(project)) return;
  const source = readFileSync(project, "utf8");
  const sanitized = source.replace(
    /^\s*DEVELOPMENT_TEAM = .*;$/gm,
    '\t\t\t\tDEVELOPMENT_TEAM = "$(APPLE_DEVELOPMENT_TEAM)";',
  );
  if (sanitized !== source) writeFileSync(project, sanitized);
}
