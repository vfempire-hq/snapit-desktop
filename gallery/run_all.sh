#!/usr/bin/env bash
# Chain batches 03 → 04 → 05 → 06 sequentially, regenerate the showcase
# after every batch so the page grows live as images land, then deploy
# so the CF-served copy stays in sync.

set -e
cd "$(dirname "$0")"

log() { echo "[$(date +%H:%M:%S)] $*"; }

deploy() {
    export CLOUDFLARE_API_TOKEN=$(grep '^CLOUDFLARE_API_TOKEN=' /home/guardiansoftiktok/sovereign-os/.env | cut -d= -f2 | awk '{print $1}')
    (cd ../purchase-backend && npx wrangler deploy 2>&1 | tail -3)
}

regen_and_deploy() {
    python3 regen_showcase.py
    deploy
}

for f in prompts/batch-03-landscape-volume.json \
         prompts/batch-04-portrait-volume.json \
         prompts/batch-05-square-volume.json \
         prompts/batch-06-story-volume.json \
         prompts/batch-07-locations.json \
         prompts/batch-08-food-travel.json \
         prompts/batch-09-fine-dining-business.json \
         prompts/batch-10-adrenaline-stills.json \
         prompts/batch-11-widen-categories.json \
         prompts/batch-13-lifecycle.json \
         prompts/batch-14-holidays.json \
         prompts/batch-15-daily-life.json \
         prompts/batch-16-work-culture.json; do
    log "== running $f =="
    python3 -u gen.py "$f"
    log "== batch done: $f =="
    regen_and_deploy
done

log "== ALL BATCHES COMPLETE =="
regen_and_deploy
