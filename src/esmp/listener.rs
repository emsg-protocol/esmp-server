use tokio::net::TcpListener;
use tokio::io::{AsyncBufReadExt, BufReader};
use crate::esmp::handler::handle_message;

pub async fn start_esmp_listener() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:5888").await?;
    println!("ESMP listener started on port 5888");

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("Accepted connection from {}", addr);
        tokio::spawn(async move {
            let mut reader = BufReader::new(socket);
            let mut buffer = String::new();
            loop {
                buffer.clear();
                let bytes_read = reader.read_line(&mut buffer).await.unwrap_or(0);
                if bytes_read == 0 {
                    break;
                }
                handle_message(&buffer).await;
            }
        });
    }
}
