pub mod steps;
pub mod world;

use cucumber::World as _;

use crate::world::BehaviorWorld;

pub async fn run_from_env() {
    let feature_path =
        std::env::var("TANREN_BDD_FEATURE_PATH").unwrap_or_else(|_| "tests/bdd/features".into());
    run_path(feature_path).await;
}

pub async fn run_path(feature_path: String) {
    BehaviorWorld::cucumber()
        .max_concurrent_scenarios(1)
        .fail_on_skipped()
        .run_and_exit(feature_path)
        .await;
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn behavior_suite() {
        crate::run_from_env().await;
    }
}
