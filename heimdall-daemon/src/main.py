import os
import subprocess
from fastapi import FastAPI, HTTPException, BackgroundTasks
from pydantic import BaseModel
import uvicorn
import logging

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger("heimdall-daemon")

app = FastAPI(title="Heimdall Daemon Controller")

class PullRequest(BaseModel):
    model: str

# Where to store the downloaded checkpoints
# Per architecture spec: This should point to an External SSD mapped via RustFS!
HF_HOME_PATH = os.environ.get("HF_HOME", "/mnt/heimdall_models/cache")

def download_huggingface_model(model_id: str):
    logger.info(f"Starting download for {model_id} to {HF_HOME_PATH}")
    env = os.environ.copy()
    env["HF_HOME"] = HF_HOME_PATH
    
    try:
        # We fire the standard huggingface-cli to reliably pull tensors
        proc = subprocess.run(
            ["huggingface-cli", "download", model_id],
            env=env,
            capture_output=True,
            text=True,
            check=True
        )
        logger.info(f"Successfully downloaded {model_id}. Output: {proc.stdout}")
    except subprocess.CalledProcessError as e:
        logger.error(f"Download failed for {model_id}. Error: {e.stderr}")

@app.post("/pull")
async def pull_model(req: PullRequest, background_tasks: BackgroundTasks):
    """
    Trigger a model download in the background.
    In the MacOS host environment, this streams gigabytes to the rustfs drive.
    """
    if not req.model:
        raise HTTPException(status_code=400, detail="model is required")
        
    # Kick off the download async so we don't timeout the Mimir Bridge API
    background_tasks.add_task(download_huggingface_model, req.model)
    
    return {"status": "started", "message": f"Downloading {req.model} in background"}

@app.get("/health")
def health_check():
    return {"status": "ok", "system": "heimdall", "hf_home": HF_HOME_PATH}

if __name__ == "__main__":
    port = int(os.environ.get("PORT", 3009))
    logger.info(f"Heimdall Daemon starting on port {port}. HF_HOME={HF_HOME_PATH}")
    uvicorn.run(app, host="0.0.0.0", port=port)
