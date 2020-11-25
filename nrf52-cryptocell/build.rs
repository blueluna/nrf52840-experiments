use std::env;
use std::path::PathBuf;

fn main() {
    let crate_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    println!(
        "cargo:rustc-link-search={}",
        crate_path
            .join("nrf_cc310/lib/cortex-m4/hard-float/no-interrupts")
            .to_string_lossy()
    );
    println!("cargo:rustc-link-lib=static=nrf_cc310_0.9.13");
}
