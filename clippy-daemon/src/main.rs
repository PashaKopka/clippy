mod dbus;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    // Initialize DB connection
    let conn = clippy_db::open().expect("Failed to open DB");

    // Share connection using std::sync::Mutex or better yet for zbus we can just pass it if we manage concurrency
    // Or we keep it open in the dbus handler. Wait, `clippy_db::open()` can be called safely maybe? SQLite handles concurrent connections gracefully, but better to share it.
    let shared_conn = std::sync::Arc::new(std::sync::Mutex::new(conn));
    let service = dbus::ClippyDaemon { conn: shared_conn };

    let _conn = zbus::connection::Builder::session()
        .expect("No session D-Bus")
        .name("com.example.clippy")
        .expect("Failed to claim D-Bus name — is another instance running?")
        .serve_at("/com/example/clippy", service)
        .expect("Failed to serve D-Bus object")
        .build()
        .await
        .expect("Failed to build D-Bus connection");

    println!("[dbus] service running");

    std::future::pending::<()>().await;

    Ok(())
}
