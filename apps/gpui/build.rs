use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
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
    let Some(rc_compiler) = find_rc_compiler(&target_env) else {
        println!("cargo:warning=No Windows resource compiler (RC/windres/rc.exe/llvm-rc.exe) found; executable icon resource skipped");
        return;
    };
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let rc_file = out_dir.join("anywhere-icon.rc");
    let res_file = out_dir.join(if target_env == "gnu" {
        "anywhere-icon.o"
    } else {
        "anywhere-icon.res"
    });
    let escaped_icon = icon_path
        .canonicalize()
        .unwrap_or(icon_path)
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('\\', "/");
    fs::write(&rc_file, format!("1 ICON \"{escaped_icon}\"\n")).expect("write icon rc");

    let compiler_name = rc_compiler
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut command = Command::new(&rc_compiler);
    if compiler_name.contains("windres") {
        command
            .arg("-i")
            .arg(&rc_file)
            .args(["-O", "coff", "-o"])
            .arg(&res_file);
    } else if compiler_name.contains("llvm-rc") {
        command.arg("/no-preprocess");
        command.arg("/fo").arg(&res_file).arg(&rc_file);
    } else {
        command.arg("/fo").arg(&res_file).arg(&rc_file);
    }
    let status = command.status();
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

fn find_rc_compiler(target_env: &str) -> Option<PathBuf> {
    if let Ok(explicit) = env::var("RC") {
        let candidate = PathBuf::from(explicit);
        if candidate.is_file() || is_on_path(&candidate) {
            return Some(candidate);
        }
    }
    let tools: &[&str] = if target_env == "gnu" {
        &["windres.exe", "windres", "rc.exe", "llvm-rc.exe", "llvm-rc"]
    } else {
        &["rc.exe", "llvm-rc.exe", "llvm-rc"]
    };
    for tool in tools {
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
