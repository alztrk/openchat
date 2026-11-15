import glob
import os
import runpy
import site
import sys

managed_cuda_home = next(
    (
        os.path.join(site_packages, "nvidia", "cu13")
        for site_packages in site.getsitepackages()
        if os.path.isfile(os.path.join(site_packages, "nvidia", "cu13", "bin", "nvcc"))
    ),
    None,
)
if managed_cuda_home is None:
    raise RuntimeError("The managed SGLang CUDA compiler is missing; reinstall the SGLang runtime.")

session_directory = os.path.dirname(os.path.realpath(__file__))
cuda_home = os.path.join(session_directory, "cuda")
cuda_libraries = os.path.join(cuda_home, "lib64")
os.makedirs(cuda_libraries, exist_ok=True)
driver_libraries = ("/usr/lib/wsl/lib", "/usr/lib/x86_64-linux-gnu", "/usr/lib64")

for directory in ("bin", "include"):
    destination = os.path.join(cuda_home, directory)
    if not os.path.lexists(destination):
        os.symlink(os.path.join(managed_cuda_home, directory), destination)

for library in glob.glob(os.path.join(managed_cuda_home, "lib", "*.so*")):
    destination = os.path.join(cuda_libraries, os.path.basename(library))
    if not os.path.lexists(destination):
        os.symlink(library, destination)

cudart = next(iter(glob.glob(os.path.join(managed_cuda_home, "lib", "libcudart.so.*"))), None)
if cudart is None:
    raise RuntimeError("The managed SGLang CUDA runtime library is missing; reinstall the SGLang runtime.")
cudart_link = os.path.join(cuda_libraries, "libcudart.so")
if not os.path.lexists(cudart_link):
    os.symlink(cudart, cudart_link)

for driver_directory in driver_libraries:
    driver = next(
        (
            os.path.join(driver_directory, name)
            for name in ("libcuda.so", "libcuda.so.1")
            if os.path.isfile(os.path.join(driver_directory, name))
        ),
        None,
    )
    if driver is not None:
        destination = os.path.join(cuda_libraries, "libcuda.so")
        if not os.path.lexists(destination):
            os.symlink(driver, destination)
        break

os.environ["CUDA_HOME"] = cuda_home
os.environ["CUDA_PATH"] = cuda_home
os.environ["PATH"] = os.path.join(cuda_home, "bin") + os.pathsep + os.environ.get("PATH", "")
for variable in ("LIBRARY_PATH", "LD_LIBRARY_PATH"):
    paths = [cuda_libraries]
    paths.extend(driver_libraries)
    paths.extend(filter(None, os.environ.get(variable, "").split(os.pathsep)))
    os.environ[variable] = os.pathsep.join(dict.fromkeys(paths))

token_path = os.environ["OPENCHAT_SGLANG_TOKEN_FILE"]
with open(token_path, encoding="utf-8") as token_file:
    token = token_file.read().strip()
sys.argv = ["sglang.launch_server", *sys.argv[1:], "--api-key", token]
if __name__ == "__main__":
    runpy.run_module("sglang.launch_server", run_name="__main__")
