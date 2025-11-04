import subprocess
import os
import shutil
import sys

#Needs
#pip install twine

def get_platform_lib_path(platform):
    """Get the path to the dynamic library for the given platform."""
    base_libs_dir = "libs"
    lib_name = ""

    if platform == "macos-arm64":
        lib_name = "libpython3.10.dylib"
    elif platform == "macos-x86_64":
        lib_name = "libpython3.10.dylib"
    elif platform == "linux":
        lib_name = "libpython3.10.so"
    elif platform == "linux-aarch64":
        lib_name = "libpython3.10.so"
    elif platform == "windows-amd64":
        lib_name = "python310.dll"
    else:
        return None

    lib_path = os.path.join(base_libs_dir, platform, lib_name)
    if os.path.exists(lib_path):
        print(f"Found {lib_name} at: {lib_path}")
        return lib_path
    else:
        print(f"Warning: {lib_name} not found in {os.path.join(base_libs_dir, platform)}")
        return None

def run_command(command, cwd, env=None):
    process = subprocess.Popen(command, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, shell=True, universal_newlines=True, env=env)
    
    while True:
        stdout_line = process.stdout.readline()
        stderr_line = process.stderr.readline()
        
        if stdout_line:
            print(stdout_line, end='')
        if stderr_line:
            print(stderr_line, end='')
            
        if stdout_line == '' and stderr_line == '' and process.poll() is not None:
            break
            
    return_code = process.poll()
    if return_code != 0:
        print(f"Command '{command}' failed with return code {return_code}")
        exit(return_code)


def run_command_allow_fail(command, cwd, env=None):
    """Run a command like run_command but return the exit code instead of exiting.

    Returns (exit_code, stdout, stderr).
    """
    process = subprocess.Popen(command, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, shell=True, universal_newlines=True, env=env)
    stdout, stderr = process.communicate()
    if stdout:
        print(stdout, end='')
    if stderr:
        print(stderr, end='')
    return process.returncode, stdout, stderr

def get_platform_tag(platform):
    if platform == "macos-arm64":
        return "macosx_11_0_arm64"
    elif platform == "macos-x86_64":
        return "macosx_10_9_x86_64"
    elif platform == "linux":
        return "manylinux1_x86_64"
    elif platform == "linux-aarch64":
        return "manylinux2014_aarch64"
    elif platform == "windows-amd64":
        return "win_amd64"
    return None

def package_wheel(out_dir, binary_src_or_dir, platform, dll_path=None):
    """Packages the native binary into a pip wheel."""
    framework_dir = '../framework'
    temp_build_dir = os.path.join(out_dir, f"build-{platform}")
    os.makedirs(temp_build_dir, exist_ok=True)

    try:
        repo_pkg_dir = os.path.join("..", "python_package")
        out_pkg_dir = os.path.join(temp_build_dir, "python_package")
        
        if os.path.exists(out_pkg_dir):
            shutil.rmtree(out_pkg_dir)
        shutil.copytree(repo_pkg_dir, out_pkg_dir, ignore=shutil.ignore_patterns('starter'))
        print(f"Created working python package for {platform} at {out_pkg_dir}")

        platform_tag = get_platform_tag(platform)
        if platform_tag:
            setup_cfg_content = f"""[bdist_wheel]
plat_name={platform_tag}
"""
            with open(os.path.join(out_pkg_dir, "setup.cfg"), "w") as f:
                f.write(setup_cfg_content)
            print(f"Generated setup.cfg for platform {platform} with tag {platform_tag}")

        target_dir = os.path.join(out_pkg_dir, "src", "noventa")
        noventa_bin_dir = os.path.join(target_dir, "noventa_bin")
        
        if os.path.exists(noventa_bin_dir):
            shutil.rmtree(noventa_bin_dir)

        if os.path.isdir(binary_src_or_dir):
            shutil.copytree(binary_src_or_dir, noventa_bin_dir)
            print(f"Copied binary directory into working python package for {platform} at {noventa_bin_dir}")
        elif os.path.exists(binary_src_or_dir):
            os.makedirs(noventa_bin_dir, exist_ok=True)
            binary_name = "noventa.exe" if "windows" in platform else "noventa"
            pkg_dest = os.path.join(noventa_bin_dir, binary_name)
            shutil.copy(binary_src_or_dir, pkg_dest)
            try:
                os.chmod(pkg_dest, 0o755)
            except Exception:
                pass
            print(f"Copied binary into working python package for {platform} at {pkg_dest}")
        else:
            print(f"Skipping python wheel build for {platform} because binary was not produced.")
            return

        if dll_path and os.path.exists(dll_path):
            dll_dest = os.path.join(noventa_bin_dir, os.path.basename(dll_path))
            shutil.copy(dll_path, dll_dest)
            print(f"Copied DLL into working python package for {platform} at {dll_dest}")

        starter_src = os.path.join(framework_dir, "starter")
        starter_dest = os.path.join(target_dir, "starter")
        shutil.copytree(
            starter_src,
            starter_dest,
            ignore=shutil.ignore_patterns('.DS_Store'),
            copy_function=shutil.copy2,
            dirs_exist_ok=True
        )
        print(f"Copied starter templates into working python package for {platform} at {starter_dest}")

        wheels_dir = os.path.join(os.path.abspath(out_dir), "wheels")
        os.makedirs(wheels_dir, exist_ok=True)
        
        print(f"Building pip wheel for {platform} (trying 'python -m build')...")
        cmd_build = f"{sys.executable} -m build --wheel --outdir {wheels_dir}"
        rc, _, _ = run_command_allow_fail(cmd_build, cwd=out_pkg_dir)
        if rc != 0:
            print(f"'python -m build' failed for {platform}; falling back to 'pip wheel'...")
            cmd_pip_wheel = f"{sys.executable} -m pip wheel . --wheel-dir {wheels_dir}"
            rc2, _, _ = run_command_allow_fail(cmd_pip_wheel, cwd=out_pkg_dir)
            if rc2 != 0:
                print(f"Fallback 'pip wheel' for {platform} also failed; aborting wheel build.")
            else:
                print(f"Wheel for {platform} built successfully using pip wheel.")
        else:
            print(f"Wheel for {platform} built successfully using python -m build.")

    finally:
        if os.path.exists(temp_build_dir):
            shutil.rmtree(temp_build_dir)
            print(f"Cleaned up temporary build directory: {temp_build_dir}")

def build_rust_framework(out_dir):
    framework_dir = '../framework'
    
    # macOS builds
    if sys.platform == "darwin":
        # macOS ARM64 (Apple Silicon)
        print("Building Rust framework for macOS ARM64 with cargo...")
        env = os.environ.copy()
        env["RUSTFLAGS"] = f"-L {os.path.abspath('libs/macos-arm64')}"
        run_command("PYO3_NO_PYTHON=1 cargo build --release --target aarch64-apple-darwin", cwd=framework_dir, env=env)
        
        macos_arm_out_dir = os.path.join(out_dir, "macos-arm64")
        os.makedirs(macos_arm_out_dir, exist_ok=True)
        
        binary_src_arm = os.path.join(framework_dir, "target", "aarch64-apple-darwin", "release", "noventa")
        if os.path.exists(binary_src_arm):
            dest_arm = os.path.join(macos_arm_out_dir, "noventa")
            shutil.copy(binary_src_arm, dest_arm)
            print(f"Copied ARM64 binary to {dest_arm}")
            lib_path = get_platform_lib_path("macos-arm64")
            package_wheel(out_dir, dest_arm, "macos-arm64", dll_path=lib_path)
        else:
            print(f"Warning: expected ARM64 binary at {binary_src_arm} not found")

        # macOS x86_64 (Intel)
        print("Building Rust framework for macOS x86_64 with cargo...")
        env = os.environ.copy()
        env["RUSTFLAGS"] = f"-L {os.path.abspath('libs/macos-x86_64')}"
        run_command("PYO3_NO_PYTHON=1 cargo build --release --target x86_64-apple-darwin", cwd=framework_dir, env=env)

        macos_x86_out_dir = os.path.join(out_dir, "macos-x86_64")
        os.makedirs(macos_x86_out_dir, exist_ok=True)

        binary_src_x86 = os.path.join(framework_dir, "target", "x86_64-apple-darwin", "release", "noventa")
        if os.path.exists(binary_src_x86):
            dest_x86 = os.path.join(macos_x86_out_dir, "noventa")
            shutil.copy(binary_src_x86, dest_x86)
            print(f"Copied x86_64 binary to {dest_x86}")
            lib_path = get_platform_lib_path("macos-x86_64")
            package_wheel(out_dir, dest_x86, "macos-x86_64", dll_path=lib_path)
        else:
            print(f"Warning: expected x86_64 binary at {binary_src_x86} not found")

    # Cross-compile for Linux
    print("Cross-compiling Rust framework for Linux with cargo zigbuild...")
    env = os.environ.copy()
    if "DYLD_LIBRARY_PATH" in env:
        del env["DYLD_LIBRARY_PATH"]
    #env["RUSTFLAGS"] = f"-L {os.path.abspath('libs/linux')} -C link-arg=-Wl,--disable-new-dtags"
    run_command("cargo zigbuild --target x86_64-unknown-linux-gnu --release", cwd=framework_dir, env=env)
    
    linux_out_dir = os.path.join(out_dir, "linux")
    os.makedirs(linux_out_dir, exist_ok=True)
    
    binary_src = os.path.join(framework_dir, "target", "x86_64-unknown-linux-gnu", "release", "noventa")
    if os.path.exists(binary_src):
        dest = os.path.join(linux_out_dir, "noventa")
        shutil.copy(binary_src, dest)
        print(f"Copied binary to {dest}")
        lib_path = get_platform_lib_path("linux")
        package_wheel(out_dir, dest, "linux", dll_path=lib_path)
    else:
        print(f"Warning: expected binary at {binary_src} not found")

    # Cross-compile for Linux aarch64
    print("Cross-compiling Rust framework for Linux aarch64 with cargo zigbuild...")
    env = os.environ.copy()
    if "DYLD_LIBRARY_PATH" in env:
        del env["DYLD_LIBRARY_PATH"]
    env["RUSTFLAGS"] = f"-L {os.path.abspath('libs/linux-aarch64')} -C link-arg=-Wl,--disable-new-dtags"
    run_command("cargo zigbuild --target aarch64-unknown-linux-gnu --release", cwd=framework_dir, env=env)

    linux_aarch64_out_dir = os.path.join(out_dir, "linux-aarch64")
    os.makedirs(linux_aarch64_out_dir, exist_ok=True)

    binary_src_aarch64 = os.path.join(framework_dir, "target", "aarch64-unknown-linux-gnu", "release", "noventa")
    if os.path.exists(binary_src_aarch64):
        dest_aarch64 = os.path.join(linux_aarch64_out_dir, "noventa")
        shutil.copy(binary_src_aarch64, dest_aarch64)
        print(f"Copied aarch64 binary to {dest_aarch64}")
        lib_path = get_platform_lib_path("linux-aarch64")
        package_wheel(out_dir, dest_aarch64, "linux-aarch64", dll_path=lib_path)
    else:
        print(f"Warning: expected aarch64 binary at {binary_src_aarch64} not found")

    # Cross-compile for Windows AMD64
    print("Cross-compiling Rust framework for Windows AMD64 with cargo xwin...")
    env = os.environ.copy()
    env["RUSTFLAGS"] = f"-L {os.path.abspath('libs/windows-amd64')}"
    run_command("PYO3_NO_PYTHON=1 cargo xwin build --target x86_64-pc-windows-msvc --release", cwd=framework_dir, env=env)

    windows_amd64_out_dir = os.path.join(out_dir, "windows-amd64")
    os.makedirs(windows_amd64_out_dir, exist_ok=True)

    binary_src_amd64 = os.path.join(framework_dir, "target", "x86_64-pc-windows-msvc", "release", "noventa.exe")
    if os.path.exists(binary_src_amd64):
        dest_amd64 = os.path.join(windows_amd64_out_dir, "noventa.exe")
        shutil.copy(binary_src_amd64, dest_amd64)
        print(f"Copied AMD64 binary to {dest_amd64}")
        lib_path = get_platform_lib_path("windows-amd64")
        package_wheel(out_dir, dest_amd64, "windows-amd64", dll_path=lib_path)
    else:
        print(f"Warning: expected AMD64 binary at {binary_src_amd64} not found")


def package_vscode_extension(out_dir):
    extension_dir = '../vscode_extension'
    print("Packaging VSCode extension...")
    
    # Install dependencies
    run_command("npm install", cwd=extension_dir)
    
    # Package the extension
    run_command(f"npx vsce package --out {os.path.join('..', 'build', out_dir)}", cwd=extension_dir)
    print(f"Packaged extension to {out_dir}")

if __name__ == "__main__":
    output_directory = "dist"
    # Clean the output directory before starting the build
    if os.path.exists(output_directory):
        shutil.rmtree(output_directory)
    os.makedirs(output_directory)
        
    build_rust_framework(output_directory)
    package_vscode_extension(output_directory)

    # Clean up platform-specific directories
    for platform in ["macos-arm64", "macos-x86_64", "linux", "linux-aarch64", "windows-amd64"]:
        platform_dir = os.path.join(output_directory, platform)
        if os.path.exists(platform_dir):
            shutil.rmtree(platform_dir)
            print(f"Cleaned up directory: {platform_dir}")
            
    print("Build process completed successfully!")