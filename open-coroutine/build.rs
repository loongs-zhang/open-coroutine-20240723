use std::env::var;
use std::fs::{read_dir, rename};
use std::path::PathBuf;

fn main() {
    //fix dylib name
    let out_dir = PathBuf::from(var("OUT_DIR").expect("env not found"));
    let deps = out_dir
        .parent()
        .expect("can not find deps dir")
        .parent()
        .expect("can not find deps dir")
        .parent()
        .expect("can not find deps dir")
        .join("deps");
    let lib_names = [
        String::from("libopen_coroutine_hook.so"),
        String::from("libopen_coroutine_hook.dylib"),
        String::from("open_coroutine_hook.lib"),
    ];
    for entry in read_dir(deps.clone())
        .expect("Failed to read deps")
        .flatten()
    {
        let file_name = entry.file_name().to_string_lossy().to_string();
        if !file_name.contains("open_coroutine_hook") {
            continue;
        }
        if lib_names.contains(&file_name) {
            break;
        }
        if file_name.eq("open_coroutine_hook.dll") {
            continue;
        }
        if cfg!(target_os = "linux") && file_name.ends_with(".so") {
            rename(deps.join(file_name), deps.join("libopen_coroutine_hook.so"))
                .expect("rename to libopen_coroutine_hook.so failed!");
        } else if cfg!(target_os = "macos") && file_name.ends_with(".dylib") {
            rename(
                deps.join(file_name),
                deps.join("libopen_coroutine_hook.dylib"),
            )
            .expect("rename to libopen_coroutine_hook.dylib failed!");
        } else if cfg!(windows) {
            if file_name.ends_with(".dll") {
                rename(deps.join(file_name), deps.join("open_coroutine_hook.dll"))
                    .expect("rename to open_coroutine_hook.dll failed!");
            } else if file_name.ends_with(".lib") {
                //fixme when link targets like ${arch}-pc-windows-msvc, this will not work
                // it seems that .dll.lib has not been generated at this timestamp
                rename(deps.join(file_name), deps.join("open_coroutine_hook.lib"))
                    .expect("rename to open_coroutine_hook.lib failed!");
            }
        }
    }
    //link hook dylib
    println!("cargo:rustc-link-lib=dylib=open_coroutine_hook");
}
