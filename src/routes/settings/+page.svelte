<script lang="ts">
  import SettingsView from "$lib/views/SettingsView.svelte";
  import { getAppController } from "$lib/state/app.svelte";

  const app = getAppController();
</script>

<SettingsView working={app.working} bind:withBackup={app.withBackup} onTeardown={() => app.teardownEngine()} />
