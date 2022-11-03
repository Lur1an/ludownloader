<script lang="ts">
  import Greet from './components/Greet.svelte'
</script>

<main class="animate-bounce">
  <h1>Welcome to Tauri!</h1>

  <div>
    <!-- svelte-ignore security-anchor-rel-noreferrer -->
    <a href="https://vitejs.dev" target="_blank">
      <img src="/vite.svg" class="animate-spin" alt="Vite Logo" />
    </a>
    <!-- svelte-ignore security-anchor-rel-noreferrer -->
    <a href="https://tauri.app" target="_blank">
      <img src="/tauri.svg" class="animate-spin" alt="Tauri Logo" />
    </a>
    <!-- svelte-ignore security-anchor-rel-noreferrer -->
    <a href="https://svelte.dev" target="_blank">
      <img src="/svelte.svg" class="animate-spin" alt="Svelte Logo" />
    </a>
  </div>

  <p>
    Click on the Tauri, Vite, and Svelte logos to learn more.
  </p>

  <div>
    <Greet header=345/>
  </div>

</main>