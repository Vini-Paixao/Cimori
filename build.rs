// Embeds the app icon and the version info (Task Manager, file properties) when building for Windows.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/brand/cimori/windows/cimori.ico");
    let v = |part: &str| std::env::var(format!("CARGO_PKG_VERSION_{part}")).unwrap_or_else(|_| "0".into());
    let (major, minor, patch) = (v("MAJOR"), v("MINOR"), v("PATCH"));
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let icon = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets/brand/cimori/windows/cimori.ico");
    // A generated .rc: the version comes from Cargo.toml, the icon by its full path.
    let rc = format!(r#"#pragma code_page(65001)
1 ICON "{icon}"
1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "Marcus Paixão"
      VALUE "FileDescription", "Cimori"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "Cimori"
      VALUE "LegalCopyright", "Marcus Paixão · GPL-3.0"
      VALUE "OriginalFilename", "Cimori.exe"
      VALUE "ProductName", "Cimori"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#, icon = icon.display().to_string().replace('\\', "\\\\"));
    let out = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("cimori.rc");
    std::fs::write(&out, rc).unwrap();
    embed_resource::compile(&out, embed_resource::NONE).manifest_optional().unwrap();
}
