#!/usr/bin/env bash
# Publish a pedestrian ride request via the REST API.
# Usage: scripts/publish_request.sh <latitude> <longitude> [max_price_cents]
set -euo pipefail

API="${API:-http://127.0.0.1:3000}"

if [ $# -lt 2 ]; then
    echo "usage: $0 <latitude> <longitude> [max_price_cents]" >&2
    exit 1
fi

LAT="$1"
LNG="$2"
PRICE="${3:-100}"

curl -sS --fail-with-body -X POST "$API/requests" \
    -H 'content-type: application/json' \
    -d "{\"latitude\": $LAT, \"longitude\": $LNG, \"max_price_cents\": $PRICE}"
echo
