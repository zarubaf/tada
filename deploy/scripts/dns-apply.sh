#!/usr/bin/env bash
# Create or update the tada DNS records in Cloudflare (ADR 0027).
# Idempotent: records that already match stay unchanged. Other records stay untouched.
#
# Usage: dns-apply.sh [--dry-run]
# Needs: deploy/inventory.local, and a Cloudflare API token with DNS edit
# rights for the zone in the file $TADA_CLOUDFLARE_TOKEN_FILE.
set -euo pipefail

ZONE="zaruba.email"
NAMES=("tada" "staging.tada")

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
inventory="$root/deploy/inventory.local"
token_file="${TADA_CLOUDFLARE_TOKEN_FILE:-$HOME/.config/tada/cloudflare-api-token}"
api="https://api.cloudflare.com/client/v4"

dry_run=false
case "${1:-}" in
  --dry-run) dry_run=true ;;
  "") ;;
  *) echo "Unknown argument: $1" >&2; exit 2 ;;
esac

[[ -r "$inventory" ]] || { echo "Missing $inventory. Copy deploy/inventory.example." >&2; exit 1; }
[[ -r "$token_file" ]] || { echo "Missing token file $token_file." >&2; exit 1; }
# shellcheck source=/dev/null
source "$inventory"
: "${TADA_HOST_IPV4:?is not set in $inventory}"

# The token goes to curl through a header file, so it never shows in the process list.
cf() {
  local method="$1" path="$2" body="${3:-}"
  local response
  response="$(curl -sS -X "$method" \
    -H @<(printf 'Authorization: Bearer %s\nContent-Type: application/json\n' "$(<"$token_file")") \
    ${body:+--data "$body"} "$api$path")"
  if [[ "$(jq -r '.success' <<<"$response")" != "true" ]]; then
    echo "Cloudflare API error on $method $path: $(jq -c '.errors' <<<"$response")" >&2
    exit 1
  fi
  printf '%s' "$response"
}

zone_id="$(cf GET "/zones?name=$ZONE" | jq -r '.result[0].id // empty')"
[[ -n "$zone_id" ]] || { echo "Zone $ZONE not found." >&2; exit 1; }

apply() {
  local type="$1" fqdn="$2" content="$3"
  local existing id current proxied body
  existing="$(cf GET "/zones/$zone_id/dns_records?type=$type&name=$fqdn")"
  if [[ "$(jq '.result | length' <<<"$existing")" -gt 1 ]]; then
    echo "conflict  $type $fqdn has more than one record; fix it by hand" >&2
    exit 1
  fi
  id="$(jq -r '.result[0].id // empty' <<<"$existing")"
  current="$(jq -r '.result[0].content // empty' <<<"$existing")"
  proxied="$(jq -r '.result[0].proxied // false' <<<"$existing")"
  body="$(jq -nc --arg t "$type" --arg n "$fqdn" --arg c "$content" \
    '{type: $t, name: $n, content: $c, proxied: false, ttl: 1, comment: "managed by tada deploy/scripts/dns-apply.sh"}')"

  if [[ -z "$id" ]]; then
    echo "create    $type $fqdn -> $content"
    $dry_run || cf POST "/zones/$zone_id/dns_records" "$body" >/dev/null
  elif [[ "$current" != "$content" || "$proxied" != "false" ]]; then
    echo "update    $type $fqdn: $current (proxied=$proxied) -> $content (proxied=false)"
    $dry_run || cf PATCH "/zones/$zone_id/dns_records/$id" "$body" >/dev/null
  else
    echo "unchanged $type $fqdn -> $content"
  fi
}

$dry_run && echo "Dry run: no changes."
for name in "${NAMES[@]}"; do
  apply A "$name.$ZONE" "$TADA_HOST_IPV4"
  [[ -z "${TADA_HOST_IPV6:-}" ]] || apply AAAA "$name.$ZONE" "$TADA_HOST_IPV6"
done
