<script lang="ts">
  import VolumesView from "$lib/views/VolumesView.svelte";
  import { getAppController } from "$lib/state/app.svelte";

  const app = getAppController();
</script>

<div class="body"><VolumesView engineState={app.engineState} refreshKey={app.imageRefreshKey} /></div>
