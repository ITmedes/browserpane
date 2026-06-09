# Pattern Library

Yes, the new admin app should plan a small internal pattern library. This is not
a separate public design system. It is a code-level reuse layer inside the new
admin app to keep repeated operational UI patterns consistent.

## Recommendation

Start with an internal pattern route and component folder:

- `code/web/bpane-admin-unified/src/lib/patterns`
- optional internal route: `/admin-new/patterns`

Keep `/admin-new/patterns` dev/internal. It should help implementation and
review, not become a product route.

## Why It Matters

The current admin complexity comes partly from repeated local solutions for:

- resource lists
- selected-resource summaries
- compact action rows
- messages and feedback
- form sections
- API payload previews
- tabs
- detail metadata
- destructive action states
- file upload/download rows
- live browser container sizing

If these are not consolidated early, the new app will likely repeat the same
maintenance problem.

## Initial Patterns

Introduce patterns only when used by at least two routes, except for shell-level
patterns that are foundational.

Suggested initial patterns:

- `AppShell`: nav, header, breadcrumbs, auth state, global messages.
- `PageHeader`: title, subtitle, primary actions, status chips.
- `ResourceList`: searchable list with selected item state.
- `ResourceSummary`: selected item metadata and primary actions.
- `DetailTabs`: route-backed tab navigation.
- `ActionBar`: compact action grouping with disabled/reason states.
- `StatusBadge`: state, health, admission, and capability indicators.
- `FeedbackMessage`: success, info, warning, error with accessibility roles.
- `EmptyState`: empty list and missing resource states.
- `LoadingState`: route and panel loading states.
- `ErrorState`: recoverable API and auth/backend errors.
- `FormSection`: titled form blocks with validation messages.
- `FieldRow`: label, hint, control, validation, and compact layout.
- `PayloadPreview`: collapsible JSON preview that does not auto-collapse.
- `DangerZone`: destructive actions and confirmation copy.
- `FileDropUpload`: file input/drop/upload progress.
- `DownloadAction`: blob download state and error feedback.
- `LiveViewportFrame`: stable container sizing for browser canvas.
- `CopyButton`: copy-to-clipboard with feedback.
- `CommandPalette`: global route/action launcher.

## Rules

- Use Svelte components and local TypeScript view models.
- Do not introduce a third-party component framework unless there is a specific
  gap that local patterns cannot cover.
- Keep patterns behavior-light. Domain decisions stay in route/application
  services.
- Keep test IDs stable at the pattern boundary where smokes depend on them.
- Do not use nested cards.
- Keep compact operational density.
- Use icons for obvious tool actions.
- Keep accessible labels and keyboard behavior first-class.

## Pattern Library Acceptance

A pattern is useful only if it has:

- at least one real route consumer
- a basic component or view-model test where logic exists
- documented states
- loading, empty, disabled, and error behavior where relevant
- stable selector behavior if smoke scripts use it

## First Pattern Slice

Implement these with the shell scaffold:

1. `AppShell`
2. `PageHeader`
3. `FeedbackMessage`
4. `ResourceList`
5. `ResourceSummary`
6. `DetailTabs`
7. `ActionBar`
8. `LiveViewportFrame`

Defer specialized patterns until the first route actually needs them.
