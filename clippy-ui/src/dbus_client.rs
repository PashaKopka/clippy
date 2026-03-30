use zbus::proxy;
#[proxy(
    interface = "com.example.clippy.Daemon",
    default_service = "com.example.clippy",
    default_path = "/com/example/clippy"
)]
pub trait ClippyDaemon {
    fn new_entry(&self, text: String) -> zbus::Result<()>;
    fn get_history(&self) -> zbus::Result<Vec<String>>;
    fn get_image_bytes(&self, id: i64) -> zbus::Result<Vec<u8>>;
    fn delete(&self, id: i64) -> zbus::Result<()>;
    fn set_pinned(&self, id: i64, pinned: bool) -> zbus::Result<()>;
    fn toggle_pin(&self, id: i64) -> zbus::Result<()>;

    #[zbus(signal)]
    fn history_changed(&self) -> zbus::Result<()>;
}

pub struct DbusClient {
    proxy: ClippyDaemonProxy<'static>,
    rt: tokio::runtime::Runtime,
}

impl DbusClient {
    pub fn new() -> zbus::Result<Self> {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let proxy = rt.block_on(async {
            let conn = zbus::connection::Builder::session()?.build().await?;
            ClippyDaemonProxy::new(&conn).await
        })?;
        Ok(Self { proxy, rt })
    }
    pub fn new_entry(&self, text: String) -> zbus::Result<()> {
        self.rt.block_on(async { self.proxy.new_entry(text).await })
    }
    pub fn get_history(&self) -> zbus::Result<Vec<String>> {
        self.rt.block_on(async { self.proxy.get_history().await })
    }
    pub fn get_image_bytes(&self, id: i64) -> zbus::Result<Vec<u8>> {
        self.rt.block_on(async { self.proxy.get_image_bytes(id).await })
    }
    pub fn delete(&self, id: i64) -> zbus::Result<()> {
        self.rt.block_on(async { self.proxy.delete(id).await })
    }
    pub fn set_pinned(&self, id: i64, pinned: bool) -> zbus::Result<()> {
        self.rt
            .block_on(async { self.proxy.set_pinned(id, pinned).await })
    }
    
    pub fn toggle_pin(&self, id: i64) -> zbus::Result<()> {
        self.rt.block_on(async { self.proxy.toggle_pin(id).await })
    }

    pub fn request_image_bytes_async(&self, id: i64, tx: async_channel::Sender<Vec<u8>>) {
        let proxy = self.proxy.clone();
        self.rt.spawn(async move {
            if let Ok(bytes) = proxy.get_image_bytes(id).await {
                let _ = tx.send(bytes).await;
            }
        });
    }
    
    pub fn spawn_history_changed_listener(&self, callback: impl Fn() + Send + 'static) -> zbus::Result<()> {
        let mut stream = self.rt.block_on(async {
            self.proxy.receive_history_changed().await
        })?;
        self.rt.spawn(async move {
            use futures_util::StreamExt;
            while let Some(_) = stream.next().await {
                callback();
            }
        });
        Ok(())
    }
}
