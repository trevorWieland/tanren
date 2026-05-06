use crate::install::assets;

pub(crate) struct ResolvedProfile {
    pub(crate) standards: Vec<assets::EmbeddedFile>,
}

pub(crate) fn resolve(profile_name: &str) -> ResolvedProfile {
    let standards = assets::profile_files(profile_name);
    ResolvedProfile { standards }
}
