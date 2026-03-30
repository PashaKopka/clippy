fn main() {
    // Compile GLib resources (CSS, UI files, icons) into the binary.
    glib_build_tools::compile_resources(
        &["."],
        "resources.gresource.xml",
        "clippy.gresource",
    );
}
