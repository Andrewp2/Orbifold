# Asset-To-Sound Workflows

Orbifold can scan, import, preview WAV samples, and use one selected WAV sample
as the project sample instrument. This page describes the current
sound-producing path and the asset workflows that are intentionally not wired
yet.

## What Produces Sound Today

The current playable path is the built-in synth:

1. Connect an audio output in `DEVICES`.
2. Connect a MIDI input, or use the piano roll to create notes.
3. Choose a scale, root, and base frequency.
4. Adjust the synth controls in the right panel.
5. Record MIDI into the current clip or edit notes in the piano roll.
6. Play the clip through the built-in synth.

WAV samples in `audio_assets/` can be auditioned from the asset browser. A
selected WAV sample can also be loaded as the project sample instrument with
`Use`, which makes MIDI and piano-roll notes trigger that sample until `Clear`
is clicked or another sample is loaded. Orbifold saves the sample path in the
project file and restores it when that file is still available. If the saved
sample is missing, Orbifold reports the unavailable file, keeps the missing
reference visible in the right-panel synth source row, and lets `Clear` remove
the stale project reference.
Instruments, presets, and impulses are library items only; selecting one reports
the unavailable sound workflow instead of silently pretending it is active.

## Current Asset Status

- WAV samples can be imported, selected, previewed, stopped, and used as the
  project sample instrument.
- Non-WAV sample files can be imported and selected, but browser preview is only
  available for WAV files right now, and they cannot be used as the project
  sample instrument.
- Instruments can be listed, including supported files and multisample
  directories, but instrument playback is not available yet.
- Presets can be listed, but synth preset loading is not available yet.
- Impulses can be listed, but effects or convolution loading is not available
  yet.

The status bar should say one of these when an asset is selected:

```text
Selected sample: <name> (WAV preview and project sample instrument available)
Selected instrument: <name> (library only; no instrument playback yet)
Selected preset: <name> (library only; no synth preset loading yet)
Selected impulse: <name> (library only; no effects loading yet)
```

## Useful Workflow For Now

Use the asset browser to prepare and audition material:

1. Put files under `audio_assets/samples/`, `audio_assets/instruments/`,
   `audio_assets/presets/`, or `audio_assets/impulses/`.
2. Click `Refresh Assets`.
3. Use `Import` when you want Orbifold to copy a file into the selected asset
   folder and avoid overwriting existing filenames.
4. Confirm the row appears and can be selected.
5. Click `Preview` to audition a WAV sample, or `Stop` to cancel playback.
6. Click `Use` to make the selected WAV the project sample instrument.
7. Play MIDI or the piano roll. Click `Clear` to return notes to the built-in
   synth waveform path. The right-panel synth source row also shows the loaded
   sample and offers `Clear`. If a project references a moved or deleted sample,
   that row shows `Sample missing ...` until you clear the stale reference.

If you need to verify general audio output, use `A4`, MIDI input, or an
existing clip. Sample preview is a browser audition. `Use` is the current
sample-to-notes path, and the selected sample path is saved with the project.

## Target Workflow Not Yet Implemented

These are product targets, not current behavior:

- Drag a sample to a clip, arrangement lane, or sampler.
- Package used sample files with a project or relink moved samples. Missing
  sample references are visible and clearable, but not repairable yet.
- Load an instrument definition and assign it to a track or clip.
- Apply a preset to the synth or an effect chain.
- Load an impulse into a convolution or effects workflow.
- Save project references to used assets and repair missing asset paths.

For supported folders, file extensions, refresh behavior, and import conflict
renaming, see `docs/asset_browser.md`.
