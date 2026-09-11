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
    // Qt 6.2 distributions provide qmake metadata without pkg-config files.
    let query = std::process::Command::new("qmake6")
        .arg("-query")
        .output()
        .expect("Renderer requires qmake6 to locate Qt development files");
    assert!(query.status.success(), "qmake6 -query failed");
    let metadata = String::from_utf8(query.stdout).expect("Qt metadata must be UTF-8");
    let properties: std::collections::HashMap<_, _> = metadata
        .lines()
        .filter_map(|line| line.split_once(':'))
        .collect();
    let property = |name| {
        properties
            .get(name)
            .unwrap_or_else(|| panic!("qmake6 did not report {name}"))
            .trim()
    };
    let version: Vec<_> = property("QT_VERSION").split('.').collect();
    assert!(
        version.first() == Some(&"6")
            && version.get(1).and_then(|minor| minor.parse::<u32>().ok()) >= Some(2),
        "Renderer requires Qt 6.2 or newer"
    );
    let headers = std::path::Path::new(property("QT_INSTALL_HEADERS"));
    let libraries = std::path::Path::new(property("QT_INSTALL_LIBS"));
    assert!(headers.is_absolute() && libraries.is_absolute());
    build.include(headers);
    println!("cargo:rustc-link-search=native={}", libraries.display());
    for module in ["Quick", "OpenGL", "Gui", "Core"] {
        build.include(headers.join(format!("Qt{module}")));
        println!("cargo:rustc-link-lib=Qt6{module}");
    }
    let jpeg = pkg_config::Config::new()
        .probe("libjpeg")
        .expect("Renderer requires libjpeg development files");
    for path in jpeg.include_paths {
        build.include(path);
    }
    build.compile("roonscape_window");
    for name in [
        "roonscape_frame_serial",
        "roonscape_frame_time_micros",
        "roonscape_submission_frame",
        "roonscape_submission_scene",
    ] {
        println!("cargo:rustc-link-arg=-Wl,--export-dynamic-symbol={name}");
    }
    println!("cargo:rerun-if-changed=native");
}
