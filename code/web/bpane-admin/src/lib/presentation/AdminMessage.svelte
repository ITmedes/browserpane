<script lang="ts">
  import { AlertCircle, CheckCircle2, Info, LoaderCircle, TriangleAlert, X } from 'lucide-svelte';
  import { resolveAdminMessageAccessibility, type AdminMessageRole } from './admin-message-accessibility';
  import type { AdminMessageVariant } from './admin-message-types';

  type AdminMessageProps = {
    readonly variant?: AdminMessageVariant;
    readonly title?: string;
    readonly message: string;
    readonly testId?: string;
    readonly compact?: boolean;
    readonly role?: AdminMessageRole;
    readonly onDismiss?: () => void;
    readonly dismissLabel?: string;
    readonly dismissTestId?: string;
  };

  type MessageTone = {
    readonly icon: typeof Info;
    readonly rootClass: string;
    readonly iconClass: string;
    readonly titleClass: string;
    readonly messageClass: string;
  };

  const MESSAGE_TONES = {
    info: {
      icon: Info,
      rootClass: 'border-[#90a6cc]/34 bg-[#172946] text-admin-ink shadow-[0_14px_34px_rgb(0_0_0_/_28%)]',
      iconClass: 'text-[#9fb1cf]',
      titleClass: 'text-admin-ink',
      messageClass: 'text-[#d9e5fa]',
    },
    success: {
      icon: CheckCircle2,
      rootClass: 'border-admin-leaf/52 bg-[#102f2c] text-admin-ink shadow-[0_14px_34px_rgb(0_0_0_/_28%)]',
      iconClass: 'text-admin-leaf',
      titleClass: 'text-admin-ink',
      messageClass: 'text-[#d7fff4]',
    },
    warning: {
      icon: TriangleAlert,
      rootClass: 'border-admin-warm/56 bg-[#332a13] text-admin-ink shadow-[0_14px_34px_rgb(0_0_0_/_28%)]',
      iconClass: 'text-admin-warm',
      titleClass: 'text-admin-ink',
      messageClass: 'text-[#fff3c2]',
    },
    error: {
      icon: AlertCircle,
      rootClass: 'border-admin-danger/60 bg-[#351b24] text-admin-ink shadow-[0_14px_34px_rgb(0_0_0_/_28%)]',
      iconClass: 'text-admin-danger',
      titleClass: 'text-admin-ink',
      messageClass: 'text-[#ffc3b7]',
    },
    loading: {
      icon: LoaderCircle,
      rootClass: 'border-admin-leaf/44 bg-[#142d3a] text-admin-ink shadow-[0_14px_34px_rgb(0_0_0_/_28%)]',
      iconClass: 'text-admin-leaf',
      titleClass: 'text-admin-ink',
      messageClass: 'text-[#d9e5fa]',
    },
    empty: {
      icon: Info,
      rootClass: 'border-[#90a6cc]/26 bg-[#111f35] text-admin-ink shadow-[0_10px_24px_rgb(0_0_0_/_20%)]',
      iconClass: 'text-[#9fb1cf]',
      titleClass: 'text-admin-ink',
      messageClass: 'text-[#c1d0e8]',
    },
  } satisfies Record<AdminMessageVariant, MessageTone>;

  let {
    variant = 'info',
    title,
    message,
    testId,
    compact = false,
    role,
    onDismiss,
    dismissLabel = 'Dismiss message',
    dismissTestId,
  }: AdminMessageProps = $props();

  const tone = $derived(MESSAGE_TONES[variant]);
  const MessageIcon = $derived(tone.icon);
  const accessibility = $derived(resolveAdminMessageAccessibility(variant, role));
  const rootClass = $derived([
    'grid w-full min-w-0 items-start gap-3 rounded-xl border border-l-[5px]',
    onDismiss ? 'grid-cols-[auto_minmax(0,1fr)_auto]' : 'grid-cols-[auto_minmax(0,1fr)]',
    compact ? 'min-h-[52px] p-3' : 'min-h-[68px] p-4',
    tone.rootClass,
  ].join(' '));
  const iconClass = $derived([
    'mt-0.5 shrink-0',
    tone.iconClass,
    variant === 'loading' ? 'animate-spin' : '',
  ].filter(Boolean).join(' '));
</script>

<section
  class={rootClass}
  role={accessibility.role}
  aria-live={accessibility.ariaLive}
  aria-atomic={accessibility.ariaAtomic}
  data-testid={testId}
>
  <MessageIcon class={iconClass} size={compact ? 16 : 18} aria-hidden="true" />
  <div class="grid min-w-0 gap-1">
    {#if title}
      <strong class={`text-sm font-extrabold leading-snug ${tone.titleClass}`}>{title}</strong>
    {/if}
    <p class={`m-0 min-w-0 text-sm font-semibold leading-normal [overflow-wrap:anywhere] ${tone.messageClass}`}>
      {message}
    </p>
  </div>
  {#if onDismiss}
    <button
      class="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-current/14 text-current/70 transition hover:bg-white/8 hover:text-current"
      type="button"
      aria-label={dismissLabel}
      title={dismissLabel}
      data-testid={dismissTestId}
      onclick={onDismiss}
    >
      <X size={15} aria-hidden="true" />
    </button>
  {/if}
</section>
