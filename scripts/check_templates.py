"""Builds every template in templates/ against this checkout.

The templates depend on the crates by git URL, because nothing is published yet. This script
copies each template to a scratch directory, points the git dependencies at the local crates,
fills in the project name, and runs `cargo check` (or `cargo test` for the headless template).
The wasm template is checked for the wasm32 target.

Usage: python scripts/check_templates.py [--fast]
  --fast  check only; skip the test run.
"""

import os
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GIT_URL = "https://github.com/tpt-solutions/tpt-window"
CRATES = {
    "tpt-window": "tpt-window",
    "tpt-window-core": "tpt-window-core",
    "tpt-window-wgpu": "tpt-window-wgpu",
    "tpt-window-test-utils": "tpt-window-test-utils",
}
DEP_LINE = re.compile(r'^(?P<name>[\w-]+) = \{ git = "' + re.escape(GIT_URL) + r'"(?P<rest>.*)\}$')


def rewrite_manifest(text: str, project: str) -> str:
    out = []
    for line in text.split("\n"):
        match = DEP_LINE.match(line)
        if match:
            crate = match.group("name")
            path = os.path.join(ROOT, CRATES[crate]).replace("\\", "/")
            line = f'{crate} = {{ path = "{path}"{match.group("rest")}}}'
        out.append(line.replace("{{project-name}}", project))
    return "\n".join(out)


def run(cmd, cwd, env):
    print("  $", " ".join(cmd), flush=True)
    return subprocess.call(cmd, cwd=cwd, env=env)


def check(template: str, fast: bool, scratch: str, env) -> bool:
    src = os.path.join(ROOT, "templates", template)
    dst = os.path.join(scratch, template)
    shutil.copytree(src, dst, ignore=shutil.ignore_patterns("cargo-generate.toml"))
    project = f"check-{template}"
    manifest = os.path.join(dst, "Cargo.toml")
    with open(manifest, encoding="utf-8") as f:
        text = f.read()
    with open(manifest, "w", encoding="utf-8") as f:
        f.write(rewrite_manifest(text, project))
    for root, _, files in os.walk(os.path.join(dst, "src")):
        for name in files:
            path = os.path.join(root, name)
            with open(path, encoding="utf-8") as f:
                text = f.read()
            with open(path, "w", encoding="utf-8") as f:
                f.write(text.replace("{{project-name}}", project))

    print(f"template {template}", flush=True)
    if template == "wasm-app":
        return run(["cargo", "check", "--target", "wasm32-unknown-unknown"], dst, env) == 0
    if template == "headless-test" and not fast:
        return run(["cargo", "test"], dst, env) == 0
    return run(["cargo", "check"], dst, env) == 0


def main() -> int:
    fast = "--fast" in sys.argv
    templates = sorted(
        d for d in os.listdir(os.path.join(ROOT, "templates"))
        if os.path.isfile(os.path.join(ROOT, "templates", d, "Cargo.toml"))
    )
    env = dict(os.environ)
    # One target directory for all templates, so the shared dependencies compile once.
    env["CARGO_TARGET_DIR"] = os.path.join(ROOT, "target", "templates")
    failed = []
    with tempfile.TemporaryDirectory(prefix="tpt-templates-") as scratch:
        for template in templates:
            if not check(template, fast, scratch, env):
                failed.append(template)
    if failed:
        print("FAILED:", ", ".join(failed))
        return 1
    print(f"all {len(templates)} templates build")
    return 0


if __name__ == "__main__":
    sys.exit(main())
