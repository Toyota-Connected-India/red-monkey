use std::process::Command;

fn main() {
    let output = Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .output()
        .expect("Error parsing the git commit");
    let git_hash =
        String::from_utf8(output.stdout).expect("Error converting stdout to string slice");
    println!("cargo:rustc-env=GIT_HASH={}", git_hash);
}
