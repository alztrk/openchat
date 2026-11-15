import pathlib
import platform
import re
import site
import sys


def patch_header(content):
    for value_type, function in (("double", "rsqrt"), ("float", "rsqrtf")):
        declaration = re.compile(
            rf"(?m)^([ \t]*extern __DEVICE_FUNCTIONS_DECL__ __device_builtin__ {value_type}[ \t]+"
            rf"{function}\({value_type} x\))(?:[ \t]+noexcept\(true\))?;[ \t]*$"
        )
        matches = list(declaration.finditer(content))
        if len(matches) != 1:
            raise RuntimeError(
                f"Unexpected CUDA {function} declaration; reinstall the SGLang runtime."
            )
        match = matches[0]
        if "noexcept(true)" not in match.group(0):
            content = (
                content[: match.start()]
                + match.group(1)
                + " noexcept(true);"
                + content[match.end() :]
            )
    return content


def main():
    libc_name, libc_version = platform.libc_ver()
    version = tuple(int(part) for part in re.findall(r"\d+", libc_version)[:2])
    if libc_name != "glibc" or version < (2, 42):
        return

    cuda_home = next(
        (
            pathlib.Path(directory) / "nvidia" / "cu13"
            for directory in site.getsitepackages()
            if (pathlib.Path(directory) / "nvidia" / "cu13" / "bin" / "nvcc").is_file()
        ),
        None,
    )
    if cuda_home is None:
        raise RuntimeError(
            "The managed SGLang CUDA toolkit is missing; reinstall the SGLang runtime."
        )

    header = cuda_home / "include" / "crt" / "math_functions.h"
    if not header.is_file():
        raise RuntimeError(
            "The managed SGLang CUDA math header is missing; reinstall the SGLang runtime."
        )

    content = header.read_text(encoding="utf-8")
    patched = patch_header(content)
    if patched != content:
        header.write_text(patched, encoding="utf-8")


if __name__ == "__main__":
    main()
