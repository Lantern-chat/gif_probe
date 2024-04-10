gif_probe
=========

Probes a GIF to detect if it _actually_ has transparent pixels, and accumulates misc data while we're at it.

The final algorithm for this is lightweight and simple, and only requires reading the first frame in full.

For the common GIF, there are only two ways to obtain real transparency. It can either have transparent
pixels in the first frame, or clears parts of the image using the `Background` dispose method after a frame.
Technically, the `Background` dispose method is supposed to fill in the frame with the background color,
but everyone ignores that.

Therefore, it is not necessary to actually accumulate and dispose pixels values.

Note: This binary intentionally has bad error handling. It either succeeds or it doesn't.
Any file that fails to process should be considered invalid.

Usage:
```
gif_probe
    [-l max_duration_in_ms]
    [-d max_pixels]
    [-m max_memory_in_bytes]
     -i path/file.gif
```

Or pass `-i -` to read from stdin, which can be useful when spawning as a subprocess.

Example usage in PowerShell 7+:

```powershell
# Using `gif_probe` directly:
gif_probe -i "path/file.gif"

# Using `Get-Content` to read the file as a byte stream. `-AsByteStream -Raw` is required.
Get-Content -Path "path/file.gif" -AsByteStream -Raw | gif_probe -i - | ConvertFrom-Json
```

Example output:
```json
{
  "alpha": false,
  "max_colors": 256,
  "duration": 267,
  "frames": 40,
  "width": 480,
  "height": 270
}
```