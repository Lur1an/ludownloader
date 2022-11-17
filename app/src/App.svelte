<script lang="ts">
  import { invoke } from '@tauri-apps/api/tauri'
	import { onMount } from 'svelte';
  let downloadUrl = "";
  async function downloadFile() {
    invoke("quick_download", {"url": downloadUrl})
      .then((msg: String) => alert(msg))
      .catch((error: String) => alert(error))
    downloadUrl = ""
  }
</script>

<main class="flex-col">
  <h1>Hello Svelte</h1>
  <input class="w-96 bg-slate-500" bind:value={downloadUrl}>
  <button on:click={downloadFile} class="bg-white font-mono ">
    DOWNLOAD
  </button>

</main>