#!/usr/bin/env python3
"""Builds a large git repository to try diffv's performance by hand.

Usage: make_fixture.py <dir> [--no-minified]

- 3000 tracked files with 3 changed lines each (some staged)
- big.rs: 100k lines, every line changed
- bundle.min.js: one 3.6 MB line, changed (skipped with --no-minified)
- 300 untracked files of 2000 lines
"""
import os
import subprocess
import sys

args = [a for a in sys.argv[1:] if not a.startswith("--")]
if len(args) != 1:
    sys.exit(__doc__)
root = args[0]
minified = "--no-minified" not in sys.argv
if os.path.exists(os.path.join(root, ".git")):
    sys.exit(f"{root} already exists; remove it first (make fixture-clean)")
os.makedirs(root, exist_ok=True)


def git(*a):
    subprocess.run(["git", *a], cwd=root, check=True, capture_output=True)


def write(path, text):
    with open(os.path.join(root, path), "w") as f:
        f.write(text)


git("init", "-q")
git("config", "user.name", "diffv-fixture")
git("config", "user.email", "fixture@example.com")

N = 3000
paths = []
for i in range(N):
    d = f"src/mod_{i // 100}/sub_{i % 7}"
    os.makedirs(os.path.join(root, d), exist_ok=True)
    p = f"{d}/file_{i}.rs"
    paths.append(p)
    write(p, "".join(f"    let value_{j} = compute({i}, {j}); // line {j}\n" for j in range(300)))
big = "".join(f"fn item_{j}() -> usize {{ {j} * 2 }}\n" for j in range(100_000))
write("big.rs", big)
mini = "var a=" + ",".join(f"x{j}:{j}" for j in range(400_000)) + ";\n"
if minified:
    write("bundle.min.js", mini)
git("add", ".")
git("commit", "-qm", "base")

for p in paths:
    lines = open(os.path.join(root, p)).read().splitlines(True)
    for j in (10, 150, 290):
        lines[j] = lines[j].replace("compute", "compute_v2")
    write(p, "".join(lines))
write("big.rs", big.replace("* 2", "* 3"))
if minified:
    write("bundle.min.js", mini.replace("x1", "y1"))
git("add", "src/mod_1")
os.makedirs(os.path.join(root, "untracked"), exist_ok=True)
for i in range(300):
    write(f"untracked/new_{i}.txt", "".join(f"new line {j}\n" for j in range(2000)))
# Refresh the index so timings don't include git re-hashing every file.
git("status", "--porcelain")
print(f"fixture ready: {root}")
