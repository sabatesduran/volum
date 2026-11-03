#[tokio::main]
async fn main() {
    init_logging();
    if let Err(error) = volum_server::run_headless().await {
        eprintln!("volumd: {error}");
        std::process::exit(1);
    }
}

fn init_logging() {
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let result = if std::env::var("VOLUM_LOG_FORMAT").as_deref() == Ok("json") {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .json()
            .try_init()
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).try_init()
    };
    if let Err(error) = result {
        eprintln!("volumd: could not initialize logging: {error}");
    }
}
