#!/usr/bin/env bash
# List ride requests near a location via the REST API.
# Usage: scripts/list_requests.sh <latitude> <longitude>
set -euo pipefail

API="${API:-http://127.0.0.1:3000}"

if [ $# -lt 2 ]; then
    echo "usage: $0 <latitude> <longitude>" >&2
    exit 1
fi

curl -sS --fail-with-body "$API/requests?latitude=$1&longitude=$2"
echo
