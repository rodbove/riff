import argparse
import json
import os
import subprocess
import sys
import tempfile

import numpy as np
import scipy.io.wavfile as wavfile
import scipy.signal as sg

BANDS = [63, 80, 100, 125, 160, 200, 250, 315, 400, 500, 630, 800, 1000, 1250,
         1600, 2000, 2500, 3150, 4000, 5000, 6300, 8000, 10000, 12500]
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def read(path, rate=None):
    r, raw = wavfile.read(path)
    x = raw.astype(np.float64)
    if np.issubdtype(raw.dtype, np.integer):
        x /= float(np.iinfo(raw.dtype).max)
    if x.ndim > 1:
        x = x.mean(1)
    if rate and r != rate:
        g = np.gcd(r, rate)
        x = sg.resample_poly(x, rate // g, r // g)
        r = rate
    return r, x


def onsets(x, hop=120):
    k = len(x) // hop
    e = np.log(np.sqrt((x[:k * hop].reshape(k, hop) ** 2).mean(1)) + 1e-5)
    o = np.maximum(np.diff(e), 0)
    return (o - o.mean()) / (o.std() + 1e-9)


def align(ref, di, hop=120, max_s=1.0, rate=48000):
    a, b = onsets(di), onsets(ref)
    n = min(len(a), len(b))
    c = sg.correlate(a[:n], b[:n], mode="full")
    lags = np.arange(-n + 1, n)
    keep = np.abs(lags) * hop < max_s * rate
    lag = int(lags[np.argmax(c * keep)] * hop)
    return np.roll(ref, lag), lag


def bands(x, rate):
    f, p = sg.welch(x, rate, nperseg=8192)
    p /= p.sum()
    return np.array([10 * np.log10(p[(f >= c / 2 ** (1 / 6)) & (f < c * 2 ** (1 / 6))].sum() + 1e-20) for c in BANDS])


def flatness(x, rate, lo, hi):
    f, _, z = sg.stft(x, rate, nperseg=2048)
    m = (f >= lo) & (f < hi)
    p = np.abs(z[m]) ** 2 + 1e-12
    return float(np.mean(np.exp(np.mean(np.log(p), 0)) / np.mean(p, 0)))


def spread(x, di, hop=480):
    k = min(len(x), len(di)) // hop
    e = 20 * np.log10(np.sqrt((x[:k * hop].reshape(k, hop) ** 2).mean(1)) + 1e-9)
    d = 20 * np.log10(np.sqrt((di[:k * hop].reshape(k, hop) ** 2).mean(1)) + 1e-9)
    playing = d > np.percentile(d, 30)
    lo, hi = np.percentile(e[playing], [10, 90])
    return hi - lo


def describe(name, x, ref, di, rate):
    b, rb = bands(x, rate), bands(ref, rate)
    d = b - rb
    used = (np.array(BANDS) >= 80) & (np.array(BANDS) <= 8000)
    err = float(np.sqrt(np.mean(d[used] ** 2)))
    fl = [flatness(x, rate, lo, hi) for lo, hi in [(250, 1000), (1000, 4000), (4000, 10000)]]
    print(f"{name}: distance {err:.2f} dB   noise 250-1k {fl[0]:.3f}  1k-4k {fl[1]:.3f}  4k-10k {fl[2]:.3f}   dynamics {spread(x, di):.1f} dB")
    print("  band (Hz)   " + " ".join(f"{c:>6}" for c in BANDS))
    print("  vs ref (dB) " + " ".join(f"{v:+6.1f}" for v in d))


def main():
    ap = argparse.ArgumentParser(description="Compare riff renders with a reference render of the same DI.")
    ap.add_argument("--di", required=True, help="the dry take (mono WAV)")
    ap.add_argument("--ref", required=True, help="the reference render, for example from AmpliTube")
    ap.add_argument("--rig", action="append", default=[], help="a rig JSON file to render (repeat for more)")
    ap.add_argument("--preset", action="append", default=[], help="a factory preset name to render (repeat for more)")
    ap.add_argument("--wav", action="append", default=[], help="a riff render that already exists (repeat for more)")
    ap.add_argument("--seconds", type=float, default=0, help="use only the first N seconds")
    ap.add_argument("--keep", help="folder to keep the renders in")
    a = ap.parse_args()

    rate, di = read(a.di)
    _, ref = read(a.ref, rate)
    ref, lag = align(ref, di, rate=rate)
    n = min(len(ref), len(di))
    if a.seconds:
        n = min(n, int(a.seconds * rate))
    di, ref = di[:n], ref[:n]
    skip = min(rate, n // 10)
    print(f"reference moved {lag / rate * 1000:+.1f} ms to line up with the DI, {n / rate:.1f} s used\n")
    noise = [flatness(ref[skip:], rate, lo, hi) for lo, hi in [(250, 1000), (1000, 4000), (4000, 10000)]]
    print(f"reference: noise 250-1k {noise[0]:.3f}  1k-4k {noise[1]:.3f}  4k-10k {noise[2]:.3f}   dynamics {spread(ref, di):.1f} dB\n")

    out = a.keep or tempfile.mkdtemp(prefix="riff-tone-")
    os.makedirs(out, exist_ok=True)
    renders = list(a.wav)
    rigs = list(a.rig)
    for name in a.preset:
        dump = subprocess.run(["cargo", "run", "-q", "--release", "-p", "riff-dsp", "--example", "render", "--", "dump", name],
                              cwd=ROOT, capture_output=True, text=True, check=True).stdout
        path = os.path.join(out, name.replace("/", "-") + ".json")
        open(path, "w").write(dump)
        rigs.append(path)
    for rig in rigs:
        wav = os.path.join(out, os.path.splitext(os.path.basename(rig))[0] + ".wav")
        subprocess.run(["cargo", "run", "-q", "--release", "-p", "riff-dsp", "--example", "render", "--",
                        "render", os.path.abspath(rig), os.path.abspath(a.di), wav, str(n / rate)], cwd=ROOT, check=True)
        renders.append(wav)
    for path in renders:
        _, x = read(path, rate)
        x = x[:n]
        describe(os.path.basename(path), x[skip:], ref[skip:len(x)], di[skip:len(x)], rate)
        print()
    if renders and not a.keep:
        print(f"renders are in {out}")


if __name__ == "__main__":
    sys.exit(main())
