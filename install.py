import os
import sys
import platform
import subprocess


def setup_path():
    # 1. 自身のスクリプトがある絶対パス（プロジェクトのルート）を取得
    project_root = os.path.dirname(os.path.abspath(__file__))
    
    # 2. バイナリとディレクトリが揃っているフォルダ（ターゲット）を特定
    # Windowsの場合は hexl.exe、Linux/Macの場合は hexl を探す
    is_windows = platform.system() == "Windows"
    bin_name = "hexl.exe" if is_windows else "hexl"
    
    release_dir = os.path.join(project_root, "target", "release")
    
    if os.path.exists(os.path.join(release_dir, bin_name)):
        target_dir = release_dir
    elif os.path.exists(os.path.join(project_root, bin_name)):
        target_dir = project_root
    else:
        print(f"❌ {bin_name} が見つかりません。先に 'cargo build --release' を実行してください。")
        sys.exit(1)

    # library と asm_fmt が存在するか確認（なければ警告）
    if not os.path.exists(os.path.join(target_dir, "library")) or not os.path.exists(os.path.join(target_dir, "asm_fmts")):
        print(f"⚠️ 警告: {target_dir} 内に 'library' または 'asm_fmt' ディレクトリが見つかりません。")
        print("   プログラムが正常に動作しない可能性があります。")

    print(f"🔍 ターゲットディレクトリを検出: {target_dir}")

    # 3. OSごとのPATH登録処理
    if is_windows:
        try:
            # ユーザー環境変数（User PATH）を取得
            import winreg
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, r"Environment", 0, winreg.KEY_READ) as key:
                current_path, _ = winreg.QueryValueEx(key, "Path")
        except Exception:
            current_path = ""

        # すでに登録済みかチェック
        if target_dir in current_path:
            print("✅ すでに Windows の PATH に登録されています。")
            return

        # setx コマンドを使ってユーザーの PATH に永続追加
        # ※PATH全体の文字数制限（1024文字）に引っかからないよう注意
        new_path = f"{current_path};{target_dir}".strip(";")
        try:
            subprocess.run(["setx", "Path", new_path], check=True, stdout=subprocess.DEVNULL)
            print("✅ Windows の環境変数 PATH に追加しました。")
            print("🚀 反映させるため、新しくコマンドプロンプトや PowerShell を開き直してください。")
        except subprocess.CalledProcessError:
            print("❌ PATH の追加に失敗しました。管理者権限が必要な場合があります。")
            
    else:
        # Linux / macOS (Unix系)
        # 使用中のシェル設定ファイルを特定 (~/.bashrc または ~/.zshrc)
        shell = os.environ.get("SHELL", "")
        home = os.path.expanduser("~")
        shell_rc = None

        if "zsh" in shell:
            shell_rc = os.path.join(home, ".zshrc")
        elif "bash" in shell or os.path.exists(os.path.join(home, ".bashrc")):
            shell_rc = os.path.join(home, ".bashrc")

        if shell_rc and os.path.exists(shell_rc):
            # すでに登録済みかチェック
            with open(shell_rc, "r") as f:
                content = f.read()

            if target_dir in content:
                print(f"✅ すでに {shell_rc} にパスが登録されています。")
            else:
                # シェル設定ファイルの末尾に PATH を追記
                with open(shell_rc, "a") as f:
                    f.write(f"\n# hexl CLI and assets path\n")
                    f.write(f'export PATH="{target_dir}:$PATH"\n')
                print(f"✅ {shell_rc} に PATH を追加しました。")
            
            print("🚀 以下のコマンドを実行するか、新しいターミナルを開くと 'hexl' がどこでも使えます！")
            print(f"   source {shell_rc}")
        else:
            print("⚠️ シェルの設定ファイルが見つかりませんでした。手動で以下のパスを PATH 環境変数に追加してください：")
            print(f'   export PATH="{target_dir}:$PATH"')

if __name__ == "__main__":
    setup_path()
