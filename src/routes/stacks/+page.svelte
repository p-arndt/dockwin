<script lang="ts">
  import StacksView from "$lib/views/StacksView.svelte";
  import { getAppController } from "$lib/state/app.svelte";

  const app = getAppController();
</script>

<StacksView
  stacks={app.stacks}
  pending={app.pending}
  engineState={app.engineState}
  errorMsg={app.errorMsg}
  compose={app.compose}
  onStackAction={(action, stack) => app.handleStackAction(action, stack)}
  setFooter={app.setFooter}
/>
