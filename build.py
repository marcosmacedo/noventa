import subprocess
import os
import shutil
import sys

def find_libpython_path():
    """Get the path to libpython, prioritizing DYLD_LIBRARY_PATH."""
    try:
        # Get Python version from the interpreter
        version_cmd = f"{sys.executable} -c 'import sys; print(f\"{{sys.version_info.major}}.{{sys.version_info.minor}}\")'"
        version_proc = subprocess.run(version_cmd, shell=True, capture_output=True, text=True, check=True)
        version = version_proc.stdout.strip()
        lib_name = f"libpython{version}.dylib"

        # Prioritize DYLD_LIBRARY_PATH if it's set
        dyld_path = os.environ.get("DYLD_LIBRARY_PATH")
        if not dyld_path:
            sys.exit("DYLD_LIBRARY_PATH is not set. Please set it to include the directory containing libpython.")
        if dyld_path:
            # Check all paths in DYLD_LIBRARY_PATH
            for path in dyld_path.split(':'):
                lib_path = os.path.join(path, lib_name)
                if os.path.exists(lib_path):
                    print(f"Found {lib_name} in DYLD_LIBRARY_PATH: {lib_path}")
                    return lib_path
                else:
                    sys.exit(f"{lib_name} not found in DYLD_LIBRARY_PATH: {path}")

        # Fallback to pyo3-build-config if not found in DYLD_LIBRARY_PATH
        lib_dir_cmd = "python -m pyo3_build_config --lib-dir"
        lib_dir_proc = subprocess.run(lib_dir_cmd, shell=True, capture_output=True, text=True, check=True)
        lib_dir = lib_dir_proc.stdout.strip()

        lib_path = os.path.join(lib_dir, lib_name)

        if os.path.exists(lib_path):
            return lib_path
        else:
            print(f"Warning: {lib_name} not found in {lib_dir} or DYLD_LIBRARY_PATH")
            return None
    except Exception as e:
        print(f"Error finding libpython: {e}")
        return None

def find_python_dll_path():
    """Get the path to python310.dll on Windows."""
    try:
        python_dir = "/Users/marcos/Downloads/python_windows"
        dll_name = "python310.dll"
        dll_path = os.path.join(python_dir, dll_name)

        if os.path.exists(dll_path):
            print(f"Found {dll_name} at: {dll_path}")
            return dll_path
        else:
            print(f"Warning: {dll_name} not found in {python_dir}")
            # Fallback to checking the directory of the current Python executable
            python_executable_path = sys.executable
            python_executable_dir = os.path.dirname(python_executable_path)
            dll_path_fallback = os.path.join(python_executable_dir, dll_name)
            if os.path.exists(dll_path_fallback):
                print(f"Found {dll_name} in fallback location: {dll_path_fallback}")
                return dll_path_fallback
            else:
                print(f"Warning: {dll_name} not found in fallback location {python_executable_dir} either.")
                return None
    except Exception as e:
        print(f"Error finding {dll_name}: {e}")
        return None

def run_command(command, cwd):
    process = subprocess.Popen(command, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, shell=True, universal_newlines=True)
    
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


def run_command_allow_fail(command, cwd):
    """Run a command like run_command but return the exit code instead of exiting.

    Returns (exit_code, stdout, stderr).
    """
    process = subprocess.Popen(command, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, shell=True, universal_newlines=True)
    stdout, stderr = process.communicate()
    if stdout:
        print(stdout, end='')
    if stderr:
        print(stderr, end='')
    return process.returncode, stdout, stderr

def package_wheel(out_dir, binary_src_or_dir, platform, dll_path=None):
    """Packages the native binary into a pip wheel."""
    framework_dir = 'framework'
    platform_out_dir = os.path.join(out_dir, platform)
    os.makedirs(platform_out_dir, exist_ok=True)

    try:
        repo_pkg_dir = os.path.join("python_package")
        out_pkg_dir = os.path.join(platform_out_dir, "python_package")
        abs_platform_out_dir = os.path.abspath(platform_out_dir)

        if os.path.exists(out_pkg_dir):
            shutil.rmtree(out_pkg_dir)
        shutil.copytree(repo_pkg_dir, out_pkg_dir, ignore=shutil.ignore_patterns('starter'))
        print(f"Created working python package for {platform} at {out_pkg_dir}")

        target_dir = os.path.join(out_pkg_dir, "src", "noventa")
        noventa_bin_dir = os.path.join(target_dir, "noventa_bin")
        
        if os.path.exists(noventa_bin_dir):
            shutil.rmtree(noventa_bin_dir)

        if os.path.isdir(binary_src_or_dir):
            shutil.copytree(binary_src_or_dir, noventa_bin_dir)
            print(f"Copied binary directory into working python package for {platform} at {noventa_bin_dir}")
        elif os.path.exists(binary_src_or_dir):
            os.makedirs(noventa_bin_dir, exist_ok=True)
            pkg_dest = os.path.join(noventa_bin_dir, "noventa")
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

        print(f"Building pip wheel for {platform} (trying 'python -m build')...")
        cmd_build = f"{sys.executable} -m build --wheel --outdir {abs_platform_out_dir}"
        rc, _, _ = run_command_allow_fail(cmd_build, cwd=out_pkg_dir)
        if rc != 0:
            print(f"'python -m build' failed for {platform}; trying to install 'build' and retrying...")
            install_cmd = f"{sys.executable} -m pip install --upgrade build"
            rc_install, _, _ = run_command_allow_fail(install_cmd, cwd=out_pkg_dir)
            if rc_install == 0:
                print(f"Installed 'build'; retrying python -m build for {platform}...")
                rc_retry, _, _ = run_command_allow_fail(cmd_build, cwd=out_pkg_dir)
                if rc_retry == 0:
                    print(f"Wheel for {platform} built successfully using python -m build after installing build.")
                else:
                    print(f"Retry for {platform} after installing 'build' still failed; falling back to 'pip wheel'...")
                    cmd_pip_wheel = f"{sys.executable} -m pip wheel . --wheel-dir {abs_platform_out_dir}"
                    rc2, _, _ = run_command_allow_fail(cmd_pip_wheel, cwd=out_pkg_dir)
                    if rc2 != 0:
                        print(f"Fallback 'pip wheel' for {platform} also failed; aborting wheel build.")
                    else:
                        print(f"Wheel for {platform} built successfully using pip wheel.")
            else:
                print(f"Could not install 'build' for {platform}; falling back to 'pip wheel'...")
                cmd_pip_wheel = f"{sys.executable} -m pip wheel . --wheel-dir {abs_platform_out_dir}"
                rc2, _, _ = run_command_allow_fail(cmd_pip_wheel, cwd=out_pkg_dir)
                if rc2 != 0:
                    print(f"Fallback 'pip wheel' for {platform} also failed; aborting wheel build.")
                else:
                    print(f"Wheel for {platform} built successfully using pip wheel.")
        else:
            print(f"Wheel for {platform} built successfully using python -m build.")

    except Exception as e:
        print(f"Error while preparing python package for {platform}: {e}")

def build_rust_framework(out_dir):
    framework_dir = 'framework'
    
    # Build for macOS
    if sys.platform == "darwin":
        print("Building Rust framework for macOS with cargo...")
        run_command("PYO3_NO_PYTHON=1 cargo build --release", cwd=framework_dir)
        
        macos_out_dir = os.path.join(out_dir, "macos")
        os.makedirs(macos_out_dir, exist_ok=True)
        
        noventa_bin_dir = os.path.join(macos_out_dir, "noventa_bin")
        os.makedirs(noventa_bin_dir, exist_ok=True)

        binary_src = os.path.join(framework_dir, "target", "release", "noventa")
        if os.path.exists(binary_src):
            dest = os.path.join(noventa_bin_dir, "noventa")
            shutil.copy(binary_src, dest)
            print(f"Copied binary to {dest}")

            libpython_path = find_libpython_path()
            if libpython_path:
                libpython_dest = os.path.join(noventa_bin_dir, os.path.basename(libpython_path))
                shutil.copy(libpython_path, libpython_dest)
                print(f"Copied {libpython_path} to {libpython_dest}")

                noventa_binary_path = os.path.join(noventa_bin_dir, "noventa")
                if os.path.exists(noventa_binary_path):
                    otool_cmd = f"otool -L {noventa_binary_path} | grep libpython"
                    try:
                        otool_proc = subprocess.run(otool_cmd, shell=True, capture_output=True, text=True)
                        if otool_proc.returncode == 0 and otool_proc.stdout:
                            original_lib_path = otool_proc.stdout.strip().split(" ")[0]
                            new_lib_path = f"@loader_path/{os.path.basename(libpython_path)}"
                            
                            print(f"Changing library path from {original_lib_path} to {new_lib_path}")
                            install_cmd = f"install_name_tool -change {original_lib_path} {new_lib_path} {noventa_binary_path}"
                            run_command(install_cmd, cwd=".")

                            rpath_cmd = f"install_name_tool -delete_rpath {os.path.dirname(original_lib_path)} {noventa_binary_path}"
                            run_command_allow_fail(rpath_cmd, cwd=".")

                            print("Successfully updated binary to use bundled libpython.")
                        else:
                            print("Warning: Could not determine original libpython path with otool.")
                    except Exception as e:
                        print(f"Error running otool or install_name_tool: {e}")
            
            package_wheel(out_dir, noventa_bin_dir, "macos")
        else:
            print(f"Warning: expected binary at {binary_src} not found")
    
    # Cross-compile for Linux
    print("Cross-compiling Rust framework for Linux with cargo zigbuild...")
    run_command("PYO3_NO_PYTHON=1 cargo zigbuild --target x86_64-unknown-linux-gnu --release", cwd=framework_dir)
    
    linux_out_dir = os.path.join(out_dir, "linux")
    os.makedirs(linux_out_dir, exist_ok=True)
    
    binary_src = os.path.join(framework_dir, "target", "x86_64-unknown-linux-gnu", "release", "noventa")
    if os.path.exists(binary_src):
        dest = os.path.join(linux_out_dir, "noventa")
        shutil.copy(binary_src, dest)
        print(f"Copied binary to {dest}")
        package_wheel(out_dir, dest, "linux")
    else:
        print(f"Warning: expected binary at {binary_src} not found")

    # Cross-compile for Windows
    print("Cross-compiling Rust framework for Windows with cargo xwin...")
    run_command("PYO3_NO_PYTHON=1 cargo xwin build --target x86_64-pc-windows-msvc --release", cwd=framework_dir)

    windows_out_dir = os.path.join(out_dir, "windows")
    os.makedirs(windows_out_dir, exist_ok=True)

    binary_src = os.path.join(framework_dir, "target", "x86_64-pc-windows-msvc", "release", "noventa.exe")
    if os.path.exists(binary_src):
        dest = os.path.join(windows_out_dir, "noventa.exe")
        shutil.copy(binary_src, dest)
        print(f"Copied binary to {dest}")

        # Always try to find and copy the DLL when cross-compiling for windows
        python_dll_path = find_python_dll_path()
        package_wheel(out_dir, dest, "windows", dll_path=python_dll_path)
    else:
        print(f"Warning: expected binary at {binary_src} not found")

def package_vscode_extension(out_dir):
    extension_dir = 'vscode_extension'
    print("Packaging VSCode extension...")
    
    # Install dependencies
    run_command("npm install", cwd=extension_dir)
    
    # Package the extension
    run_command(f"npx vsce package --out {os.path.join('..', out_dir)}", cwd=extension_dir)
    print(f"Packaged extension to {out_dir}")

if __name__ == "__main__":
    output_directory = "out"
    # Clean the output directory before starting the build
    if os.path.exists(output_directory):
        shutil.rmtree(output_directory)
    os.makedirs(output_directory)
        
    build_rust_framework(output_directory)
    package_vscode_extension(output_directory)
    print("Build process completed successfully!")