<script lang="ts">
  // Single mounted host that renders the shared confirm dialog. Mount once near
  // the app root; everything else calls confirmDialog() from state/confirm.
  import * as AlertDialog from "$lib/components/ui/alert-dialog/index.js";
  import { buttonVariants } from "$lib/components/ui/button/index.js";
  import { cn } from "$lib/utils.js";
  import { confirmStore } from "../state/confirm.svelte.js";

  const req = $derived(confirmStore.current);
</script>

<AlertDialog.Root
  bind:open={confirmStore.open}
  onOpenChange={(o) => {
    // Escape / overlay dismiss closes without an explicit choice — treat as cancel.
    if (!o) confirmStore.cancel();
  }}
>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>{req?.title ?? ""}</AlertDialog.Title>
      {#if req?.description}
        <AlertDialog.Description class="whitespace-pre-line">
          {req.description}
        </AlertDialog.Description>
      {/if}
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel onclick={() => confirmStore.cancel()}>
        {req?.cancelText ?? "Cancel"}
      </AlertDialog.Cancel>
      <AlertDialog.Action
        class={req?.destructive
          ? cn(buttonVariants({ variant: "destructive" }))
          : undefined}
        onclick={() => confirmStore.accept()}
      >
        {req?.confirmText ?? "Confirm"}
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
