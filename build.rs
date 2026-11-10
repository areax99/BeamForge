//! On Windows, put the icon (assets/icon.ico) and the program details inside beamforge.exe.
//! Other systems get their icon from the app bundle (macOS) or the .desktop file (Linux).

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=assets/document.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        // icon resource 1: the program; 2: saved .beamforge projects (see src/fileicon.rs)
        res.set_icon_with_id("assets/icon.ico", "1");
        res.set_icon_with_id("assets/document.ico", "2");
        res.set("ProductName", "BeamForge");
        res.set("FileDescription", "BeamForge - free laser software");
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed the Windows icon: {e}");
        }
    }
}
