use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=EUNOMIO_HELPER_BINARY");
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let dist = manifest_dir.join("../helper/dist");
    std::fs::create_dir_all(&dist).expect("create helper/dist");
    let target = dist.join("cursor-helper");

    if let Ok(src) = std::env::var("EUNOMIO_HELPER_BINARY") {
        let src_path = PathBuf::from(&src);
        println!("cargo:rerun-if-changed={}", src_path.display());
        if src_path.exists() {
            std::fs::copy(&src_path, &target).expect("copy helper binary");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms =
                    std::fs::metadata(&target).expect("stat helper").permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&target, perms).expect("chmod helper");
            }
        } else {
            println!(
                "cargo:warning=EUNOMIO_HELPER_BINARY={} does not exist; cursor-helper will be missing",
                src
            );
        }
    } else {
        println!("cargo:rerun-if-changed={}", target.display());
    }
}
