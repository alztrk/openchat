import exllamav3
import torch

if not torch.cuda.is_available():
    raise SystemExit("CUDA is unavailable to ExLlamaV3.")
