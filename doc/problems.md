# Problem codes

This catalog lists the stable problem codes of the tada API (ADR 0037).
The `type` of each error response links to its entry here.
The command `tada problems` writes this file. Do not change it by hand.

A code never changes its meaning. A code that is no longer used stays reserved.

| Code                                                          | Status | Meaning                                                                                            |
| ------------------------------------------------------------- | ------ | -------------------------------------------------------------------------------------------------- |
| <a id="malformed-request"></a>`malformed-request`             | 400    | The body is not valid JSON or does not match the schema.                                           |
| <a id="unauthenticated"></a>`unauthenticated`                 | 401    | No valid session or token.                                                                         |
| <a id="organization-required"></a>`organization-required`     | 403    | The session has no organization. The client lets the member choose one.                            |
| <a id="recent-sign-in-required"></a>`recent-sign-in-required` | 403    | The action needs a sign-in of at most 15 minutes ago. The client asks the member to sign in again. |
| <a id="forbidden"></a>`forbidden`                             | 403    | The caller can see the record but lacks the permission for this action.                            |
| <a id="not-found"></a>`not-found`                             | 404    | The record does not exist, or it is in a scope the caller cannot see.                              |
| <a id="record-version-conflict"></a>`record-version-conflict` | 409    | The record changed after the caller read it.                                                       |
| <a id="invalid-transition"></a>`invalid-transition`           | 409    | The current state does not allow this change.                                                      |
| <a id="payload-too-large"></a>`payload-too-large`             | 413    | The body is larger than the limit.                                                                 |
| <a id="unsupported-media-type"></a>`unsupported-media-type`   | 415    | The media type is not supported.                                                                   |
| <a id="validation-failed"></a>`validation-failed`             | 422    | Values break a domain rule. The `errors` list names them.                                          |
| <a id="rate-limited"></a>`rate-limited`                       | 429    | Too many requests. The response has `Retry-After`.                                                 |
| <a id="unavailable"></a>`unavailable`                         | 503    | A dependency is unavailable. The client can retry.                                                 |
| <a id="internal"></a>`internal`                               | 500    | An unexpected error. The response contains no other detail.                                        |
