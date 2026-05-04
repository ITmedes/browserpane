<script lang="ts">
  let { children } = $props();
</script>

{@render children()}

<style>
  :global(:root) {
    color: #182018;
    background: #eef1df;
    font-family: "Aptos", "IBM Plex Sans", "Segoe UI", sans-serif;
    font-synthesis: none;
    text-rendering: optimizeLegibility;
  }

  :global(body) {
    min-width: 320px;
    min-height: 100vh;
    margin: 0;
    background:
      radial-gradient(circle at 15% 15%, rgba(219, 107, 59, 0.22), transparent 28rem),
      radial-gradient(circle at 82% 8%, rgba(65, 116, 99, 0.2), transparent 24rem),
      linear-gradient(135deg, #f5f1df 0%, #e7ebd5 48%, #d6dfca 100%);
  }

  :global(*) {
    box-sizing: border-box;
  }
</style>
