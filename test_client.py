import os
import shutil
import subprocess
import sys

def copy_dir_all(src, dst):
    if os.path.exists(dst):
        shutil.rmtree(dst)
    shutil.copytree(src, dst)

def find_file(directory, filename):
    for root, _, files in os.walk(directory):
        if filename in files:
            return os.path.join(root, filename)
    return None

def main(target_dir, client_id, build):
    if build:
        if os.path.exists(target_dir):
            shutil.rmtree(target_dir)

        os.makedirs(target_dir)

        # Copy target/debug recursively
        debug_src = "target/debug"
        debug_dst = os.path.join(target_dir, "debug")
        copy_dir_all(debug_src, debug_dst)

        # Copy assets recursively
        assets_src = "assets"
        assets_dst = os.path.join(debug_dst, "assets")
        copy_dir_all(assets_src, assets_dst)

        # Find steam_api64.dll and copy it
        steam_api_path = find_file("target/debug/build", "steam_api64.dll")
        if steam_api_path:
            steam_api_dst = os.path.join(debug_dst, "steam_api64.dll")
            shutil.copy(steam_api_path, steam_api_dst)

    hello2_dir = os.path.join(target_dir, "debug")
    # Look for hello2.exe in the created directory and run it with commands
    if os.path.exists(hello2_dir):
        os.chdir(hello2_dir)
        try:
            exe = subprocess.Popen(["hello2.exe", "client", "-c", client_id])
            exe.wait()
            if exe.returncode != 0:
                print("hello2.exe execution failed", file=sys.stderr)
                sys.exit(1)
        except Exception as e:
            print(f"Failed to execute hello2.exe: {e}", file=sys.stderr)
            sys.exit(1)
    else:
        print(f"hello2.exe not found in {hello2_dir}", file=sys.stderr)

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python test_client.py <client_id> <target_dir (false)>")
        sys.exit(1)
    client_id = sys.argv[1]
    build = sys.argv[2] == "true" if len(sys.argv) > 2 else False
    target_dir = f"target/client_{client_id}"

    main(target_dir, client_id, build)