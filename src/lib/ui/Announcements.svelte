<script lang="ts">
  import { announcer } from './announce.svelte';
</script>

<!-- Kept in the page and filled: what a reader is told without the focus moving (guidelines, 11). -->
<div class="spoken" role="status">{announcer.polite}</div>
<div class="spoken" role="alert">{announcer.urgent}</div>
