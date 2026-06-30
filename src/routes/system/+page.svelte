<script lang="ts">
  import SystemView from "$lib/views/SystemView.svelte";
  import { getAppController } from "$lib/state/app.svelte";

  const app = getAppController();
</script>

<SystemView engineState={app.engineState} refreshKey={app.imageRefreshKey} />
