"""Prepare QuickJS-NG 0.16.2 without changing the supplied source tree."""
import argparse
import hashlib
from pathlib import Path
import shutil

parser = argparse.ArgumentParser()
parser.add_argument("--source", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
if args.source.resolve() == args.output.resolve():
    parser.error("output must differ from the source directory")
header = (args.source / "cutils.h").read_bytes()
if hashlib.sha256(header).hexdigest() != "810709e35fe0ece09321ad4c207dba9973aa19f91fa909e00754896b39cc3201":
    raise SystemExit("Unsupported cutils.h: use QuickJS-NG d8e1cc68 (0.16.2)")
args.output.mkdir(parents=True, exist_ok=True)
for path in args.source.iterdir():
    if path.suffix in (".c", ".h") and path.name != "cutils.h":
        shutil.copy2(path, args.output / path.name)
text = header.decode()
include = "#if !defined(_WIN32) && !defined(EMSCRIPTEN) && !defined(__wasi__) && !defined(__DJGPP)"
threads = "#if defined(EMSCRIPTEN) || defined(__wasi__) || defined(__DJGPP)"
text = text.replace(include, include + " && !defined(POCKET_QJS_NO_THREADS)")
text = text.replace(threads, threads + " || defined(POCKET_QJS_NO_THREADS)")
output = args.output / "cutils.h"
if not output.exists() or output.read_text() != text:
    output.write_text(text)
print(args.output)
