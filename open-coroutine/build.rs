use std::env;
use std::path::PathBuf;

fn main() {
    //fix dylib name
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let deps = out_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("deps");
    let mut pattern = deps.to_str().unwrap().to_owned();
    if cfg!(target_os = "linux") {
        pattern += "/libopen_coroutine_hook*.so";
        for path in glob::glob(&pattern)
            .expect("Failed to read glob pattern")
            .flatten()
        {
            std::fs::rename(path, deps.join("libopen_coroutine_hook.so"))
                .expect("rename to libopen_coroutine_hook.so failed!");
        }
    } else if cfg!(target_os = "macos") {
        pattern += "/libopen_coroutine_hook*.dylib";
        for path in glob::glob(&pattern)
            .expect("Failed to read glob pattern")
            .flatten()
        {
            std::fs::rename(path, deps.join("libopen_coroutine_hook.dylib"))
                .expect("rename to libopen_coroutine_hook.dylib failed!");
        }
    } else if cfg!(target_os = "windows") {
        let dll_pattern = pattern.clone() + "/open_coroutine_hook*.dll";
        for path in glob::glob(&dll_pattern)
            .expect("Failed to read glob pattern")
            .flatten()
        {
            std::fs::rename(path, deps.join("open_coroutine_hook.dll"))
                .expect("rename to open_coroutine_hook.dll failed!");
        }

        let lib_pattern = pattern.clone() + "/open_coroutine_hook*.dll.lib";
        for path in glob::glob(&lib_pattern)
            .expect("Failed to read glob pattern")
            .flatten()
        {
            std::fs::rename(path, deps.join("open_coroutine_hook.lib"))
                .expect("rename to open_coroutine_hook.lib failed!");
        }
        let lib_pattern = pattern + "/open_coroutine_hook*.lib";
        for path in glob::glob(&lib_pattern)
            .expect("Failed to read glob pattern")
            .flatten()
        {
            std::fs::rename(path, deps.join("open_coroutine_hook.lib"))
                .expect("rename to open_coroutine_hook.lib failed!");
        }
    } else {
        panic!("unsupported platform");
    }
    //link hook dylib
    println!("cargo:rustc-link-lib=dylib=open_coroutine_hook");
}
