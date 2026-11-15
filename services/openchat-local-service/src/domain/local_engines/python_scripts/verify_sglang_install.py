from importlib.metadata import version
from pathlib import Path
import site

import torch

if version("sglang") != "0.5.23":
    raise SystemExit("The installed SGLang version does not match the pinned release.")
if not any(
    (Path(directory) / "nvidia" / "cu13" / "bin" / "nvcc").is_file()
    for directory in site.getsitepackages()
):
    raise SystemExit("The managed CUDA compiler is missing from the SGLang environment.")
if not torch.cuda.is_available():
    raise SystemExit("CUDA is unavailable to SGLang.")
