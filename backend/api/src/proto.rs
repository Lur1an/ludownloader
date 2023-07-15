use uuid::Uuid;

include!(concat!(env!("OUT_DIR"), "/proto.rs"));

impl DownloadMetadata {
    /// Returns the UUID of the download by parsing the raw bytes
    /// This function is unsafe, it unwraps inside.
    pub fn id(&self) -> Uuid {
        Uuid::from_slice(&self.uuid).unwrap()
    }
}
