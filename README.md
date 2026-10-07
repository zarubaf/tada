# tada

tada is an event-planning workspace for clubs.
[PRODUCT.md](doc/PRODUCT.md) describes what we build, and [ARCHITECTURE.md](doc/ARCHITECTURE.md) describes how.
The [roadmap](doc/roadmap.md) shows what comes next.

## Run it locally

You need [mise](https://mise.jdx.dev), [rustup](https://rustup.rs) and Docker.
The mise tasks find `cargo` in `~/.cargo/bin`. To use `cargo` directly in your shell, add `. "$HOME/.cargo/env"` to your shell profile.

1. Run `mise run setup` once. This installs the pinned tools and the Git hooks.
2. Run `mise run dev:up`. This starts PostgreSQL, Garage and Mailpit.
3. Run `mise run dev:serve`. This applies the migrations and starts the API on port 8080.
4. Run `mise run dev:web` in a second terminal.
5. Open `http://localhost:5173`.

Each request needs a session.
Create an organization and read the sign-in mail in Mailpit as the [contributing guide](doc/contributing.md#local-runtime) describes.

Run `mise run dev:down` to stop the services. The data stays in the Docker volumes.

## Work on tada

Read [AGENTS.md](AGENTS.md) and the [contributing guide](doc/contributing.md) before you change anything.
Run `mise run check` before each commit.

## License

[Apache-2.0](LICENSE)
