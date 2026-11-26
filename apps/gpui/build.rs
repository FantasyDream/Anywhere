use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let icon_path = manifest_dir.join("../../resources/icon/Anywhere.ico");
    println!("cargo:rerun-if-changed={}", icon_path.display());
    println!("cargo:rerun-if-env-changed=RC");
    if !icon_path.is_file() {
        println!(
            "cargo:warning=Windows icon missing at {}; skipping executable icon resource",
            icon_path.display()
        );
        return;
    }
    let Some(rc_compiler) = find_rc_compiler() else {
        println!("cargo:warning=No Windows resource compiler (RC/rc.exe/llvm-rc.exe) found; executable icon resource skipped");
        return;
    };
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let rc_file = out_dir.join("anywhere-icon.rc");
    let res_file = out_dir.join("anywhere-icon.res");
    let escaped_icon = icon_path
        .canonicalize()
        .unwrap_or(icon_path)
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('\\', "/");
    fs::write(&rc_file, format!("1 ICON \"{escaped_icon}\"\n")).expect("write icon rc");

    let mut command = Command::new(&rc_compiler);
    let is_llvm_rc = rc_compiler
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.to_ascii_lowercase().contains("llvm-rc"));
    if is_llvm_rc {
        command.arg("/no-preprocess");
    }
    let status = command.arg("/fo").arg(&res_file).arg(&rc_file).status();
    match status {
        Ok(status) if status.success() && res_file.is_file() => {
            println!(
                "cargo:rustc-link-arg-bin=anywhere-gpui={}",
                res_file.display()
            );
        }
        Ok(status) => {
            println!(
                "cargo:warning=Resource compiler {} exited with {status}; executable icon resource skipped",
                rc_compiler.display()
            );
        }
        Err(error) => {
            println!(
                "cargo:warning=Failed to run resource compiler {}: {error}",
                rc_compiler.display()
            );
        }
    }
}

fn find_rc_compiler() -> Option<PathBuf> {
    if let Ok(explicit) = env::var("RC") {
        let candidate = PathBuf::from(explicit);
        if candidate.is_file() || is_on_path(&candidate) {
            return Some(candidate);
        }
    }
    for tool in ["rc.exe", "llvm-rc.exe", "llvm-rc"] {
        let candidate = PathBuf::from(tool);
        if is_on_path(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_on_path(program: &Path) -> bool {
    if program.is_file() {
        return true;
    }
    let Some(paths) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&paths).any(|dir| dir.join(program).is_file())
}
