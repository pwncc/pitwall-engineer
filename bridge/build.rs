use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=../dashboard/dist");
    // Only embed icon when targeting Windows
    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "windows" {
        return;
    }

    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let icon_path = manifest.join("assets").join("icon.ico");
    let rc_path = out_dir.join("icon.rc");
    let res_path = out_dir.join("icon.res");

    // Rerun only when the icon changes
    println!("cargo:rerun-if-changed=assets/icon.ico");

    if !icon_path.exists() {
        eprintln!("cargo:warning=assets/icon.ico not found — skipping icon embed");
        return;
    }

    // Write a minimal Win32 resource script
    let icon_str = icon_path
        .to_str()
        .expect("icon path is valid UTF-8")
        .replace('\\', "/"); // zig rc is fine with forward slashes on Linux
    fs::write(&rc_path, format!("IDI_ICON1 ICON \"{}\"\n", icon_str)).expect("write icon.rc");

    if env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default() == "msvc" {
        let sdk_root = PathBuf::from(
            env::var("ProgramFiles(x86)").unwrap_or_else(|_| r"C:\Program Files (x86)".into()),
        )
        .join("Windows Kits/10/bin");
        let mut versions = fs::read_dir(sdk_root)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("10."))
            })
            .collect::<Vec<_>>();
        versions.sort();
        if let Some(rc) = versions
            .into_iter()
            .rev()
            .map(|p| p.join("x64/rc.exe"))
            .find(|p| p.exists())
        {
            if Command::new(rc)
                .arg("/nologo")
                .arg("/fo")
                .arg(&res_path)
                .arg(&rc_path)
                .status()
                .is_ok_and(|s| s.success())
            {
                println!("cargo:rustc-link-arg={}", res_path.display());
                return;
            }
        }
        eprintln!(
            "cargo:warning=Windows resource compiler unavailable; keeping default executable icon"
        );
        return;
    }

    // Locate zig (prefer $HOME/.local/bin/zig, fall back to PATH)
    let zig = env::var("ZIG_PATH").unwrap_or_else(|_| {
        let home = env::var("HOME").unwrap_or_default();
        format!("{}/.local/bin/zig", home)
    });

    // "--" prevents zig rc from treating absolute Linux paths as /option flags
    let status = match Command::new(&zig)
        .args([
            "rc",
            "--",
            rc_path.to_str().unwrap(),
            res_path.to_str().unwrap(),
        ])
        .status()
    {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cargo:warning=zig not found ({e}) — skipping icon embed");
            return;
        }
    };

    if !status.success() {
        eprintln!("cargo:warning=`zig rc` failed — skipping icon embed");
        return;
    }

    // LLD (used by cargo-zigbuild for windows-gnu) accepts .res files directly
    println!("cargo:rustc-link-arg={}", res_path.display());
}
