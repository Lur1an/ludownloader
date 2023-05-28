# ludownloader
A download manager with Rust backend and Svelte browser-frontend

---
## Downloader:
The `downloader` crate has also a `p2pdownload` module planned to try and implement a torrent client.
- [x] Downloading files
- [ ] Premium download hoster implementations (e.g. rapidgator, uploaded, ...)
- [ ] Managing credentials
- [ ] unpacking of archives (not a priority)
## Server
The `server` crate is a webserver that initializes the managers and provides an API to interact with them, the communication protocol will be some form of binary-encoding of data structures.
