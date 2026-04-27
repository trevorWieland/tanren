#[tokio::main(flavor = "current_thread")]
async fn main() {
    tanren_bdd::run_from_env().await;
}
