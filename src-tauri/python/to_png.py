import os
import subprocess
import sys

OUT_FORMAT: str = "png"
ACCEPTED_EXTENSIONS: set[str] = {"png", "jpg", "jpeg", "bmp", "webp", "gif", "tiff", "avif"}
RESULT_FOLDER: str = "outputs_converted"

def main(file_path: str) -> None:
    if not os.path.isdir(RESULT_FOLDER):
        try:
            os.makedirs(RESULT_FOLDER)
        except (PermissionError, FileExistsError):
            sys.exit(1)

    if not os.path.isfile(file_path):
        sys.exit(1)

    file_prefix, file_ext = os.path.splitext(file_path)
    file_ext = file_ext.lstrip('.').lower()
    file_ext = file_ext.lstrip('.').lower()

    if file_ext not in ACCEPTED_EXTENSIONS:
        sys.exit(1)

    OUTPUT_PATH: str = f"{RESULT_FOLDER}/{file_prefix}_converted_by_alris.{OUT_FORMAT}"

    try:
        subprocess.run(
            ["ffmpeg",
                "-y", # always answer yes
                "-i",
                file_path,
                OUTPUT_PATH],
            capture_output=False,
            text=True,
            check=True
        )
    except FileNotFoundError:
        sys.exit(1)
    except subprocess.CalledProcessError:
        sys.exit(1)

if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(1) # fail
    main(sys.argv[1]) # run main
    # success
    sys.exit(0)
