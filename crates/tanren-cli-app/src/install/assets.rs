#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AssetKind {
    Generated,
    PreservedStandard,
}

pub(super) struct AssetSpec {
    pub(super) kind: AssetKind,
    pub(super) rel_path: &'static str,
    pub(super) expected_content: Option<&'static str>,
}

pub(super) fn asset_catalog() -> Vec<AssetSpec> {
    let generated = [
        (
            ".claude/commands/architect-system.md",
            include_str!("../../../../commands/project/architect-system.md"),
        ),
        (
            ".claude/commands/craft-roadmap.md",
            include_str!("../../../../commands/project/craft-roadmap.md"),
        ),
        (
            ".claude/commands/identify-behaviors.md",
            include_str!("../../../../commands/project/identify-behaviors.md"),
        ),
        (
            ".claude/commands/plan-product.md",
            include_str!("../../../../commands/project/plan-product.md"),
        ),
    ];

    let preserved = ["docs/standards/global/tech-stack.md"];

    let mut catalog = Vec::with_capacity(generated.len() + preserved.len());

    for (path, content) in generated {
        catalog.push(AssetSpec {
            kind: AssetKind::Generated,
            rel_path: path,
            expected_content: Some(content),
        });
    }

    for path in preserved {
        catalog.push(AssetSpec {
            kind: AssetKind::PreservedStandard,
            rel_path: path,
            expected_content: None,
        });
    }

    catalog
}
