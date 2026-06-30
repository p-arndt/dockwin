<script lang="ts">
  import ImagesView from "$lib/views/ImagesView.svelte";
  import { getAppController } from "$lib/state/app.svelte";

  const app = getAppController();
</script>

<ImagesView engineState={app.engineState} refreshKey={app.imageRefreshKey} />
