"""Link the no_std fixture; this does not create flashable board firmware."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument("target", choices=["thumbv7em-none-eabihf", "riscv32imafc-unknown-none-elf"])
parser.add_argument("--cc", required=True)
args = parser.parse_args()
compiler = str(Path(shutil.which(args.cc) or args.cc).resolve())
flags = (["-mcpu=cortex-m4", "-mthumb", "-mfloat-abi=hard", "-mfpu=fpv4-sp-d16"]
         if args.target.startswith("thumb") else ["-march=rv32imafc", "-mabi=ilp32f"])
env = os.environ.copy()
suffix = args.target.replace("-", "_")
env["CC_" + suffix] = compiler
env["AR_" + suffix] = compiler.removesuffix("gcc") + "ar"
env["CFLAGS_" + suffix] = " ".join(flags)
paths = set()
for name in ["c", "m", "nosys", "gcc"]:
    library = Path(subprocess.check_output([compiler, *flags, "-print-file-name=lib" + name + ".a"], text=True).strip())
    if not library.is_file():
        raise SystemExit("Missing target library: " + str(library))
    paths.add(str(library.parent))
link = []
for path in sorted(paths):
    link += ["-C", "link-arg=-L" + path]
for argument in ["--entry=_start", "--start-group", "-lc", "-lm", "-lnosys", "-lgcc", "--end-group"]:
    link += ["-C", "link-arg=" + argument]
subprocess.run(["cargo", "rustc", "--manifest-path", str(Path(__file__).with_name("Cargo.toml")),
                "--target", args.target, "--release", "--locked", "--", *link], env=env, check=True)
