#!/usr/bin/env python3
import argparse
import subprocess
import sys
import os
import shlex

def run_command(command, description):
    print(f"==> {description}...")
    try:
        if isinstance(command, str):
            command = shlex.split(command)
        subprocess.check_call(command, shell=False)
    except subprocess.CalledProcessError as e:
        print(f"Error during {description}: {e}")
        return False
    except FileNotFoundError:
        print(f"Error: Command not found: {command[0] if isinstance(command, list) else command}")
        return False
    return True

def main():
    parser = argparse.ArgumentParser(description="Generate Linux and Windows builds for Genteel.")
    parser.add_argument("--debug", action="store_true", help="Generate debug builds (default is release)")
    args = parser.parse_args()

    # Ensure we are in the project root
    script_dir = os.path.dirname(os.path.realpath(__file__))
    project_root = os.path.abspath(os.path.join(script_dir, ".."))
    os.chdir(project_root)

    mode = "debug" if args.debug else "release"
    cargo_args = [] if args.debug else ["--release"]

    print(f"Generating {mode.capitalize()} Builds for Genteel...")

    # Build for Linux (Native)
    linux_desc = f"Building for Linux (Native, {mode})"
    if not run_command(["cargo", "build"] + cargo_args, linux_desc):
        sys.exit(1)

    # Build for Windows (using cargo-xwin)
    # Requires cargo-xwin to be installed (cargo install cargo-xwin)
    windows_desc = f"Building for Windows (cargo-xwin, {mode})"
    windows_cmd = ["cargo", "xwin", "build", "--target", "x86_64-pc-windows-msvc"] + cargo_args
    
    if not run_command(windows_cmd, windows_desc):
        print("\nWarning: Windows build failed. Ensure cargo-xwin is installed.")
        print("To install: cargo install cargo-xwin")
    else:
        print(f"\nWindows {mode} build successful!")

    print(f"\n{mode.capitalize()} builds complete!")
    print(f"Linux: target/{mode}/genteel")
    print(f"Windows: target/x86_64-pc-windows-msvc/{mode}/genteel.exe")

if __name__ == "__main__":
    main()
