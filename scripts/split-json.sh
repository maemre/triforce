#!/bin/bash

if [ "$#" -ne 2 ]; then
  echo "Usage: $0 <N> <input.json>"
  exit 1
fi

N=$1
INPUT=$2
BASENAME="${INPUT%.json}"

jq ".[:$N]" "$INPUT" > "${BASENAME}-first-${N}.json"
jq ".[$N:]" "$INPUT" > "${BASENAME}-after-${N}.json"

echo "Created ${BASENAME}-first-${N}.json and ${BASENAME}-after-${N}.json"
