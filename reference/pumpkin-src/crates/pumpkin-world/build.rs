fn main() -> std::io::Result<()> {
    println!("cargo:rustc-check-cfg=cfg(pumpkin_compiled_worldgen_profile)");
    println!("cargo:rerun-if-env-changed=PUMPKIN_COMPILED_WORLDGEN_PROFILE");
    if let Some(directory) = std::env::var_os("PUMPKIN_COMPILED_WORLDGEN_PROFILE") {
        let directory = std::path::Path::new(&directory);
        println!("cargo:rerun-if-changed={}", directory.display());
        if directory.join("cleanroom.rs").is_file() {
            return Ok(());
        }
        let source = directory.join("decoration.rs").canonicalize()?;
        println!("cargo:rerun-if-changed={}", source.display());
        println!(
            "cargo:rustc-env=PUMPKIN_WORLDGEN_PROFILE_DECORATION={}",
            source.display()
        );
        println!("cargo:rustc-cfg=pumpkin_compiled_worldgen_profile");
    }
    Ok(())
}
