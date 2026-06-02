fn main() {
    // Provide a custom Windows app manifest so the embedded manifest carries an
    // explicit `asInvoker` execution level — Tauri's default manifest omits it,
    // which lets Windows installer-detection force a UAC prompt on our
    // "...installer.exe" filename (os error 740 under `cargo run`).
    let attributes = tauri_build::Attributes::new().windows_attributes(
        tauri_build::WindowsAttributes::new()
            .app_manifest(include_str!("windows-app-manifest.xml")),
    );
    tauri_build::try_build(attributes).expect("failed to run tauri_build");
}
