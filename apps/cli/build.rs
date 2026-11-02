//! Embeds the icon, version information and manifest into `tellshot.exe` on Windows.

fn main() -> Result<(), String> {
    tellshot_buildinfo::embed(tellshot_buildinfo::Resources {
        description: "Tellshot command-line tool",
        file_name: "tellshot.exe",
    })
}
