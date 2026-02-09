import subprocess
import json
import shutil
import sys


def copy_setting_file(src: str, dict: str):
    shutil.copy(src, dict)

def main(path: str):
    ext = ""
    if sys.platform.startswith("win"):
        ext = ".exe"
    elif sys.platform.startswith("linux"):
        ext = ".so"
    elif sys.platform.startswith("darwin"):
        print("macOS用の処理")
    else:
        print("その他OS")

    with open(f"{path}/build.json", encoding="utf-8") as f:
        for obj in json.load(f):
            result = subprocess.run(
                [
                    "g++",
                    "-std=c++17",
                    "-shared",
                    "-fPIC",
                    " ".join([f"{path}/" + file for file in obj["files"]]),
                    "-O2",
                    "-o",
                    f"{"extern_lib/std/" + obj["name"] + ext}",
                ],
                capture_output=True,
                text=True
            )
            if result.returncode != 0:
                print("コンパイル失敗")
                print(result.stderr)
            else:
                print("コンパイル成功")
                print(f"{path}/{obj["setting_file"]}", f"extern_lib/std/{obj["setting_file"]}")
                copy_setting_file(
                    f"{path}/{obj["setting_file"]}",
                    f"extern_lib/std/{obj["setting_file"]}"
                )


if __name__ == "__main__":
    main("std_lib")
