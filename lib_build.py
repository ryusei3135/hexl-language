# std_lib/にある
# c言語などでできたライブライブをコンパイルし移動する

import subprocess
import json
import shutil
import sys
from pathlib import Path


def copy_setting_file(src: str, dst: str):
    shutil.copy(src, dst)


def main(path: str):
    out_dir = Path("extern_lib/std")
    out_dir.mkdir(parents=True, exist_ok=True)

    if sys.platform.startswith("win"):
        ext = ".dll"
    elif sys.platform.startswith("linux"):
        ext = ".so"
    elif sys.platform.startswith("darwin"):
        ext = ".dylib"
    else:
        raise RuntimeError("未対応のOSです")

    with open(f"{path}/build.json", encoding="utf-8") as f:
        for obj in json.load(f):
            output = out_dir / (obj["name"] + ext)

            files = [f"{path}/{file}" for file in obj["files"]]

            result = subprocess.run(
                [
                    "clang++",
                    "-std=c++17",
                    "-shared",
                    "-fPIC",
                    *files,          # ← ここが重要
                    "-O2",
                    "-o",
                    str(output),
                ],
                capture_output=True,
                text=True
            )

            if result.returncode != 0:
                print("コンパイル失敗")
                print(result.stderr)
            else:
                print("コンパイル成功")

                src_setting = f'{path}/{obj["setting_file"]}'
                dst_setting = out_dir / obj["setting_file"]

                print(src_setting, dst_setting)
                copy_setting_file(src_setting, dst_setting)

main("std_lib")
