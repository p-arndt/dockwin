<script lang="ts">
  import NetworksView from "$lib/views/NetworksView.svelte";
  import { getAppController } from "$lib/state/app.svelte";

  const app = getAppController();
</script>

<div class="body"><NetworksView engineState={app.engineState} refreshKey={app.imageRefreshKey} /></div>
