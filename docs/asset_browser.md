# Asset Browser

Orbifold has an early asset browser for reusable sound-related files. It is a
library and import surface today; it is not yet a full sampler, preset browser,
or drag-to-track workflow.

## Folders

Assets live under `audio_assets/` in category folders:

- `audio_assets/samples/`
- `audio_assets/instruments/`
- `audio_assets/presets/`
- `audio_assets/impulses/`

Orbifold creates these folders on startup or refresh if they do not exist.

## Supported Files

The browser accepts these extensions by category:

- Samples: `wav`, `aif`, `aiff`, `flac`, `ogg`, `mp3`
- Instruments: `sfz`, `sf2`, `json`, `toml`, `yaml`, `yml`
- Presets: `json`, `toml`, `yaml`, `yml`, `ron`, `preset`
- Impulses: `wav`, `aif`, `aiff`, `flac`

The scan is recursive. Hidden files and folders whose names start with `.` are
ignored. Instrument subdirectories are also shown as rows with a trailing `/`,
so a multisample folder can be selected as a single library item.

## Refreshing

The left browser scans assets on startup. Use `Refresh` after manually adding,
removing, or moving files under `audio_assets/`.

Refresh preserves the selected asset when the same path still exists. If the
selected file disappears, refresh clears the stale selection and reports that in
the status bar. Rows for missing files may remain marked until the next refresh.
Existing file rows show a compact file size so imported libraries are easier to
scan at a glance.

The selected asset stays visible in a compact detail strip near the browser
actions. It shows the selected row name plus the current workflow status, such
as WAV-preview availability, missing-file state, or library-only limitations.
Selected WAV samples also show duration, sample rate, channel count, and file
size when the metadata can be read.

## Searching

Use the search field at the top of the asset browser to filter the current asset
category. Search terms match asset names, paths, category names, and singular
category labels. Multiple terms must all match. `Backspace` edits the query,
`Escape` clears it, and the visible `Clear` button appears while a search is
active. If no rows match, the empty state points back to clearing the search.

## Importing

Use the asset category tabs to choose `Samples`, `Instruments`, `Presets`, or
`Impulses`, then click `Import`.

Import copies the chosen file into the selected category folder. It does not
move or delete the source file. Unsupported extensions are rejected with a
visible status error.

If the target filename already exists, Orbifold keeps the existing file and
chooses a unique name:

```text
kick.wav
kick_2.wav
kick_3.wav
```

The status bar reports the rename, for example:

```text
Imported sample as kick_2.wav (kick.wav already exists)
```

## Previewing And Using Samples

Select a WAV sample and click `Preview` to audition it through the connected
audio output. `Stop` cancels the current preview. Preview is deliberately narrow
for now: it supports WAV files only and plays them as a quick browser audition.

Click `Use` to load the selected WAV as the project sample instrument.
MIDI and piano-roll notes will trigger that sample until `Clear` is clicked or
another sample is loaded. Orbifold saves the sample path in the project file and
restores it when that file is still available. If a saved sample path is missing,
the right-panel synth source row shows `Sample missing ...` and keeps `Clear`
available so the stale project reference is explicit and removable.

If audio is not connected, preview reports that in the status bar instead of
silently doing nothing. Non-WAV sample files can still be imported and listed,
but preview and sample-instrument loading report the current WAV-only
limitation as `WAV required for preview/use`.

## Current Limitations

- Only WAV samples can be previewed from the browser.
- Non-WAV sample rows stay visible but report `WAV required for preview/use`.
- Browser-imported asset bytes are stored in IndexedDB and restored on reload.
  Legacy `localStorage` records are merged into IndexedDB when possible, and
  still act as a fallback if IndexedDB is unavailable. Browser storage quotas
  still apply.
- Project files save the selected sample path and keep missing references
  visible, but they do not package or relink moved sample files yet.
- Waveform thumbnails, tags, and deep codec metadata are not shown.
- Assets cannot yet be dragged to the arrangement or a track.
- Presets are not yet loadable into the synth.
- Impulses are listed but no convolution/effects workflow exists yet.
- Tags, favorites, and rich missing-file repair are not implemented.

The current useful workflow is: collect files, import them into known folders,
preview WAV samples when needed, optionally load one as the project sample
instrument, confirm they are visible, and use the status bar to catch
unsupported files or name conflicts.

For what can and cannot currently make sound, see `docs/asset_to_sound.md`.
