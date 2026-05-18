# Orbifold File Formats

Orbifold currently uses plain text files for settings, projects, and autosave
recovery. These formats are meant to be inspectable during prototype work, but
they are not yet a stable public interchange contract.

## Settings

Default settings are stored in `orbifold_settings.txt` in the working directory.
If that file is missing, Orbifold will try to read the legacy
`microtonal_daw_settings.txt` file for compatibility. New saves always use the
Orbifold filename.

Settings are line-oriented `key=value` pairs:

```text
audio_output_name=
midi_input_name=
scala_path=
lumatone_path=
root_midi=69
base_freq=440
ui_scale=1
show_asset_browser=true
show_scale_browser=false
show_clip_panel=true
layout_left_width=240
layout_track_width=180
layout_right_width=300
layout_bottom_height=420
layout_browser_split_height=190
master_gain=0.35
attack_ms=5
release_ms=100
waveform=sine
drive=1
filter_cutoff_hz=20000
delay_mix=0
delay_feedback=0.25
delay_time_ms=250
midi_debug=false
midi_channel_filter=all
scale_library=scales/31-edo.scl
recent_project=project.orbifold
```

Blank lines and lines beginning with `#` are ignored. Unknown keys, malformed
numbers, invalid booleans, invalid waveforms, and invalid MIDI channel filters
are load errors. Optional path/name fields are represented as empty values.
Repeated `scale_library` and `recent_project` keys append multiple paths.

Settings saves write through a hidden temporary file named like
`.orbifold_settings.txt.<pid>.tmp`, then rename it over the target settings file.
The temp file is removed when the rename fails.

## Projects

Project files use the `.orbifold` extension by convention and begin with the
current marker:

```text
orbifold_project=1
```

The loader still accepts the legacy marker below so old `.mtdaw` project files
can be opened:

```text
microtonal_daw_project=1
```

The rest of the file is line-oriented project state:

```text
scala_path=scales/31-edo.scl
lumatone_path=keymaps/31-edo.ltn
sample_instrument_path=audio_assets/samples/kick.wav
root_midi=69
base_freq=440
waveform=sine
master_gain=0.35
attack_ms=5
release_ms=100
drive=1
filter_cutoff_hz=20000
delay_mix=0
delay_feedback=0.25
delay_time_ms=250
bpm=120
loop_beats=16
overdub=false
quantize_grid=1/16
quantize_on_record=true
metronome_enabled=false
next_note_id=2
```

`sample_instrument_path` is a project reference to the WAV sample used as the
project sample instrument. The file is not packaged into the project. If the
path is missing on load, Orbifold keeps the reference in the project state,
reports it as unavailable, shows `Sample missing ...` in the synth source row,
and lets the user clear the stale reference.

Relative `scala_path`, `lumatone_path`, and `sample_instrument_path` values are
resolved relative to the project file's directory when loading. When saving,
paths under the project directory are written back as relative references so a
project folder can be moved without rewriting those references as absolute
machine-local paths.

Clip notes are stored as tab-separated rows after the scalar fields:

```text
note	1	0	1	69	69	0	69	96	440	false
```

The note columns are:

1. `id`
2. `start_beats`
3. `duration_beats`
4. `key_index`
5. `musical_note`
6. `raw_channel`
7. `raw_note`
8. `velocity`
9. `freq`
10. `mapped_from_lumatone`

Project saves write through a same-directory temporary file named like
`.project.orbifold.<pid>.tmp`. When overwriting an existing project, Orbifold
keeps up to three backup generations:

- `project.orbifold.bak`
- `project.orbifold.bak.2`
- `project.orbifold.bak.3`

The current project moves to `.bak`, older backups shift up one generation, and
the oldest generation is discarded.

## Autosave

Dirty project edits are written to an autosave file using the same text format
as project files. For the default settings path, the recovery file is:

```text
orbifold_autosave.orbifold
```

For non-default settings paths, Orbifold derives the autosave name from the
settings filename by removing `_settings` from the stem and appending
`_autosave.orbifold`.

Autosave recovery is only offered when that path is a real file. Directories,
stale paths, and failed writes are reported as errors instead of being exposed as
recoverable projects. Saving the project or returning to a clean project state
clears the autosave file.

## Compatibility Notes

- Current project saves write `orbifold_project=1`.
- Current settings saves write `orbifold_settings.txt`.
- Legacy `microtonal_daw_project=1` project files are accepted.
- Legacy `microtonal_daw_settings.txt` settings are accepted only as a fallback
  when the default Orbifold settings file is missing.
- Project and settings files are intentionally strict about unknown keys so
  malformed state does not silently turn into hidden defaults.
