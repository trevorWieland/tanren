use include_dir::{Dir, DirEntry, include_dir};

static COMMANDS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../commands");
static PROFILES_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../profiles");

pub(crate) struct EmbeddedFile {
    pub(crate) relative_path: String,
    pub(crate) content: &'static [u8],
}

pub(crate) fn command_files() -> Vec<EmbeddedFile> {
    let mut files = Vec::new();
    collect_files(&COMMANDS_DIR, "commands", &mut files, is_command_file);
    files
}

pub(crate) fn profile_files(profile_name: &str) -> Vec<EmbeddedFile> {
    let mut files = Vec::new();

    if let Some(default_dir) = PROFILES_DIR.get_dir("default") {
        collect_files(default_dir, "standards", &mut files, is_markdown);
    }

    if profile_name != "default" {
        if let Some(profile_dir) = PROFILES_DIR.get_dir(profile_name) {
            collect_files(profile_dir, "standards", &mut files, is_markdown);
        }
    }

    files
}

fn is_command_file(path: &std::path::Path) -> bool {
    path.starts_with("project") && path.extension().is_some_and(|ext| ext == "md")
}

fn is_markdown(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|ext| ext == "md")
}

fn collect_files(
    dir: &'static Dir<'_>,
    prefix: &str,
    out: &mut Vec<EmbeddedFile>,
    filter: fn(&std::path::Path) -> bool,
) {
    for entry in dir.entries() {
        match entry {
            DirEntry::Dir(d) => {
                collect_files(d, prefix, out, filter);
            }
            DirEntry::File(f) => {
                if filter(f.path()) {
                    let relative = format!("{prefix}/{}", f.path().display());
                    out.push(EmbeddedFile {
                        relative_path: relative,
                        content: f.contents(),
                    });
                }
            }
        }
    }
}
