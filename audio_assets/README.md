# Audio Assets

Project audio content lives here. Keep source code in `src/` and put reusable sound material in these folders:

- `samples/`: one-shots, loops, recordings, and imported audio clips.
- `instruments/`: instrument definitions, multisample maps, and sample sets.
- `presets/`: synth, effect, routing, and project sound presets.
- `impulses/`: impulse responses and other convolution assets.

The app browser scans these folders at startup and when `Refresh` is pressed. `Import` copies a chosen file into the currently selected category.

Prefer clear filenames with tempo, pitch, or tuning details when they matter, for example `kick_soft_96bpm.wav` or `kalimba_c4_17edo.wav`.

Large third-party libraries should usually stay outside the repo unless they are intentionally vendored for the project.
