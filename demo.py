# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "codeanim>=0.3.2",
# ]
# ///
import os
import subprocess
import tempfile
from os import makedirs
from shutil import rmtree

from codeanim import CodeAnim, Key


def touch(path: str):
    subprocess.call(["touch", path])


rmtree("demo", ignore_errors=True)

makedirs("demo/source/movie")
touch("demo/source/movie/Star.Trek.Generations.mkv")
makedirs("demo/source/show")
touch("demo/source/show/Sample.mkv")
touch("demo/source/show/Star.Trek.The.Next.Generation.S01E01.mkv")
touch("demo/source/show/Star.Trek.The.Next.Generation.S01E02.mkv")
touch("demo/source/show/Star.Trek.The.Next.Generation.S02E01.mkv")
touch("demo/source/show/Star.Trek.The.Next.Generation.S02E02.mkv")
makedirs("demo/target")

with CodeAnim() as ca:
    with tempfile.TemporaryDirectory() as zsh_home:
        with open(os.path.join(zsh_home, ".zshrc"), "w") as fp:
            fp.write('export PROMPT="$ "\n')

        with ca.delay(end=0.2):
            ca.shell.activate("iTerm2")
            ca.tap("n", modifiers=[Key.cmd])
            ca.shell.resize("iTerm2", (100, 100), (710, 513))

            demo_dir = os.path.join(os.path.dirname(os.path.realpath(__file__)), "demo")
            ca.paste(f"cd {demo_dir}")
            ca.tap(Key.enter)
            ca.paste(f"ZDOTDIR={zsh_home} termsvg rec demo.cast")
            ca.tap(Key.enter)

    with ca.delay(tap=0.05, end=2, keys={" ": 0.2}):
        ca.write("tree")
        with ca.delay(end=5):
            ca.tap(Key.enter)
        ca.write("mediar link source/show target/")
        ca.tap(Key.enter)
        ca.tap(Key.enter)
        ca.tap(Key.enter)
        ca.tap(Key.enter)
        with ca.delay(end=5):
            ca.tap(Key.enter)
        ca.write("tree")
        with ca.delay(end=10):
            ca.tap(Key.enter)
        ca.tap("d", modifiers=[Key.ctrl])
        ca.tap("d", modifiers=[Key.ctrl])


subprocess.call(["termsvg", "export", "demo/demo.cast", "-o", "demo.svg"])

with open("demo.svg", "r") as fp:
    lines = fp.readlines()

with open("demo.svg", "w") as fp:
    for line in lines:
        if line.startswith("@keyframes"):
            line = (
                line.replace("#00ff00", "#91e364")
                .replace("#00ffff", "#01d0df")
                .replace("#00cd00", "#91e364")
                .replace("#cdcd00", "#db8c44")
                .replace("#ffff00", "#db8c44")
                .replace("#0000ee", "#3d6d97")
                .replace("#5c5cff", "#17afff")
            )
        fp.write(line)
