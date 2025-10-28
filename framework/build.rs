fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    
    if target_os == "linux" {
        println!("cargo:rustc-link-search=native=/Users/marcos/Downloads/python_linux/lib");
        println!("cargo:rustc-link-lib=dylib=python3.10");
    } else if target_os == "windows" {
        println!("cargo:rustc-link-search=native=/Users/marcos/Downloads/python_windows/libs");
        println!("cargo:rustc-link-lib=dylib=python310");
    } else if target_os == "macos" {
        println!("cargo:rustc-link-search=native=/Users/marcos/.conda/envs/py310/lib");
        println!("cargo:rustc-link-lib=dylib=python3.10");
    }
}
