# ludownloader
A download manager with Rust backend with a simple and open API to allow multiple client implementations.

---
## Downloader:
- [x] Downloading files
- [x] REST API with integration tests.
- [ ] Download-packaging
- [ ] Storing download metadata on disk as a persistence layer
- [ ] A way to manage multiple proxied `reqwest::Client` for Downloads
- [ ] Premium download hoster implementations (e.g. rapidgator, uploaded, ...) on top of the HttpDownload module
- [ ] Managing credentials
- [ ] Module p2pdownload
- [ ] Module Hyperdownload (Downloading multiple download-parts in parallel with different proxies to bypass server speed-limits)
