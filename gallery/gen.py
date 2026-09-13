#!/usr/bin/env python3
# gallery/gen.py — SnapIT showcase gallery batch runner.
#
# Queues Flux dev fp8 prompts against ComfyUI on 10.1.0.2:8188, pulls the
# resulting images back, and organises them by section so the showcase page
# can point at them directly.
#
# Runs against the shared workflow at /home/.../gallery/workflow.flux.json —
# only the KSampler seed, positive prompt text and dimensions are patched
# per prompt.

import argparse, json, os, sys, time, urllib.parse, urllib.request, uuid, random, socket
from pathlib import Path

COMFY_HOST = os.environ.get("COMFY_HOST", "10.1.0.2:8188")
OUT_DIR = Path(os.environ.get("SNAPIT_GALLERY_OUT", "/home/guardiansoftiktok/snapit-desktop/gallery/out"))
PROMPTS_DIR = Path(os.environ.get("SNAPIT_GALLERY_PROMPTS", "/home/guardiansoftiktok/snapit-desktop/gallery/prompts"))
CLIENT_ID = str(uuid.uuid4())

# Curated per-section prompt bank. Every entry: (section, filename_prefix, aspect, prompt).
# Aspect keys map to (width, height) below.
ASPECTS = {
    "wide16x9":   (1344, 768),
    "square":     (1024, 1024),
    "portrait":   (896, 1152),
    "story":      (720, 1280),
}

# The base workflow — Flux dev fp8 UNET + dual CLIP + ae VAE + BasicGuider chain
def build_workflow(prompt_text: str, w: int, h: int, seed: int) -> dict:
    return {
        "10": {"class_type": "UNETLoader", "inputs": {
            "unet_name": "flux1-dev-fp8-unet.safetensors",
            "weight_dtype": "fp8_e4m3fn_fast",
        }},
        "11": {"class_type": "DualCLIPLoader", "inputs": {
            "clip_name1": "t5xxl_fp8_e4m3fn.safetensors",
            "clip_name2": "clip_l.safetensors",
            "type": "flux",
        }},
        "12": {"class_type": "VAELoader", "inputs": {"vae_name": "ae.safetensors"}},
        "20": {"class_type": "CLIPTextEncode", "inputs": {
            "clip": ["11", 0],
            "text": prompt_text,
        }},
        "21": {"class_type": "FluxGuidance", "inputs": {
            "conditioning": ["20", 0],
            "guidance": 3.2,
        }},
        "22": {"class_type": "CLIPTextEncode", "inputs": {
            "clip": ["11", 0],
            "text": "",
        }},
        "30": {"class_type": "EmptySD3LatentImage", "inputs": {
            "width": w, "height": h, "batch_size": 1,
        }},
        "40": {"class_type": "BasicScheduler", "inputs": {
            "model": ["10", 0],
            "scheduler": "simple",
            "steps": 20,
            "denoise": 1.0,
        }},
        "41": {"class_type": "KSamplerSelect", "inputs": {
            "sampler_name": "euler",
        }},
        "42": {"class_type": "RandomNoise", "inputs": {
            "noise_seed": seed,
        }},
        "43": {"class_type": "BasicGuider", "inputs": {
            "model": ["10", 0],
            "conditioning": ["21", 0],
        }},
        "44": {"class_type": "SamplerCustomAdvanced", "inputs": {
            "noise":     ["42", 0],
            "guider":    ["43", 0],
            "sampler":   ["41", 0],
            "sigmas":    ["40", 0],
            "latent_image": ["30", 0],
        }},
        "50": {"class_type": "VAEDecode", "inputs": {
            "samples": ["44", 0],
            "vae":     ["12", 0],
        }},
        "60": {"class_type": "SaveImage", "inputs": {
            "images": ["50", 0],
            "filename_prefix": "snapit-gallery",
        }},
    }

def http_json(method, path, body=None, host=None):
    host = host or COMFY_HOST
    url = f"http://{host}{path}"
    data = json.dumps(body).encode("utf-8") if body is not None else None
    req = urllib.request.Request(url, data=data, method=method)
    if data is not None:
        req.add_header("Content-Type", "application/json")
    with urllib.request.urlopen(req, timeout=90) as r:
        return json.loads(r.read())

def queue_prompt(workflow: dict) -> str:
    resp = http_json("POST", "/prompt", {"prompt": workflow, "client_id": CLIENT_ID})
    return resp["prompt_id"]

def poll_history(prompt_id: str, timeout_s: int = 180) -> dict | None:
    """Poll /history/<id> until the prompt appears. Returns the entry or None
    after timeout_s. 180s is plenty for a Flux dev 20-step 1344x768 on the
    4080 — if it hasn't landed by then, something's wedged and we should
    give up on this prompt rather than block the whole batch."""
    started = time.time()
    while time.time() - started < timeout_s:
        try:
            hist = http_json("GET", f"/history/{prompt_id}")
            if prompt_id in hist:
                return hist[prompt_id]
        except Exception:
            pass
        # Also give up early if the queue no longer knows about this prompt
        # (someone cleared the queue or the prompt was auto-purged)
        try:
            q = http_json("GET", "/queue")
            in_queue = any(
                (r[1] if len(r) > 1 else "") == prompt_id
                for r in (q.get("queue_running", []) + q.get("queue_pending", []))
            )
            if not in_queue and time.time() - started > 20:
                # Not queued and not in history — the prompt died silently.
                return None
        except Exception:
            pass
        time.sleep(2)
    return None

def download_image(filename: str, subfolder: str, image_type: str, dest: Path):
    qs = urllib.parse.urlencode({"filename": filename, "subfolder": subfolder, "type": image_type})
    url = f"http://{COMFY_HOST}/view?{qs}"
    with urllib.request.urlopen(url, timeout=90) as r:
        data = r.read()
    dest.write_bytes(data)

def run_prompt(entry: dict) -> Path | None:
    """entry = { section, name, aspect, prompt, seed? }
    Returns path to the downloaded image or None on failure."""
    section = entry["section"]
    name = entry["name"]
    aspect = entry.get("aspect", "wide16x9")
    prompt_text = entry["prompt"]
    seed = entry.get("seed", random.randint(0, 2**31))
    w, h = ASPECTS.get(aspect, ASPECTS["wide16x9"])

    dest_dir = OUT_DIR / section
    dest_dir.mkdir(parents=True, exist_ok=True)
    dest = dest_dir / f"{name}.png"
    if dest.exists() and dest.stat().st_size > 1024:
        print(f"[skip] {section}/{name} already exists")
        return dest

    wf = build_workflow(prompt_text, w, h, seed)
    try:
        pid = queue_prompt(wf)
    except Exception as e:
        print(f"[err ] queue failed for {name}: {e}")
        return None

    print(f"[wait] {section}/{name} ({w}x{h}) — prompt_id={pid[:12]}…")
    hist = poll_history(pid)
    if not hist:
        print(f"[err ] no history for {name}")
        return None

    outputs = hist.get("outputs", {})
    for _node_id, out in outputs.items():
        for img in out.get("images", []):
            download_image(img["filename"], img.get("subfolder", ""), img.get("type", "output"), dest)
            print(f"[ok  ] wrote {dest.relative_to(OUT_DIR.parent)}")
            return dest
    print(f"[warn] no image in output for {name}")
    return None

def load_prompts(path: Path) -> list[dict]:
    return json.loads(path.read_text())

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("prompts_file", help="Path to JSON prompt list")
    ap.add_argument("--limit", type=int, default=None)
    ap.add_argument("--section", default=None, help="Only run entries with this section")
    args = ap.parse_args()

    prompts = load_prompts(Path(args.prompts_file))
    if args.section:
        prompts = [p for p in prompts if p["section"] == args.section]
    if args.limit:
        prompts = prompts[:args.limit]

    print(f"Starting batch of {len(prompts)} prompts → {OUT_DIR}")
    written = 0
    for i, entry in enumerate(prompts, 1):
        print(f"\n[{i}/{len(prompts)}] {entry['section']}/{entry['name']}")
        if run_prompt(entry):
            written += 1
        # Regen the showcase every 4 shots so the site grows LIVE during batches
        if written > 0 and written % 4 == 0:
            _mid_batch_regen()
    print(f"\n== batch complete: {written}/{len(prompts)} images written ==")

def _mid_batch_regen():
    """Fire-and-forget regen + deploy + auto-commit so the showcase grows
    mid-batch AND the worktree stays clean without hand-holding."""
    import subprocess
    try:
        subprocess.Popen(
            ["bash", "-c",
             "cd /home/guardiansoftiktok/snapit-desktop/gallery && "
             "python3 regen_showcase.py > /tmp/mid-regen.log 2>&1 && "
             "export CLOUDFLARE_API_TOKEN=$(grep '^CLOUDFLARE_API_TOKEN=' /home/guardiansoftiktok/sovereign-os/.env | cut -d= -f2 | awk '{print $1}') && "
             "cd /home/guardiansoftiktok/snapit-desktop/purchase-backend && "
             "npx wrangler deploy >> /tmp/mid-regen.log 2>&1 && "
             "cd /home/guardiansoftiktok/snapit-desktop && "
             "git add gallery/out purchase-backend/public/preview-x8f2r7/gallery purchase-backend/public/preview-x8f2r7/showcase.html mock/gallery-manifest.js purchase-backend/public/preview-x8f2r7/gallery-manifest.js && "
             "( git diff --cached --quiet || git commit -m 'snapit(gallery): auto-commit mid-batch showcase drop' >> /tmp/mid-regen.log 2>&1 )"],
            start_new_session=True,
        )
        print("[regen] mid-batch regen + deploy + auto-commit fired in background")
    except Exception as e:
        print(f"[regen] mid-batch trigger failed: {e}")

if __name__ == "__main__":
    main()
