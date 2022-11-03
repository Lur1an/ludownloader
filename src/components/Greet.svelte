<script lang="ts">
  import { invoke } from "@tauri-apps/api/tauri"
  export let header: string = "";

  let name = "";
  let greetMsg = ""
  let realMessage = ""
  $: if (greetMsg) {
    realMessage = greetMsg + " asshole..."
  }

  async function greet() {
    // Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
    greetMsg = await invoke("greet", { name })
  }

  let pressedKeyMap = new Map<string, boolean>()
  let pressedKeys = ""
  let displayPressedKeys = false;

  function addKey(e: KeyboardEvent) {
    pressedKeyMap = pressedKeyMap.set(e.key, true)
  }

  function removeKey(e: KeyboardEvent) {
    pressedKeyMap = pressedKeyMap.set(e.key, false)
  }

  $: {
    console.log("Updating keys")
    let keyArr = []
    console.log(pressedKeyMap)
    for (const [key, value] of pressedKeyMap) {
      if (value) keyArr.push(key)
    }
    console.log(keyArr)
    pressedKeys = keyArr.join(', ')
    displayPressedKeys = keyArr.length != 0
  }


</script>

<svelte:window on:keydown={addKey} on:keyup={removeKey}/>
<div>
  <div class="row">
    <input id="greet-input" placeholder="Enter a name..." bind:value={name} />
    <button on:click={greet}>
      Greet
    </button>
  </div>
  <p>{realMessage}</p>
  <p>You are holding : {pressedKeys}</p>

  <p>This is my Header given by App: {header}</p>
  
</div>