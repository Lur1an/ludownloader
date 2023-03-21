# ludownloader
The idea for this repo is to help me practice rust, svelte and software architecture overall. The idea came to me when I got sick of JDownloader being outdated and not rendering correctly on 4k screens and on linux.

The plan is to implement the components one at a time (downloader-core, account-manager, link-crawler, download-manager) and then expose a GUI to interact with them over a Svelte frontend. (Initial idea was Tauri, but after having experienced the use-case of deploying a downloader on something like an OrangePI5 I want the GUI to be accessible over the local network)
