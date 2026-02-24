#!/usr/bin/env python3
import subprocess
import sys
import os
import shlex

def run_command(command, description):
    print(f"==> {description}...")
    try:
        if isinstance(command, str):
            command = shlex.split(command)
        subprocess.check_call(command)
    except subprocess.CalledProcessError as e:
        print(f"Error during {description}: {e}")
        return False
    except FileNotFoundError:
        print(f"Error: Command not found: {command[0] if isinstance(command, list) else command}")
        return False
    return True

def main():
    # Ensure we are in the project root
    script_dir = os.path.dirname(os.path.realpath(__file__))
    project_root = os.path.abspath(os.path.join(script_dir, ".."))
    os.chdir(project_root)

    print("Generating Release Builds for Genteel...")

    # Build for Linux (Native)
    if not run_command(["cargo", "build", "--release"], "Building for Linux (Native)"):
        sys.exit(1)

    # Build for Windows (Cross-compile)
    # This requires x86_64-pc-windows-gnu target to be installed
    # rustup target add x86_64-pc-windows-gnu
    # and a cross-linker (like mingw-w64)
    if not run_command(["cargo", "build", "--release", "--target", "x86_64-pc-windows-gnu"], "Building for Windows (Cross-compile)"):
        print("\nWarning: Windows build failed. Ensure x86_64-pc-windows-gnu target and mingw-w64 are installed.")
        print("To install target: rustup target add x86_64-pc-windows-gnu")
    else:
        print("\nWindows build successful!")

    print("\nBuilds complete!")
    print("Linux: target/release/genteel")
    print("Windows: target/x86_64-pc-windows-gnu/release/genteel.exe")

if __name__ == "__main__":
    main()
