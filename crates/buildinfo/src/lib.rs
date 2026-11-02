//! Used from `build.rs` of each Tellshot binary.
//!
//! On Windows targets it compiles a resource file that gives the executable:
//! - the Tellshot icon (Explorer, taskbar, Alt+Tab, installer shortcuts),
//! - version information (Properties → Details, crash reports, enterprise inventory),
//! - an application manifest: per-monitor DPI awareness (sharp UI on high-DPI screens),
//!   UTF-8 as the process code page, long-path support, and Common Controls v6 so native
//!   dialogs use the modern style.
//!
//! On other targets it does nothing.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// What to embed into one executable.
#[derive(Debug, Clone, Copy)]
pub struct Resources<'a> {
    /// Shown in Task Manager and file properties, for example "Tellshot editor".
    pub description: &'a str,
    /// File name of the executable, for example "tellshot-editor.exe".
    pub file_name: &'a str,
}

/// Embeds Windows resources when building for Windows; a no-op elsewhere.
///
/// # Errors
/// When the resource file cannot be written or compiled.
pub fn embed(resources: Resources<'_>) -> Result<(), String> {
    cargo("rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return Ok(());
    }
    let manifest_dir = PathBuf::from(env("CARGO_MANIFEST_DIR")?);
    let out_dir = PathBuf::from(env("OUT_DIR")?);
    let icon = workspace_root(&manifest_dir).join("assets").join("icons").join("tellshot.ico");
    cargo(&format!("rerun-if-changed={}", icon.display()));

    let version = env("CARGO_PKG_VERSION")?;
    let numeric = numeric_version(&version);
    let manifest_path = out_dir.join("tellshot.manifest");
    write(&manifest_path, MANIFEST)?;
    let rc = resource_script(&resources, &version, &numeric, &icon, &manifest_path)?;
    let rc_path = out_dir.join("tellshot.rc");
    write(&rc_path, &rc)?;
    embed_resource::compile(&rc_path, embed_resource::NONE)
        .manifest_required()
        .map_err(|e| e.to_string())
}

/// Emits a directive to Cargo; build scripts talk to Cargo through stdout.
fn cargo(directive: &str) {
    #[expect(clippy::print_stdout, reason = "the build-script protocol is stdout")]
    {
        println!("cargo:{directive}");
    }
}

fn env(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|e| format!("{name}: {e}"))
}

fn workspace_root(manifest_dir: &Path) -> PathBuf {
    manifest_dir.ancestors().nth(2).map_or_else(|| manifest_dir.to_path_buf(), Path::to_path_buf)
}

/// "0.1.0-rc.1" → "0,1,0,0" (Windows wants four integers).
fn numeric_version(version: &str) -> String {
    let core = version.split(['-', '+']).next().unwrap_or("0");
    let mut parts: Vec<u16> = core.split('.').map(|p| p.parse().unwrap_or(0)).collect();
    parts.resize(4, 0);
    parts.iter().map(u16::to_string).collect::<Vec<_>>().join(",")
}

/// Paths in .rc files are C strings: escape backslashes.
fn rc_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}

fn resource_script(
    r: &Resources<'_>,
    version: &str,
    numeric: &str,
    icon: &Path,
    manifest: &Path,
) -> Result<String, String> {
    let mut rc = String::new();
    let lines = [
        "#pragma code_page(65001)".to_owned(),
        format!("1 ICON \"{}\"", rc_path(icon)),
        format!("1 24 \"{}\"", rc_path(manifest)),
        "1 VERSIONINFO".to_owned(),
        format!("FILEVERSION {numeric}"),
        format!("PRODUCTVERSION {numeric}"),
        "FILEFLAGSMASK 0x3fL".to_owned(),
        "FILEOS 0x40004L".to_owned(),
        "FILETYPE 0x1L".to_owned(),
        "BEGIN".to_owned(),
        "  BLOCK \"StringFileInfo\"".to_owned(),
        "  BEGIN".to_owned(),
        "    BLOCK \"040904b0\"".to_owned(),
        "    BEGIN".to_owned(),
        "      VALUE \"CompanyName\", \"Tellshot contributors\"".to_owned(),
        format!("      VALUE \"FileDescription\", \"{}\"", r.description),
        format!("      VALUE \"FileVersion\", \"{version}\""),
        format!("      VALUE \"InternalName\", \"{}\"", r.file_name),
        "      VALUE \"LegalCopyright\", \"MIT OR Apache-2.0\"".to_owned(),
        format!("      VALUE \"OriginalFilename\", \"{}\"", r.file_name),
        "      VALUE \"ProductName\", \"Tellshot\"".to_owned(),
        format!("      VALUE \"ProductVersion\", \"{version}\""),
        "    END".to_owned(),
        "  END".to_owned(),
        "  BLOCK \"VarFileInfo\"".to_owned(),
        "  BEGIN".to_owned(),
        "    VALUE \"Translation\", 0x409, 1200".to_owned(),
        "  END".to_owned(),
        "END".to_owned(),
    ];
    for line in lines {
        writeln!(rc, "{line}").map_err(|e| e.to_string())?;
    }
    Ok(rc)
}

fn write(path: &Path, contents: &str) -> Result<(), String> {
    #[expect(clippy::disallowed_methods, reason = "build output in OUT_DIR, not user data")]
    std::fs::write(path, contents).map_err(|e| format!("write {}: {e}", path.display()))
}

const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity type="win32" name="Tellshot" version="1.0.0.0"/>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0"
        processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
      <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
      <longPathAware xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">true</longPathAware>
    </windowsSettings>
  </application>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_become_four_windows_integers() {
        assert_eq!(numeric_version("0.1.0"), "0,1,0,0", "plain");
        assert_eq!(numeric_version("1.2.3-rc.1"), "1,2,3,0", "pre-release suffix dropped");
        assert_eq!(numeric_version("2.0.0+build.7"), "2,0,0,0", "build metadata dropped");
    }

    #[test]
    fn resource_script_escapes_windows_paths() {
        let r = Resources { description: "Tellshot editor", file_name: "tellshot-editor.exe" };
        let rc = resource_script(
            &r,
            "0.1.0",
            "0,1,0,0",
            Path::new(r"C:\src\tellshot\assets\icons\tellshot.ico"),
            Path::new(r"C:\out\tellshot.manifest"),
        )
        .unwrap();
        assert!(rc.contains(r#"1 ICON "C:\\src\\tellshot\\assets\\icons\\tellshot.ico""#), "{rc}");
        assert!(rc.contains("FILEVERSION 0,1,0,0"), "{rc}");
        assert!(rc.contains(r#"VALUE "OriginalFilename", "tellshot-editor.exe""#), "{rc}");
    }
}
