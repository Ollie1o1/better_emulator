// Point the linker at Homebrew's SDL2 on macOS when not building SDL from source.
fn main() {
    let macos = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos");
    let system_sdl = std::env::var_os("CARGO_FEATURE_SDL").is_some()
        && std::env::var_os("CARGO_FEATURE_BUNDLED").is_none();
    if macos && system_sdl {
        for dir in ["/opt/homebrew/lib", "/usr/local/lib"] {
            if std::path::Path::new(dir).join("libSDL2.dylib").exists() {
                println!("cargo:rustc-link-search=native={dir}");
            }
        }
    }
}
