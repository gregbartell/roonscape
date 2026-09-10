fn main() {
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let shaders = format!(
        "static const char *graphics_shader = R\"SHADER({})SHADER\";\n\
         static const char *sprite_shader = R\"SHADER({})SHADER\";\n",
        include_str!("native/graphics.frag"),
        include_str!("native/sprite.frag"),
    );
    std::fs::write(output.join("shaders.h"), shaders).unwrap();
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .include(output)
        .file("native/window.cpp");
    for package in ["Qt6Quick", "Qt6OpenGL", "libjpeg"] {
        let mut config = pkg_config::Config::new();
        if package.starts_with("Qt6") {
            config.atleast_version("6.2");
        }
        let library = config
            .probe(package)
            .unwrap_or_else(|error| panic!("Renderer requires {package}: {error}"));
        for path in library.include_paths {
            build.include(path);
        }
    }
    build.compile("roonscape_window");
    for name in ["roonscape_frame_serial", "roonscape_frame_time_micros"] {
        println!("cargo:rustc-link-arg=-Wl,--export-dynamic-symbol={name}");
    }
    println!("cargo:rerun-if-changed=native");
}
