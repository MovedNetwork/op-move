use std::{process::Command, str};

fn main() {
    let output = Command::new("rustc")
        .arg("--version")
        .output()
        .expect("Failed to execute rustc --version");
    let version = str::from_utf8(&output.stdout)
        .expect("Failed to convert rustc --version output to a string")
        .trim()
        .split(' ')
        .nth(1)
        .unwrap_or("unknown_version")
        .to_string();
    println!("cargo::rustc-env=RUSTC_VERSION=rust_{}", version);

    let target = std::env::var("TARGET").unwrap_or("unknown_target".into());
    println!("cargo::rustc-env=TARGET_TRIPLET={}", target);
}
