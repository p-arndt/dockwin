<script lang="ts">
  import SettingsView from "$lib/views/SettingsView.svelte";
  import { getAppController } from "$lib/state/app.svelte";

  const app = getAppController();
</script>

<SettingsView
  working={app.working}
  bind:withBackup={app.withBackup}
  engineState={app.engineState}
  engineLine={app.engineLine}
  engineTone={app.engineTone}
  engineBusy={app.engineBusy}
  engineToggleDisabled={app.engineToggleDisabled}
  onToggleEngine={() => app.toggleEngine()}
  onRestartEngine={() => app.restartEngine()}
  onTeardown={() => app.teardownEngine()}
/>
