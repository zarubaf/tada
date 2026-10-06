# tada

tada is an event-planning workspace for clubs.
[PRODUCT.md](doc/PRODUCT.md) describes what we build, and [ARCHITECTURE.md](doc/ARCHITECTURE.md) describes how.
The [roadmap](doc/roadmap.md) shows what is done and what comes next.

## Run it locally

You need [mise](https://mise.jdx.dev), [rustup](https://rustup.rs) and Docker.
Add `~/.cargo/bin` to your `PATH`.

1. Run `mise run setup` once. This installs the pinned tools and the Git hooks.
2. Run `mise run dev:up`. This starts PostgreSQL, Garage and Mailpit.
3. Run `mise run dev:serve`. This applies the migrations and starts the API on port 8080.
4. Run `mise run dev:web` in a second terminal.
5. Open `http://localhost:5173`.

The page shows the events of the development organization.
A debug build acts as the owner of this organization for each request ([ADR 0053](doc/adr/0053-development-authenticator.md)).
The web client cannot create events yet. Create one through the API:

```sh
curl -X POST http://localhost:8080/api/v1/events \
  -H 'Content-Type: application/json' \
  -d '{"key": "TEST30", "name": "Tag der offenen Tür Testwil"}'
```

Run `mise run dev:down` to stop the services. The data stays in the Docker volumes.

## Work on tada

Read [AGENTS.md](AGENTS.md) and the [contributing guide](doc/contributing.md) before you change anything.
Run `mise run check` before each commit.

## License

[Apache-2.0](LICENSE)
