use zbus::proxy;
#[proxy(
    interface = "com.example.clippy.Daemon",
    default_service = "com.example.clippy",
    default_path = "/com/example/clippy"
)]
pub trait ClippyDaemon {
    fn new_entry(&self, text: String) -> zbus::Result<()>;
    fn get_history(&self) -> zbus::Result<Vec<String>>;
    fn delete(&self, id: i64) -> zbus::Result<()>;
    fn set_pinned(&self, id: i64, pinned: bool) -> zbus::Result<()>;
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
    pub fn delete(&self, id: i64) -> zbus::Result<()> {
        self.rt.block_on(async { self.proxy.delete(id).await })
    }
    pub fn set_pinned(&self, id: i64, pinned: bool) -> zbus::Result<()> {
        self.rt.block_on(async { self.proxy.set_pinned(id, pinned).await })
    }
}
