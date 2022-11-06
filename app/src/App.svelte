<script lang="ts">
	import { onMount } from "svelte";
  import { invoke } from '@tauri-apps/api/tauri'

  onMount(() => {
    invoke("greet", {name: "Ass"});
  })
</script>

<main class="">
  <h1>Welcome to Ludownloader</h1>
</main>