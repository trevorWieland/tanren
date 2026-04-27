use std::collections::HashSet;
use std::sync::Arc;

use crate::execution::SecretName;

use super::{CompiledSecretMatcher, DefaultOutputRedactor, RedactionHints};

#[derive(Debug, Clone)]
pub(super) struct CompiledHintArtifacts {
    pub(super) source_hints: RedactionHints,
    pub(super) hint_keys: Arc<HashSet<String>>,
    pub(super) secret_matcher: Arc<CompiledSecretMatcher>,
}

impl DefaultOutputRedactor {
    pub(super) fn compiled_hint_artifacts(&self, hints: &RedactionHints) -> CompiledHintArtifacts {
        if let Some(cached) = self
            .cached_hint_artifacts
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .filter(|cached| cached.source_hints == *hints)
            .cloned()
        {
            return cached;
        }

        let compiled = CompiledHintArtifacts {
            source_hints: hints.clone(),
            hint_keys: Arc::new(
                hints
                    .required_secret_names
                    .iter()
                    .map(SecretName::as_str)
                    .map(str::to_owned)
                    .collect(),
            ),
            secret_matcher: Arc::new(CompiledSecretMatcher::from_hints(
                hints,
                self.policy.min_secret_fragment_len(),
            )),
        };

        *self
            .cached_hint_artifacts
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(compiled.clone());
        compiled
    }
}
