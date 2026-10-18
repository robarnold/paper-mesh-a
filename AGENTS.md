# Repository agent instructions

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

## Development environment

Run all project tools in the devcontainer. Do not install Rust, ESP tools,
Python packages, or other project dependencies on the host. Do not change the
host home directory or shell startup files.

The host can run Docker and the devcontainer CLI to control the container.
The host can also read and edit repository files. Use these commands from the
repository root:

```sh
devcontainer up --workspace-folder .
devcontainer exec --workspace-folder . just ci
```

If the container cannot start, report the failure. Ask the user how to continue.
Do not run checks on the host as an alternative.

Before a commit, run `just ci` in the container. Do not use semantic commit
messages. Do not flash a connected device unless the user requests it.

## Implementation boundaries

The intended device is the LILYGO T5 S3 E-Paper Pro with LoRa. The firmware in
`firmware/` uses Rust, esp-hal, and Embassy on esp-rtos. Run Cargo from that
directory to load the target configuration and the `esp` toolchain.
The firmware implements display output and single-contact touch input.
Display refresh runs on core 1. Touch interrupts and the application run on core 0.
System power management stays planned work.
The design does not use ESPHome or an ESP-IDF application runtime.

The root Cargo workspace builds the shared `no_std` application in `crates/app/`
and the native Rust simulator in `simulator/`.
It also tests the ED047TC1 encoder and GT911 protocol in separate crates.
Firmware stays outside that
workspace to isolate Xtensa settings and ESP dependency features.

Keep application behavior and rendering in the shared crate. Keep simulator
HTTP and wall-clock pacing in the native binary. Hardware drivers implement the
application's platform interfaces through esp-hal and embedded-hal.

When shared application contracts change, update `docs/architecture.md`.
Keep those contracts and `docs/device-interfaces.md` independent of board SKUs.
Put board inventories, chip behavior, wiring, and conformance cases in
`docs/boards/`. Adapters convert these details into capabilities, geometry, and
logical controls. Do not add board or chip checks to shared application policy.

The current demonstration uses the shared portrait rotation. Native display
storage stays 960 × 540, while the application sees 540 × 960.
Run `just sim` in the devcontainer to serve the simulator on localhost:8080.

## External references

Keep vendor documents external. Use links to upstream sources at specified
revisions. Do not commit copies, extracted text, or document screenshots.
The reference index is `docs/vendor/lilygo/t5-s3-pro/README.md`.
Download documents into the devcontainer's `/tmp` directory when necessary.

Record project findings in the applicable backend profile. Include source links
and page or section references. Keep general service requirements and display
requirements in the architecture and portable contract documents.
