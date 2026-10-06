# Settings

The command `tada settings` writes this file. Do not change it by hand.
Run `mise run gen` after a change of the settings code.

Settings come from environment variables (ADR 0025).
A variable that ends in `_FILE` gives the path of a file that contains a secret (ADR 0036).

| Variable                         | Value                  | Default                    | Secret | Commands                                 | Description                                                                                                                         |
| -------------------------------- | ---------------------- | -------------------------- | ------ | ---------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `TADA_LOG`                       | log filter             | `info`                     | no     | `serve`, `worker`, `telegram`, `migrate` | The level filter of the logs, in the syntax of `tracing-subscriber`, for example `info,tada_api=debug`.                             |
| `TADA_DATABASE_URL`              | PostgreSQL URL         | none                       | no     | `serve`, `worker`, `telegram`, `migrate` | The database URL without the password, for example `postgres://tada@db:5432/tada`.                                                  |
| `TADA_DATABASE_PASSWORD_FILE`    | file path              | none                       | yes    | `serve`, `worker`, `telegram`, `migrate` | The file that contains the database password.                                                                                       |
| `TADA_PORT`                      | port number            | `8080`                     | no     | `serve`                                  | The TCP port of the HTTP server. The server does not handle TLS.                                                                    |
| `TADA_TRUSTED_PROXIES`           | list of network ranges | empty                      | no     | `serve`                                  | The ranges of the reverse proxies, separated by commas, for example `10.0.0.0/8`. The server accepts `X-Request-Id` only from them. |
| `TADA_WEB_ROOT`                  | folder path            | empty                      | no     | `serve`                                  | The folder of the built web client. The image sets it. If it is empty, the server delivers the API only.                            |
| `TADA_S3_ENDPOINT`               | URL                    | none                       | no     | `serve`                                  | The URL of the S3 API, for example `http://garage:3900`.                                                                            |
| `TADA_S3_REGION`                 | text                   | none                       | no     | `serve`                                  | The S3 region. Garage uses the `s3_region` of its configuration.                                                                    |
| `TADA_S3_BUCKET`                 | text                   | none                       | no     | `serve`                                  | The bucket for all objects of this installation.                                                                                    |
| `TADA_S3_ACCESS_KEY_ID_FILE`     | file path              | none                       | yes    | `serve`                                  | The file that contains the S3 access key ID.                                                                                        |
| `TADA_S3_SECRET_ACCESS_KEY_FILE` | file path              | none                       | yes    | `serve`                                  | The file that contains the S3 secret access key.                                                                                    |
| `TADA_TELEGRAM_BOT_TOKEN_FILE`   | file path              | none                       | yes    | `telegram`                               | The file that contains the token of the Telegram bot.                                                                               |
| `TADA_TELEGRAM_API_URL`          | URL                    | `https://api.telegram.org` | no     | `telegram`                               | The base URL of the Telegram Bot API. Tests set a local fake.                                                                       |
