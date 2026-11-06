# Documentation guide

These rules apply to all contributors.
They cover documentation, comments, review text, commit descriptions, and agent responses.

## Documentation principles

State why the design exists. Explain the constraints and tradeoffs that shaped the decision.
Describe operational consequences that a reader cannot infer from the implementation.

Link to upstream documentation for general explanations of deployed components.
Describe a component's behavior only when a subtle detail affects how the design achieves its purpose.
Explain that connection with only the detail needed to understand the decision.

Treat code and configuration as the source of truth for current behavior.
Link to the files that define values, versions, resource inventories, and settings.
Include a configuration detail only to explain a decision or to complete a non-obvious procedure.

Do not document rollout processes.
Include deployment steps only to bootstrap the cluster after a disaster.
State that recovery purpose at the start of the procedure.

Organize documentation by responsibility and concept.
Keep overview documents general enough for future applications and platform services.
Do not organize an overview around the current module list or implemented feature list.
Use overviews to explain purpose, boundaries, ownership, and relationships.
Put details needed to explain a design decision or contract in its topic document.
For example, orientation state and coordinate spaces belong in a separate platform document.

Separate application choices from common platform behavior in the documentation.
Describe the platform as a toolkit that applications can use.
Identify current behavior, planned work, and decisions that remain open in each topic.
Record known limitations with the behavior that they affect.
Keep each detailed rule in one primary document.
Link to that rule from other documents.

## Code comments and module documentation

Use inline comments to explain why the code makes a choice.
Name the constraint it satisfies or the failure it prevents. Omit comments that only restate the operation.

Use module documentation to explain why the module exists and which responsibilities it owns.
For Rust documentation conventions, follow [How to write documentation in the rustdoc book](https://doc.rust-lang.org/rustdoc/how-to-write-documentation.html).
When the module makes a design or implementation choice, add a `# Design choices` section.
Use one bullet per choice. Explain the constraint it satisfies or the consequence for callers.
Link to repository specifications with paths relative to the source file so readers reach the specification in the same revision.
For example, the [input contract](docs/services/input.md) requires debounced button events.
The button module must filter contact bounce so one physical press produces one press event.
The backend selects the debounce interval for its hardware.

## Test intent

Write a short comment above each test that states the condition and the expected observable behavior.
If the reason for that expectation is unclear, name the contract or failure the test guards against.
Leave setup steps and assertion details in the test body.

## Document map

Choose a document by the behavior or responsibility that it describes.
Keep each topic complete enough to explain its purpose and limits.
Create a separate topic when its details need independent explanation.
Do not create a document for each source file or small feature.

| Document | Responsibility |
| --- | --- |
| [README.md](README.md) | Project purpose, startup commands, and navigation |
| [Architecture](docs/architecture.md) | Code boundaries, dependencies, and ownership |
| [Application](docs/application.md) | Application screens, workflows, layout, and policy choices |
| [Shared UI standard](docs/ui.md) | Common visual language, navigation, control behavior, and review rules |
| [Application UI rules](docs/application-ui.md) | Application destinations, operation restrictions, style selection, and layout |
| [Platform](docs/platform.md) | General platform responsibilities and links to detailed topics |
| `docs/platform/` | Specific toolkit mechanisms, execution, and shared state |
| [Device services](docs/device-interfaces.md) | Common contract requirements and links to service topics |
| `docs/services/` | Detailed portable service contracts and their status |
| [Display contract](docs/display-constraints.md) | Portable display and lighting requirements |
| [Hardware backends](docs/hardware.md) | Profile index and requirements for new backends |
| `docs/boards/` | Board inventories, chip behavior, wiring, and conformance cases |
| [Simulator](docs/simulator.md) | Simulator purpose, boundaries, and topic navigation |
| `docs/simulator/` | Execution, API, controls, and scenarios |
| [Development](docs/development.md), [firmware builds](docs/firmware-build.md) | Environment, commands, CI, and build mechanisms |
| [Terminology](docs/terminology.md) | Project terms and their meanings |
| `docs/vendor/` | External reference indexes and source metadata |
| Crate README files | Crate scope, protocol references, and license notices |

## Documentation updates

Keep platform responsibilities and portable contracts independent of board SKUs.
For each new contract, identify a consumer, a backend, and deterministic scenarios.
When code boundaries change, update `docs/architecture.md`.
When behavior changes, update its primary document.
When a topic moves, update its links and document maps.
Check relative links and section anchors after documentation changes.

## English prose

Use [ASD-STE100, Issue 9 (2025-01-15)](https://www.asd-ste100.org/assets/files/ASD-STE100_ISSUE9.pdf)
for English prose. This requirement applies to documentation, comments, review
text, commit descriptions, and agent responses.

Use the complete standard for language decisions. Apply these checks during
revision:

- Select dictionary words with the specified meaning and part of speech.
- Use technical nouns and technical verbs from the permitted subject-field
  categories. Do not classify ordinary jargon as a technical term.
- Use one term for each concept. Refer to [the project terminology](docs/terminology.md).
- Use American English spelling.
- Use active voice. Use passive voice in descriptions only when the agent is unknown.
- Use simple verb forms. Use an `-ing` form only as a technical noun or its modifier.
- Give each instruction as a command. Put a necessary condition before the command.
- Give one instruction per sentence, except for actions that occur at the same time.
- Use a maximum of 20 words for instructions. Use a maximum of 25 words for descriptions.
- Limit each paragraph to one topic and six sentences.
- Use a maximum of three words in noun groups. Give a shorter form for a longer technical name.
- Include necessary articles. Do not use contractions, semicolons, or unnecessary jargon.
- Use the word-count rules in section 8. Do not treat a length check as a complete STE check.

Keep commands, identifiers, URLs, units, register values, source references,
and executable content during prose changes. Keep licenses, quoted source
text, and third-party content. Keep vendor documents and the standard external.
If reference processing is necessary, use the devcontainer's `/tmp` directory.

## Third-party dependencies and assets

Prefer package dependencies to copies of third-party source or assets.
Pin each new or updated third-party dependency to an explicit version.
Apply this rule to runtime, build, and development dependencies.

For Cargo registry dependencies, use an exact requirement such as `=0.9.0`.
Do not use implicit caret requirements, version ranges, or wildcards.
For Git dependencies, use a full commit hash in `rev`.
Keep local workspace dependencies as path dependencies.

Exact requirements keep dependency updates explicit when Cargo resolves dependencies again.
Keep the applicable lockfile consistent with each manifest change.
The [workspace lockfile](Cargo.lock) and [firmware lockfile](firmware/Cargo.lock) record the resolved dependency graphs.
Keep version values in manifests and lockfiles instead of duplicating them in prose.

Check the upstream license for the selected version against the intended use and distribution.
Check bundled artwork separately, including artwork that appears in generated images.
If the terms are unclear, resolve the question before adoption.

Record the source and license for dependencies and assets distributed with simulator or device images.
Development-only tools require no license entry in project documentation.
For distributed material, link to the file that pins the reviewed version.
If no file identifies that version, record it with the license decision.
Include required attribution and license notices with the distributed material.

## External references and evidence

Keep vendor documents external. Use links to upstream sources at specified
revisions. Do not commit copies, extracted text, or document screenshots.
Required license and attribution notices are an exception to this restriction.
The reference index is `docs/vendor/lilygo/t5-s3-pro/README.md`.
Download documents into the devcontainer's `/tmp` directory when necessary.

Record project behavior, design decisions, and known limitations in the applicable
documentation. Put board-specific findings in the applicable backend profile.
Include source links and page or section references.
Keep general service requirements and display requirements in the portable contract documents.

Put evidence from individual test runs in PR descriptions or commit messages.
This evidence includes observed firmware hashes, logs, message counts, timings,
and other temporary test results. Do not add this evidence to project
documentation.

Keep verified conclusions and remaining limits in the applicable topic.
Keep source revisions and dates that identify a reference review.
Do not present calculated examples or simulator delays as measured hardware performance.

## Event handler names

Prefer `will_` and `did_` prefixes to `on_` for event handlers and callback registration methods.
Use `will_` before the named operation. Use `did_` after the named event or transition occurs.
Select the name from actual dispatch timing, not from the caller's intent.
A `will_` handler does not imply cancellation support. Document any cancellation contract separately.
For example, page entry uses `did_enter`, page exit uses `will_leave`, and completed activation uses `did_activate`.
