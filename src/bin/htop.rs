//! htop — Alias binary for pstop.
//! When installed via `cargo install pstop`, this gives users the `htop` command on Windows.

fn main() {
    let exe = std::env::current_exe().expect("Failed to get current exe path");
    let dir = exe.parent().expect("Failed to get exe directory");
    let pstop = dir.join("pstop.exe");

    // Forward all arguments to pstop
    let args: Vec<String> = std::env::args().skip(1).collect();

    let status = std::process::Command::new(&pstop)
        .args(&args)
        .status()
        .unwrap_or_else(|e| {
            eprintln!("Failed to launch pstop: {}", e);
            eprintln!("Make sure pstop is installed: cargo install pstop");
            std::process::exit(1);
        });

    std::process::exit(status.code().unwrap_or(1));
}
