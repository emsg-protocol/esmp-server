mod esmp;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    esmp::listener::start_esmp_listener().await
}
