<script lang="ts">
  import ContainersView from "$lib/views/ContainersView.svelte";
  import { getAppController } from "$lib/state/app.svelte";

  const app = getAppController();
</script>

<ContainersView
  containers={app.containers}
  pending={app.pending}
  runningCount={app.runningCount}
  errorMsg={app.errorMsg}
  selected={app.selectedContainer}
  detailFull={app.detailFull}
  loaded={app.containersLoaded}
  onAction={(action, c) => app.handleAction(action, c)}
  onBulkAction={(action, cs) => app.handleBulkAction(action, cs)}
  onSelect={(c) => app.selectContainer(c)}
  onCloseDetail={() => app.closeDetail()}
  onToggleFull={() => app.toggleDetailFull()}
/>
