fn main() {
    // Compile GLib resources (CSS, UI files, icons) into the binary.
    glib_build_tools::compile_resources(
        &["src/ui/resources"],
        "resources.gresource.xml",
        "clippy.gresource",
    );
}