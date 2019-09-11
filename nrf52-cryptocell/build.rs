use bindgen;

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rustc-link-search=nrf_cc310/lib/cortex-m4/hard-float/no-interrupts");
    println!("cargo:rustc-link-lib=libnrf_cc310_0.9.12");

    let bindings = bindgen::Builder::default()
        .ctypes_prefix("cty")
        .use_core()
        .header("wrapper.h")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
