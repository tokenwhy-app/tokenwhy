from pathlib import Path
from PIL import Image

HERE = Path(__file__).resolve().parent
SRC = HERE.parents[1] / "brand" / "icon-512.png"


def fit(img, w, h, scale):
    side = int(min(w, h) * scale)
    canvas = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    icon = img.resize((side, side), Image.LANCZOS)
    canvas.alpha_composite(icon, ((w - side) // 2, (h - side) // 2))
    return canvas


def main():
    out = HERE / "Assets"
    out.mkdir(exist_ok=True)
    img = Image.open(SRC).convert("RGBA")
    jobs = {
        "Square44x44Logo.png": (44, 44, 1.0),
        "Square150x150Logo.png": (150, 150, 0.66),
        "Wide310x150Logo.png": (310, 150, 0.66),
        "StoreLogo.png": (50, 50, 1.0),
        "SplashScreen.png": (620, 300, 0.66),
    }
    for size in (16, 24, 32, 48, 256):
        jobs[f"Square44x44Logo.targetsize-{size}.png"] = (size, size, 1.0)
        jobs[f"Square44x44Logo.targetsize-{size}_altform-unplated.png"] = (size, size, 1.0)
    for name, (w, h, s) in jobs.items():
        fit(img, w, h, s).save(out / name)


if __name__ == "__main__":
    main()
