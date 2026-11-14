//! On Windows, put the icon (assets/icon.ico) and the program details inside rayetch.exe.
//! Other systems get their icon from the app bundle (macOS) or the .desktop file (Linux).

/// Build time in UTC as "yyyy-MM-dd @ hh:mm:ss", the format of the original's About page
/// (HelpDialog::HelpDialog 0x100c1a2b8, "Built on: ... UTC").
fn build_utc() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // civil-from-days (proleptic Gregorian calendar)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02} @ {:02}:{:02}:{:02}", rem / 3600, rem / 60 % 60, rem % 60)
}

fn main() {
    // rebuild the date whenever the code changes
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rustc-env=RAYETCH_BUILD_UTC={}", build_utc());
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=assets/document.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        // icon resource 1: the program; 2: saved .rayetch projects (see src/fileicon.rs)
        res.set_icon_with_id("assets/icon.ico", "1");
        res.set_icon_with_id("assets/document.ico", "2");
        res.set("ProductName", "RayEtch");
        res.set("FileDescription", "RayEtch - free laser software");
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed the Windows icon: {e}");
        }
    }
}
