# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "codeanim>=0.2.1",
# ]
# ///
import os
import subprocess
import tempfile
from os import makedirs
from shutil import rmtree

from codeanim import Key
from codeanim import codeanim as ca


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


with tempfile.TemporaryDirectory() as zsh_home:
    with open(os.path.join(zsh_home, ".zshrc"), "w") as fp:
        fp.write('export PROMPT="$ "\n')

    ca.shell.activate("iTerm2")
    ca.tap("n", modifiers=[Key.cmd])
    ca.shell.resize("iTerm2", (100, 100), (710, 513))

    ca.paste("cd ~/Developer/mediar/demo")
    ca.tap(Key.enter)
    ca.paste(f"ZDOTDIR={zsh_home} termsvg rec demo.cast")
    ca.tap(Key.enter)


ca.delay.set(tap=0.05, end=1)
ca.write("tree")
ca.tap(Key.enter)
ca.delay.pause(5)
ca.write("mediar link source/show target/")
ca.tap(Key.enter)
ca.delay.pause(1)
ca.tap(Key.enter)
ca.delay.pause(1)
ca.tap(Key.enter)
ca.delay.pause(1)
ca.tap(Key.enter)
ca.delay.pause(1)
ca.tap(Key.enter)
ca.delay.pause(5)
ca.write("tree")
ca.tap(Key.enter)
ca.delay.pause(10)
ca.tap("d", modifiers=[Key.ctrl])
ca.tap("d", modifiers=[Key.ctrl])


subprocess.call(["termsvg", "export", "demo/demo.cast", "-o", "demo.svg"])
