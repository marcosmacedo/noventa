import sys
import subprocess
import os
from importlib.resources import files, as_file

def main():
    """
    This is the entry point for the 'noventa' command.
    It locates the binary and the starter templates and executes the binary.
    """
    try:
        # Get the path to the noventa binary
        noventa_binary_path = files('noventa').joinpath('noventa_bin/noventa')
        
        # Get the path to the starter templates
        starter_path = files('noventa').joinpath('starter')

        # Get the Python home path
        python_home = sys.prefix
        
        # Set the PYTHONHOME environment variable for the subprocess
        env = os.environ.copy()
        env['PYTHONHOME'] = python_home

        with as_file(noventa_binary_path) as bin_path, as_file(starter_path) as st_path:
            # Pass the starter path as a --starter flag
            cmd = [str(bin_path), "--starter", str(st_path)] + sys.argv[1:]
            
            # Execute the binary with the modified environment
            result = subprocess.run(cmd, env=env)
            sys.exit(result.returncode)

    except Exception as e:
        print(f"Error executing noventa: {e}")
        sys.exit(1)

if __name__ == '__main__':
    main()