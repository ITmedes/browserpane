# NodeJS / TypeScript Standards

These standards apply to TypeScript and Node.js code across the BrowserPane workspace:

- `code/web/bpane-client`
- `code/integrations/mcp-bridge`
- future TypeScript or Node.js packages in this repo

Use this file with `AGENTS.md`. If this file and live code disagree, prefer the code, then runtime manifests, then this file.

## Intent

- Optimize for low entry cost for human developers.
- Prefer a Java-leaning style: explicit classes, explicit visibility, explicit boundaries.
- Maximize refactor safety, testability, and local reasoning.
- Prefer boring, explicit code over clever TypeScript tricks.
- Treat class-first public APIs, hard file-size caps, and `type`-first domain models as BrowserPane policy choices layered on top of broader TypeScript and Node.js guidance.

## Core Rules

- Every file must be a module. Do not rely on script files or global scope.
- No mutable global state.
- No global-modifying modules, prototype patching, or implicit side effects on import.
- Default public unit is a class, not an exported top-level function.
- Do not export top-level functions except for framework-required entrypoints, tiny pure adapters, or generated glue.
- Prefer named exports over default exports.
- Prefer composition over inheritance.
- Keep modules closed and explicit. Do not depend on declaration merging as an application pattern.

## Validation Baseline

TypeScript changes should normally pass the narrowest relevant set of:

- `cd code/web/bpane-client && npx tsc --noEmit`
- `cd code/web/bpane-client && npm test`
- `cd code/web/bpane-client && npm run build`
- `cd code/web/bpane-client && npm run test:coverage`
- `cd code/integrations/mcp-bridge && npm run build`

When linting is added or expanded, use typed linting rather than syntax-only linting.

## Project Structure

Organize TypeScript code around explicit layers and bounded contexts:

- `domain/`: domain types, value objects, invariants, domain services, domain errors
- `application/`: use cases, coordinators, orchestration, transaction boundaries
- `infra/`: browser APIs, Node APIs, transport, persistence, timers, logging, codecs
- `presentation/` or `ui/`: DOM rendering, event wiring, view models
- `test-support/`: builders, fakes, stubs, shared test helpers

Rules:

- Domain code must not depend on DOM APIs, Node APIs, fetch, timers, storage, logging, or transport details.
- Infrastructure may depend inward on domain and application; domain must never depend outward on infrastructure.
- Do not create a generic `shared/` dumping ground. Share only stable, generic primitives with clear ownership.
- Protocol and wire-format models are not domain models.

## Size And Layout

- Target `100` to `150` lines per source file and per class.
- Hard cap is `200` lines per file and per class unless there is a documented reason.
- Split a file when it has more than one reason to change.
- Keep one primary class per file.
- Keep methods short enough to scan without scrolling; target `30` lines or less.
- Constructors must stay lightweight and must not perform network, DOM subscription, or async startup work.

## Type System Baseline

Prefer a strict compiler configuration. For application packages, the baseline should include:

- `strict`
- `exactOptionalPropertyTypes`
- `noUncheckedIndexedAccess`
- `noImplicitOverride`
- `noImplicitReturns`
- `noFallthroughCasesInSwitch`
- `useUnknownInCatchVariables`
- `noPropertyAccessFromIndexSignature` where practical

Rules:

- Do not use `any` in production code.
- Use `unknown` at trust boundaries, then validate and narrow.
- Prefer `readonly` data wherever mutation is not required.
- Prefer string literal unions over `enum` for application-level state and variants.
- Prefer explicit return types on exported functions and public methods.
- Avoid boolean-flag APIs that combine multiple behaviors. Use explicit command objects or discriminated unions instead.
- Prefer `null` only when an external API or protocol requires it. Otherwise use absence or an explicit union.

## `type` vs `interface`

This repo deliberately uses a stricter rule than common TypeScript defaults.

Project standard:

- Use `type` for domain models, DTOs, view models, result types, command types, event types, tuples, branded IDs, and discriminated unions.
- Use `interface` only for open contracts, ports implemented by multiple classes, ambient declarations, or cases where extension is intentionally part of the design.
- Do not use `interface` for routine domain data objects.

Rationale:

- Official TypeScript and Google guidance often lean toward `interface` for plain object shapes.
- This repo instead prefers closed `type` aliases for most application data because they work better with discriminated unions, keep models closed to hidden extension, and make mapping boundaries more explicit.
- This is a deliberate project choice for maintainability, not a claim that TypeScript itself requires it.

## Domain Modelling

Model each bounded context explicitly. Do not reuse one model across unrelated domains just because fields happen to match today.

Required model categories:

- Wire or transport model: shape used on the network or protocol boundary
- Domain model: shape that carries business meaning and invariants
- View model: shape tailored for rendering or UI interaction
- Persistence or configuration model: shape stored on disk or in config

Rules:

- Each layer owns its own model types.
- Never pass transport DTOs directly into domain services.
- Never expose domain models directly to UI or transport without mapping.
- Use discriminated unions for variant state, workflow state, and result state.
- Prefer explicit value objects for IDs, tokens, and constrained values.
- Use branded aliases when a raw `string` or `number` would be ambiguous.

Example direction:

- `SessionWireMessage` -> `SessionMessageMapper.toDomain(...)` -> `Session`
- `Session` -> `SessionViewModelMapper.toViewModel(...)` -> `SessionViewModel`

## Mapping Strategy

Cross-boundary mapping must be explicit.

- Use mapper or assembler classes such as `SessionWireMapper`, `AudioConfigMapper`, or `ClipboardViewModelMapper`.
- A mapper should own one boundary and one direction at a time.
- Mapping methods must validate the incoming representation before constructing the target representation.
- Do not use unchecked casts such as `as SomeDomainType` as a substitute for mapping.
- Do not reuse one type alias across domains, layers, or features to “save code”.
- If two models are intentionally identical today but belong to different contexts, keep two types and a trivial mapper.

## Classes And Object Design

- Use classes for behaviorful units such as services, controllers, use cases, mappers, factories, and repositories.
- Do not use classes as passive data bags.
- Class fields must be `private` by default.
- Prefer `private readonly` for dependencies and configuration.
- Do not expose public mutable fields.
- If state must be observable, expose it through methods or explicit read-only getters.
- Use `protected` rarely. Inheritance should be exceptional, not routine.
- `static` members are allowed for constants and factory helpers, not for shared mutable state.
- Public methods should represent a small, coherent API surface.
- Separate commands from queries where practical.

## Construction And Dependency Injection

- Do not instantiate service collaborators inside a class.
- Wire dependencies in a composition root, factory, or builder.
- Constructors should receive dependencies, configuration, and owned runtime objects only.
- Creating DOM nodes, buffers, or protocol frames owned by the class is acceptable inside the class.
- Creating service objects, transports, repositories, clocks, or loggers inside the class is not acceptable.
- Prefer constructor injection for mandatory dependencies.
- Use factories for complex object graphs or runtime-specific assembly.
- Avoid service locators and singleton lookups.

For testability:

- Inject clock, random source, logger, transport, storage, and browser or Node adapters.
- Avoid direct calls to `Date.now()`, `Math.random()`, global `fetch`, or process-wide state inside domain or application classes.

## Method Design

Every public method must have a clear flow:

1. Validate input and state preconditions.
2. Execute one coherent unit of work.
3. Translate or wrap errors where boundary context is added.
4. Return a clearly typed result.

Rules:

- Public methods must start with explicit validation or delegate immediately to a named validation method.
- Private methods may rely on caller-validated invariants, but that contract must stay obvious from the call flow.
- Do not mix parsing, validation, mapping, side effects, and rendering in one method.
- Prefer one abstraction level per method.
- Use early returns to keep control flow flat.

## Validation

Treat all external input as untrusted:

- network payloads
- DOM events
- URL params
- local storage
- process environment
- config files
- third-party library outputs

Rules:

- Validate once at the trust boundary, then map into a trusted internal model.
- Use dedicated validators, guard methods, or value-object constructors for invariants.
- Validation failures must produce typed errors with stable codes.
- Optional fields must be validated explicitly; do not rely on truthiness checks for business meaning.
- When a field can be absent or present in multiple valid forms, model it as a discriminated union instead of loose optional properties.

## Errors And Exception Handling

Use typed errors, stable error codes, and explicit cause chains.

- Never throw strings.
- Never throw plain objects.
- Never swallow errors silently.
- Never log and rethrow repeatedly at every layer.

Error baseline:

- Extend `Error` for project-specific errors.
- Include a stable `code`.
- Include `cause` when wrapping another error.
- Include structured details only when they are safe and useful.

Recommended categories:

- `ValidationError`
- `DomainError`
- `NotFoundError`
- `ConflictError`
- `AuthorizationError`
- `ConfigurationError`
- `ExternalDependencyError`
- `TransportError`
- `UnexpectedStateError`

Usage rules:

- Use result unions for expected business alternatives.
- Use exceptions for invalid input, invariant violations, external failures, and unexpected runtime conditions.
- Catch errors at architectural boundaries, add context, then rethrow or translate.
- Messages should be human-readable; `code` should be machine-stable.
- When wrapping, preserve the original error with `cause`.

## Promise And Async Rules

- Every Promise must be awaited, returned, or handled explicitly.
- No floating Promises.
- No async callbacks in places that ignore returned Promises unless wrapped deliberately.
- Fire-and-forget work must be rare and must route failures to a named handler.
- Do not mix `.then()` chains and `await` in the same method without a clear reason.
- Prefer `async` and `await` for linear application flow.

## Testing Standards

- Design every class so it can be instantiated in a unit test with fakes or stubs.
- No hidden singleton dependencies.
- No hidden environment reads inside business logic.
- One test suite should focus on one public class or one mapper.
- Add regression tests for bug fixes before or alongside the fix.
- Keep tests deterministic.
- Use builders or object mothers for complex fixtures.
- Avoid sharing mutable fixtures across tests.
- Prefer testing public behavior over private implementation details.

## Linting And Tooling

When linting is configured for a TypeScript package, start from typed linting with `typescript-eslint` recommended and strict type-checked rule sets, then add project-specific rules.

Priority rules for this repo:

- `@typescript-eslint/explicit-member-accessibility`
- `@typescript-eslint/explicit-function-return-type`
- `@typescript-eslint/no-explicit-any`
- `@typescript-eslint/no-floating-promises`
- `@typescript-eslint/no-misused-promises`
- `@typescript-eslint/switch-exhaustiveness-check`
- `@typescript-eslint/use-unknown-in-catch-callback-variable`

## BrowserPane-Specific Guidance

- Keep browser transport, DOM handling, media decoding, and rendering concerns out of domain models.
- Wire protocol constants and packet layouts belong to dedicated protocol modules, not to UI classes.
- Browser event handlers should translate raw browser events into typed application commands before business logic runs.
- Shared session state should be represented as explicit unions or value objects, not loose mutable bags.
- Client-side feature flags and capability sets should use typed models and explicit mappers.

## Review Checklist

Before merging TypeScript or Node.js changes, check:

- Is the file and class size within the target range?
- Is the code organized around one bounded context and one responsibility?
- Are domain, transport, and view models separate?
- Is every cross-boundary mapping explicit?
- Are public methods validating inputs and preconditions?
- Are errors typed, coded, and chained with `cause` where relevant?
- Are dependencies injected instead of created inside classes?
- Are there any public mutable fields, global methods, or hidden singleton dependencies?
- Will the next engineer be able to unit test this class in isolation?

## Reference Basis

This file is based primarily on current TypeScript, Node.js, and `typescript-eslint` guidance, then adapted deliberately for BrowserPane:

- TypeScript docs on modules, classes, narrowing, type aliases, interfaces, and TSConfig strictness
- Node.js docs on errors, error propagation, and `Error` `cause`
- `typescript-eslint` guidance for explicit member visibility, explicit return types, Promise handling, exhaustiveness, and `unknown` in catch handlers
- Google TypeScript Style Guide as a useful contrast point, especially where this repo intentionally chooses a different `type`-first rule for application data models

Reference links:

- https://www.typescriptlang.org/docs/handbook/2/everyday-types.html
- https://www.typescriptlang.org/docs/handbook/2/narrowing.html
- https://www.typescriptlang.org/tsconfig/
- https://nodejs.org/api/errors.html
- https://typescript-eslint.io/rules/explicit-member-accessibility/
- https://typescript-eslint.io/rules/no-floating-promises/
- https://typescript-eslint.io/rules/switch-exhaustiveness-check/
- https://google.github.io/styleguide/tsguide.html
