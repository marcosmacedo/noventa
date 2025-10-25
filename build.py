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

def build_rust_framework(out_dir):
    framework_dir = 'framework'
    # By default build a native Rust binary using `cargo build --release`.
    # This avoids using maturin which can produce wheels that bundle Python
    # and (depending on the build environment) end up statically linking libpython.
    # If you still want a Python wheel built by maturin, set the env var
    # USE_MATURIN=1 when running this script.
    use_maturin = os.environ.get("USE_MATURIN") == "1"

    if use_maturin:
        print("Building Rust framework with maturin (wheel)...")
        targets = [
            "aarch64-apple-darwin",
        ]
        for target in targets:
            print(f"Building wheel for target: {target}")
            run_command(f"maturin build --release -i {sys.executable} --target {target} --out {os.path.join('..', out_dir)}", cwd=framework_dir)
        return

    # Build a native binary. This will link to the Python library that the
    # pyo3 build-config finds on the build machine (preferably a shared lib).
    print("Building Rust framework as a native binary with cargo...")
    # You can add extra targets if you have cross toolchains set up.
    run_command("cargo build --release", cwd=framework_dir)

    # Copy the produced binary into the output dir so downstream packaging
    # (the VSCode extension, etc.) can pick it up easily.
    binary_src = os.path.join(framework_dir, "target", "release", "noventa")
    if os.path.exists(binary_src):
        dest = os.path.join(out_dir, "noventa")
        shutil.copy(binary_src, dest)
        print(f"Copied binary to {dest}")
    else:
        print(f"Warning: expected binary at {binary_src} not found")

    # If on macOS, bundle libpython
    if sys.platform == "darwin":
        libpython_path = find_libpython_path()
        if libpython_path:
            # Copy libpython to be alongside the noventa binary
            libpython_dest = os.path.join(out_dir, os.path.basename(libpython_path))
            shutil.copy(libpython_path, libpython_dest)
            print(f"Copied {libpython_path} to {libpython_dest}")

            # Modify the noventa binary to look for libpython in the same directory
            noventa_binary_path = os.path.join(out_dir, "noventa")
            if os.path.exists(noventa_binary_path):
                # Get the original path from the binary's perspective
                otool_cmd = f"otool -L {noventa_binary_path} | grep libpython"
                try:
                    otool_proc = subprocess.run(otool_cmd, shell=True, capture_output=True, text=True)
                    if otool_proc.returncode == 0 and otool_proc.stdout:
                        original_lib_path = otool_proc.stdout.strip().split(" ")[0]
                        new_lib_path = f"@loader_path/{os.path.basename(libpython_path)}"
                        
                        print(f"Changing library path from {original_lib_path} to {new_lib_path}")
                        install_cmd = f"install_name_tool -change {original_lib_path} {new_lib_path} {noventa_binary_path}"
                        run_command(install_cmd, cwd=".")

                        # Also remove the old rpath to ensure it's not used
                        rpath_cmd = f"install_name_tool -delete_rpath {os.path.dirname(original_lib_path)} {noventa_binary_path}"
                        run_command_allow_fail(rpath_cmd, cwd=".")

                        print("Successfully updated binary to use bundled libpython.")
                    else:
                        print("Warning: Could not determine original libpython path with otool.")
                except Exception as e:
                    print(f"Error running otool or install_name_tool: {e}")

    # Package the native binary into a pip wheel, but operate on a copy of
    # the python_package tree inside the output directory so we don't mutate the repo.
    try:
        repo_pkg_dir = os.path.join("python_package")
        out_pkg_dir = os.path.join(out_dir, "python_package")
        abs_out_dir = os.path.abspath(out_dir)

        # Remove any existing copy in out/ and copy the repo python_package into out/
        if os.path.exists(out_pkg_dir):
            shutil.rmtree(out_pkg_dir)
        shutil.copytree(repo_pkg_dir, out_pkg_dir)
        print(f"Created working python package at {out_pkg_dir}")

        # Define the target directory for the binary and templates
        target_dir = os.path.join(out_pkg_dir, "src", "noventa")

        # Copy binary into the working package
        noventa_bin_dir = os.path.join(target_dir, "noventa_bin")
        os.makedirs(noventa_bin_dir, exist_ok=True)
        pkg_dest = os.path.join(noventa_bin_dir, "noventa")
        if os.path.exists(binary_src):
            shutil.copy(binary_src, pkg_dest)
            try:
                os.chmod(pkg_dest, 0o755)
            except Exception:
                pass
            print(f"Copied binary into working python package at {pkg_dest}")
        else:
            print("Skipping python wheel build because binary was not produced.")
            return

        # Copy the starter template directory into the working package
        starter_src = os.path.join(framework_dir, "starter")
        starter_dest = os.path.join(target_dir, "starter")
        if os.path.exists(starter_src):
            if os.path.exists(starter_dest):
                shutil.rmtree(starter_dest)
            #  `copy_function=shutil.copy` is used to ensure that all files, including hidden ones (dotfiles), are copied.
            # Use a robust copytree that includes dotfiles but ignores .DS_Store
            shutil.copytree(
                starter_src,
                starter_dest,
                ignore=shutil.ignore_patterns('.DS_Store'),
                copy_function=shutil.copy2,
                dirs_exist_ok=True
            )
            print(f"Copied starter templates into working python package at {starter_dest}")
        else:
            print(f"No starter directory at {starter_src}; skipping starter copy")

        # Build the wheel using the working package directory. Output the wheel
        # directly into the top-level out directory (abs path) so it's easy to find.
        print("Building pip wheel that contains the native binary (trying 'python -m build')...")
        cmd_build = f"{sys.executable} -m build --wheel --outdir {abs_out_dir}"
        rc, _, _ = run_command_allow_fail(cmd_build, cwd=out_pkg_dir)
        if rc != 0:
            print("'python -m build' failed; trying to install 'build' into this Python and retrying...")
            # Try to install the build package into the interpreter and retry
            install_cmd = f"{sys.executable} -m pip install --upgrade build"
            rc_install, _, _ = run_command_allow_fail(install_cmd, cwd=out_pkg_dir)
            if rc_install == 0:
                print("Installed 'build'; retrying python -m build...")
                rc_retry, _, _ = run_command_allow_fail(cmd_build, cwd=out_pkg_dir)
                if rc_retry == 0:
                    print("Wheel built successfully using python -m build after installing build.")
                else:
                    print("Retry after installing 'build' still failed; falling back to 'pip wheel'...")
                    cmd_pip_wheel = f"{sys.executable} -m pip wheel . --wheel-dir {abs_out_dir}"
                    rc2, _, _ = run_command_allow_fail(cmd_pip_wheel, cwd=out_pkg_dir)
                    if rc2 != 0:
                        print("Fallback 'pip wheel' also failed; aborting wheel build.")
                    else:
                        print("Wheel built successfully using pip wheel.")
            else:
                print("Could not install 'build' into the interpreter; falling back to 'pip wheel'...")
                cmd_pip_wheel = f"{sys.executable} -m pip wheel . --wheel-dir {abs_out_dir}"
                rc2, _, _ = run_command_allow_fail(cmd_pip_wheel, cwd=out_pkg_dir)
                if rc2 != 0:
                    print("Fallback 'pip wheel' also failed; aborting wheel build.")
                else:
                    print("Wheel built successfully using pip wheel.")
        else:
            print("Wheel built successfully using python -m build.")

    except Exception as e:
        print(f"Error while preparing python package: {e}")

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