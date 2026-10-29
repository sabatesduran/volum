#[tokio::main]
async fn main() {
    if let Err(error) = volum_lib::run_headless().await {
        eprintln!("volumd: {error}");
        std::process::exit(1);
    }
}
