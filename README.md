# ludownloader
A download manager with Rust backend and Svelte browser-frontend

---
## Downloader:
- [x] Downloading files
- [ ] A Frontend
- [ ] Download-packaging
- [ ] A way to manage multiple proxied `reqwest::Client` for Downloads
- [ ] Premium download hoster implementations (e.g. rapidgator, uploaded, ...)
- [ ] Managing credentials
- [ ] Module p2pdownload
- [ ] Module Hyperdownload (Downloading multiple download-parts in parallel with potentially different proxies to bypass server speed-limits)
- [ ] Link-crawler/grabber
## Server
The `server` crate is a webserver that initializes the managers and provides an API to interact with them, the communication protocol will be some form of binary-encoding of data structures.
